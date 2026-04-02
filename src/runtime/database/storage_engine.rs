use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::Path;
use serde::{Serialize, Deserialize};
use crate::runtime::execution::df_engine::DataChunk;

#[derive(Debug, Serialize, Deserialize)]
pub struct PageHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub chunk_size: u32,
    pub row_count: u32,
    pub checksum: u32,
}

pub struct NyxBlockStorage {
    pub base_path: String,
}

impl NyxBlockStorage {
    pub fn new(base_path: String) -> Self {
        if !Path::new(&base_path).exists() {
            std::fs::create_dir_all(&base_path).unwrap();
        }
        Self { base_path }
    }

    pub fn write_chunk(&self, table_name: &str, chunk: &DataChunk) -> std::io::Result<u64> {
        let file_path = format!("{}/{}.nyx", self.base_path, table_name);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        let serialized = bincode::serialize(chunk).map_err(std::io::Error::other)?;
        let checksum = crc32fast::hash(&serialized);
        let header = PageHeader {
            magic: *b"NYXB",
            version: 1,
            chunk_size: serialized.len() as u32,
            row_count: chunk.size as u32,
            checksum,
        };

        let start_pos = file.seek(SeekFrom::End(0))?;
        file.write_all(&bincode::serialize(&header).map_err(std::io::Error::other)?)?;
        file.write_all(&serialized)?;
        file.sync_all()?;

        Ok(start_pos)
    }

    pub fn read_chunks(&self, table_name: &str) -> std::io::Result<Vec<DataChunk>> {
        let file_path = format!("{}/{}.nyx", self.base_path, table_name);
        if !Path::new(&file_path).exists() {
            return Ok(Vec::new());
        }

        let mut file = File::open(file_path)?;
        let mut chunks = Vec::new();

        loop {
            let mut header_buf = [0u8; 17]; // Size of serialized PageHeader
            if file.read_exact(&mut header_buf).is_err() { break; }
            let header: PageHeader = bincode::deserialize(&header_buf).map_err(std::io::Error::other)?;

            let mut body_buf = vec![0u8; header.chunk_size as usize];
            file.read_exact(&mut body_buf)?;

            let calc_checksum = crc32fast::hash(&body_buf);
            if header.checksum != 0 && calc_checksum != header.checksum {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("CRC mismatch in chunk database! Expected: {}, Calc: {}", header.checksum, calc_checksum)
                ));
            }

            let chunk: DataChunk = bincode::deserialize(&body_buf).map_err(std::io::Error::other)?;
            chunks.push(chunk);
        }

        Ok(chunks)
    }
}
