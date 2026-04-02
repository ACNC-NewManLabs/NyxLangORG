use std::io::{Write, Read, Seek, SeekFrom};
use serde::{Serialize, Deserialize};
use serde_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalOp {
    InsertChunk { table_name: String, chunk_id: u64, row_count: usize },
    CreateTable { name: String, schema_json: String },
    DropTable { name: String },
    Commit { tx_id: u64 },
    Abort { tx_id: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalEntry {
    lsn: u64, // Log Sequence Number
    tx_id: u64,
    op: WalOp,
    checksum: u32,
}

pub struct WalEngine {
    path: String,
    file: std::fs::File,
    current_lsn: u64,
}

impl WalEngine {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)?;
        
        let mut engine = Self {
            path: path.to_string(),
            file,
            current_lsn: 0,
        };
        
        println!("[WAL] Initialized at {}", engine.path);
        
        // Recover LSN
        engine.current_lsn = engine.find_last_lsn()?;
        Ok(engine)
    }

    fn find_last_lsn(&mut self) -> std::io::Result<u64> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut last_lsn = 0;
        let mut reader = std::io::BufReader::new(&self.file);
        
        while let Ok(entry) = self.read_entry(&mut reader) {
            last_lsn = entry.lsn;
        }
        
        self.file.seek(SeekFrom::End(0))?;
        Ok(last_lsn)
    }

    fn read_entry<R: Read>(&self, reader: &mut R) -> std::io::Result<WalEntry> {
        let mut size_buf = [0u8; 8];
        reader.read_exact(&mut size_buf)?;
        let size = u64::from_le_bytes(size_buf) as usize;
        
        let mut buf = vec![0u8; size];
        reader.read_exact(&mut buf)?;
        
        let entry: WalEntry = serde_json::from_slice(&buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(entry)
    }

    pub fn log_op(&mut self, tx_id: u64, op: WalOp) -> std::io::Result<u64> {
        self.current_lsn += 1;
        let entry = WalEntry {
            lsn: self.current_lsn,
            tx_id,
            op,
            checksum: 0, // Simplified
        };
        
        let buf = serde_json::to_vec(&entry).unwrap_or_default();
        self.file.write_all(&(buf.len() as u64).to_le_bytes())?;
        self.file.write_all(&buf)?;
        self.file.sync_all()?; // Force flush to disk
        
        Ok(self.current_lsn)
    }

    pub fn replay<F>(&mut self, mut apply_op: F) -> std::io::Result<()> 
    where F: FnMut(WalOp) {
        self.file.seek(SeekFrom::Start(0))?;
        let mut reader = std::io::BufReader::new(&self.file);
        
        while let Ok(entry) = self.read_entry(&mut reader) {
            apply_op(entry.op);
        }
        
        self.file.seek(SeekFrom::End(0))?;
        Ok(())
    }
}
