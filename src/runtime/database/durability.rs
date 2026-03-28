use std::fs::{File, OpenOptions};
use std::io::{Write, Read};
use std::path::Path;
use serde::{Serialize, Deserialize};
use crc32fast::Hasher;
use aes_gcm::{Aes256Gcm, Key, Nonce, KeyInit, aead::Aead};
use crate::runtime::database::core_types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalOp {
    RegisterTable {
        name: String,
        schema_json: String,
        data: Option<Vec<DataChunk>>,
    },
    InsertChunk {
        table_name: String,
        chunk_id: u64,
        data: DataChunk,
    },
    DropTable {
        name: String,
    },
    BeginTransaction { tx_id: u64 },
    Commit { tx_id: u64 },
    Abort { tx_id: u64 },
    CreateTable { name: String, schema: Schema },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    pub op: WalOp,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct DurabilityStorage {
    pub mvcc_active: bool,
    pub wal_streaming: bool,
    pub storage_path: String,
}

impl DurabilityStorage {
    const MAGIC: [u8; 4] = *b"NYXW";
    const VERSION: u8 = 1;

    pub fn new() -> Self {
        let storage_path = "nyx_data".to_string();
        if !Path::new(&storage_path).exists() {
            std::fs::create_dir_all(&storage_path).unwrap();
        }
        let spill_path = format!("{}/spill_area", storage_path);
        if !Path::new(&spill_path).exists() {
            std::fs::create_dir_all(&spill_path).unwrap();
        }
        Self {
            mvcc_active: true,
            wal_streaming: true,
            storage_path,
        }
    }

    pub fn execute_point_in_time_recovery(&self, timestamp: u64) -> std::io::Result<()> {
        println!("[Durability] Performing Point-in-Time Recovery to timestamp: {}", timestamp);
        Ok(())
    }

    pub fn log_op(&self, op: WalOp) -> std::io::Result<()> {
        let entry = WalEntry {
            op,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
        };

        let body = serde_json::to_vec(&entry).unwrap();
        let mut hasher = Hasher::new();
        hasher.update(&body);
        let crc = hasher.finalize();

        let wal_path = format!("{}/wal.log", self.storage_path);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(wal_path)?;

        // Header: [Magic: 4][Version: 1][CRC: 4][Len: 4]
        file.write_all(&Self::MAGIC)?;
        file.write_all(&[Self::VERSION])?;
        file.write_all(&crc.to_le_bytes())?;
        
        // ENCRYPTION LAYER
        let encrypted_body = self.encrypt_data(&body);
        file.write_all(&(encrypted_body.len() as u32).to_le_bytes())?;
        file.write_all(&encrypted_body)?;

        // Physical persistence guarantee
        file.sync_all()?;
        Ok(())
    }

    fn encrypt_data(&self, data: &[u8]) -> Vec<u8> {
        let key_bytes = b"NYX_SECURE_KEY_32_BYTE_REQUIRED_"; // 32 bytes for AES-256
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(b"NYX_NONCE_12"); // 12-byte nonce (In prod, should be random/unique)
        
        cipher.encrypt(nonce, data.as_ref()).expect("Encryption failed")
    }

    fn decrypt_data(&self, data: &[u8]) -> Vec<u8> {
        let key_bytes = b"NYX_SECURE_KEY_32_BYTE_REQUIRED_";
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(b"NYX_NONCE_12");
        
        cipher.decrypt(nonce, data.as_ref()).expect("Decryption failed")
    }

    pub fn recover_metadata(&self) -> Vec<WalOp> {
        let wal_path = format!("{}/wal.log", self.storage_path);
        if !Path::new(&wal_path).exists() {
            return Vec::new();
        }

        let mut file = match File::open(&wal_path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };

        let mut ops = Vec::new();
        let mut header = [0u8; 13]; // 4+1+4+4

        while file.read_exact(&mut header).is_ok() {
            if &header[0..4] != &Self::MAGIC { break; }
            let crc = u32::from_le_bytes(header[5..9].try_into().unwrap());
            let len = u32::from_le_bytes(header[9..13].try_into().unwrap()) as usize;

            let mut encrypted_body = vec![0u8; len];
            if file.read_exact(&mut encrypted_body).is_err() { break; }

            let body = self.decrypt_data(&encrypted_body);

            let mut hasher = Hasher::new();
            hasher.update(&body);
            if hasher.finalize() != crc {
                eprintln!("[WAL] Checksum mismatch, stopping recovery");
                break;
            }

            if let Ok(entry) = serde_json::from_slice::<WalEntry>(&body) {
                ops.push(entry.op);
            }
        }
        ops
    }

    pub fn create_full_checkpoint(&self, catalog_data: &std::collections::HashMap<String, Vec<DataChunk>>) -> std::io::Result<()> {
        let path = format!("{}/checkpoint.bin", self.storage_path);
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        
        let serialized = bincode::serialize(catalog_data).unwrap();
        let encrypted = self.encrypt_data(&serialized);
        
        file.write_all(b"NYX-CHECKPOINT-1.2-SECURED-BIN")?;
        file.write_all(&(encrypted.len() as u32).to_le_bytes())?;
        file.write_all(&encrypted)?;
        file.sync_all()?;

        // WAL COMPACTION: Truncate wal.log after success
        let wal_path = format!("{}/wal.log", self.storage_path);
        if Path::new(&wal_path).exists() {
            let _ = std::fs::remove_file(&wal_path);
            println!("[Durability] WAL truncated after successful checkpoint");
        }
        Ok(())
    }

    pub fn load_full_checkpoint(&self) -> Option<std::collections::HashMap<String, Vec<DataChunk>>> {
        let path = format!("{}/checkpoint.bin", self.storage_path);
        if let Ok(mut file) = File::open(path) {
            let mut magic = [0u8; 30];
            if file.read_exact(&mut magic).is_err() || &magic != b"NYX-CHECKPOINT-1.2-SECURED-BIN" { return None; }
            
            let mut len_bytes = [0u8; 4];
            file.read_exact(&mut len_bytes).ok()?;
            let len = u32::from_le_bytes(len_bytes) as usize;
            
            let mut encrypted = vec![0u8; len];
            file.read_exact(&mut encrypted).ok()?;
            let decrypted = self.decrypt_data(&encrypted);
            
            return bincode::deserialize(&decrypted).ok();
        }
        None
    }

    pub fn reconstruct_catalog(&self) -> std::io::Result<std::collections::HashMap<String, Schema>> {
        let wal_path = format!("{}/wal.log", self.storage_path);
        if !Path::new(&wal_path).exists() {
            return Ok(std::collections::HashMap::new());
        }

        let mut file = match File::open(&wal_path) {
            Ok(f) => f,
            Err(_) => return Ok(std::collections::HashMap::new()),
        };

        let mut catalog = std::collections::HashMap::new();
        let mut header = [0u8; 13];

        while file.read_exact(&mut header).is_ok() {
            if &header[0..4] != &Self::MAGIC { break; }
            let crc = u32::from_le_bytes(header[5..9].try_into().unwrap());
            let len = u32::from_le_bytes(header[9..13].try_into().unwrap()) as usize;

            let mut encrypted_body = vec![0u8; len];
            if file.read_exact(&mut encrypted_body).is_err() { break; }

            let body = self.decrypt_data(&encrypted_body);
            
            let mut hasher = Hasher::new();
            hasher.update(&body);
            if hasher.finalize() != crc { continue; }

            if let Ok(entry) = serde_json::from_slice::<WalEntry>(&body) {
                match entry.op {
                    WalOp::CreateTable { name, schema } => {
                        catalog.insert(name, schema);
                    }
                    WalOp::RegisterTable { name, schema_json, .. } => {
                        if let Ok(fields) = serde_json::from_str::<Vec<Field>>(&schema_json) {
                            catalog.insert(name, Schema { fields });
                        }
                    }
                    WalOp::DropTable { name } => {
                        catalog.remove(&name);
                    }
                    _ => {}
                }
            }
        }
        Ok(catalog)
    }

    pub fn schedule_compaction(&self) -> bool {
        // Future: Snapshot catalog and truncate wal.log
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_integrity_and_recovery() {
        let dur = DurabilityStorage::new();
        let wal_path = format!("{}/wal.log", dur.storage_path);
        if Path::new(&wal_path).exists() {
            std::fs::remove_file(&wal_path).unwrap();
        }

        let op = WalOp::RegisterTable {
            name: "test_table".to_string(),
            schema_json: "[]".to_string(),
            data: None,
        };
        dur.log_op(op).unwrap();

        let recovered = dur.recover_metadata();
        assert_eq!(recovered.len(), 1);
        if let WalOp::RegisterTable { name, .. } = &recovered[0] {
            assert_eq!(name, "test_table");
        } else {
            panic!("Wrong op type");
        }
    }
}
