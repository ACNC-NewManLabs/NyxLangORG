use std::io::{Write, Read, Seek};
use std::sync::Arc;
use serde_json;
use crate::runtime::execution::df_engine::{DataChunk, Column, ColumnData, Schema, Bitmap};

/// NyxTable binary format:
/// [Header: 4 bytes "NYXT"]
/// [Version: 4 bytes]
/// [Schema Length: 8 bytes]
/// [Schema JSON]
/// [Number of Chunks: 8 bytes]
/// For each chunk:
///   [Chunk Row Count: 8 bytes]
///   For each column:
///     [Data Type: 1 byte]
///     [Data Size: 8 bytes]
///     [Raw Data]
///     [Raw Validity Bitmap Size: 8 bytes]
///     [Raw Validity Bitmap]

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlockStats {
    pub min_f64: Option<f64>,
    pub max_f64: Option<f64>,
    pub min_i64: Option<i64>,
    pub max_i64: Option<i64>,
}

pub struct NyxTableWriter;

impl NyxTableWriter {
    pub fn write_to_file(path: &str, schema: &Schema, chunks: &[DataChunk]) -> std::io::Result<()> {
        let mut file = std::fs::File::create(path)?;
        
        // Header & Version (v2)
        file.write_all(b"NYXT")?;
        file.write_all(&2u32.to_le_bytes())?;
        
        // Schema
        let schema_json = serde_json::to_string(schema).unwrap_or_else(|_| "[]".to_string());
        file.write_all(&(schema_json.len() as u64).to_le_bytes())?;
        file.write_all(schema_json.as_bytes())?;
        
        // Number of Blocks
        file.write_all(&(chunks.len() as u64).to_le_bytes())?;
        
        let mut block_offsets = Vec::with_capacity(chunks.len());
        
        for chunk in chunks {
            // Record block start offset
            let current_offset = file.stream_position()?;
            block_offsets.push(current_offset);
            
            file.write_all(&(chunk.size as u64).to_le_bytes())?;
            
            for col in &chunk.columns {
                // Calculate Stats
                let stats = match &col.data {
                    ColumnData::F64(v) => {
                        let mut min = f64::MAX;
                        let mut max = f64::MIN;
                        for &x in v.iter() {
                            if x < min { min = x; }
                            if x > max { max = x; }
                        }
                        BlockStats { min_f64: Some(min), max_f64: Some(max), min_i64: None, max_i64: None }
                    }
                    ColumnData::I64(v) => {
                        let mut min = i64::MAX;
                        let mut max = i64::MIN;
                        for &x in v.iter() {
                            if x < min { min = x; }
                            if x > max { max = x; }
                        }
                        BlockStats { min_f64: None, max_f64: None, min_i64: Some(min), max_i64: Some(max) }
                    }
                    _ => BlockStats { min_f64: None, max_f64: None, min_i64: None, max_i64: None },
                };

                // Write Stats JSON
                let stats_json = serde_json::to_string(&stats).unwrap_or_else(|_| "{}".to_string());
                file.write_all(&(stats_json.len() as u64).to_le_bytes())?;
                file.write_all(stats_json.as_bytes())?;

                // Data capture
                let data_to_write = match &col.data {
                    ColumnData::F64(v) => unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 8) }.to_vec(),
                    ColumnData::I64(v) => unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 8) }.to_vec(),
                    ColumnData::Bool(v) => v.iter().map(|&b| if b { 1u8 } else { 0u8 }).collect(),
                    ColumnData::Str { data, offsets } => {
                        let mut buf = (offsets.len() as u64).to_le_bytes().to_vec();
                        let offset_bytes: &[u8] = unsafe {
                            std::slice::from_raw_parts(offsets.as_ptr() as *const u8, offsets.len() * 8)
                        };
                        buf.extend_from_slice(offset_bytes);
                        buf.extend_from_slice(data);
                        buf
                    }
                    _ => vec![],
                };

                let dtype_byte = match &col.data {
                    ColumnData::F64(_) => 1u8,
                    ColumnData::I64(_) => 2u8,
                    ColumnData::Bool(_) => 3u8,
                    ColumnData::Str { .. } => 4u8,
                    _ => 0u8,
                };

                file.write_all(&[dtype_byte])?;
                file.write_all(&(data_to_write.len() as u64).to_le_bytes())?;
                file.write_all(&data_to_write)?;
                
                // Validity
                if let Some(bitmap) = &col.validity {
                    file.write_all(&(bitmap.data.len() as u64).to_le_bytes())?;
                    file.write_all(&bitmap.data)?;
                } else {
                    file.write_all(&0u64.to_le_bytes())?;
                }
            }
        }
        
        // Write Block Index (Trailer)
        let trailer_offset = file.stream_position()?;
        for offset in block_offsets {
            file.write_all(&offset.to_le_bytes())?;
        }
        file.write_all(&trailer_offset.to_le_bytes())?;
        
        Ok(())
    }

    pub fn read_from_file(path: &str) -> std::io::Result<(Schema, Vec<DataChunk>)> {
        let mut file = std::fs::File::open(path)?;
        let mut header = [0u8; 4];
        file.read_exact(&mut header)?;
        if &header != b"NYXT" {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid NyxTable header"));
        }
        
        let mut version_buf = [0u8; 4];
        file.read_exact(&mut version_buf)?;
        let version = u32::from_le_bytes(version_buf);
        
        let mut schema_len_buf = [0u8; 8];
        file.read_exact(&mut schema_len_buf)?;
        let schema_len = u64::from_le_bytes(schema_len_buf);
        let mut schema_json = vec![0u8; schema_len as usize];
        file.read_exact(&mut schema_json)?;
        let schema: Schema = serde_json::from_slice(&schema_json).expect("Corrupt schema JSON");
        
        let mut num_blocks_buf = [0u8; 8];
        file.read_exact(&mut num_blocks_buf)?;
        let num_blocks = u64::from_le_bytes(num_blocks_buf);
        
        let mut chunks = Vec::with_capacity(num_blocks as usize);
        
        for _ in 0..num_blocks {
            let mut row_count_buf = [0u8; 8];
            file.read_exact(&mut row_count_buf)?;
            let row_count = u64::from_le_bytes(row_count_buf) as usize;
            
            let mut columns = Vec::with_capacity(schema.fields.len());
            
            for field in &schema.fields {
                if version >= 2 {
                    // Read Stats (v2+)
                    let mut stats_len_buf = [0u8; 8];
                    file.read_exact(&mut stats_len_buf)?;
                    let stats_len = u64::from_le_bytes(stats_len_buf);
                    let mut stats_json = vec![0u8; stats_len as usize];
                    file.read_exact(&mut stats_json)?;
                    // Statistics can be used for pushdown optimization here
                }

                let mut dtype_buf = [0u8; 1];
                file.read_exact(&mut dtype_buf)?;
                let dtype_byte = dtype_buf[0];
                
                let mut data_size_buf = [0u8; 8];
                file.read_exact(&mut data_size_buf)?;
                let data_size = u64::from_le_bytes(data_size_buf) as usize;
                
                let mut data_raw = vec![0u8; data_size];
                file.read_exact(&mut data_raw)?;
                
                let column_data = match dtype_byte {
                    1 => {
                        let vec: Vec<f64> = unsafe {
                            std::slice::from_raw_parts(data_raw.as_ptr() as *const f64, data_size / 8).to_vec()
                        };
                        ColumnData::F64(Arc::new(vec))
                    }
                    2 => {
                        let vec: Vec<i64> = unsafe {
                            std::slice::from_raw_parts(data_raw.as_ptr() as *const i64, data_size / 8).to_vec()
                        };
                        ColumnData::I64(Arc::new(vec))
                    }
                    3 => {
                        let vec: Vec<bool> = data_raw.iter().map(|&b| b != 0).collect();
                        ColumnData::Bool(Arc::new(vec))
                    }
                    4 => {
                        let mut cursor = 0;
                        let mut offset_count_buf = [0u8; 8];
                        offset_count_buf.copy_from_slice(&data_raw[0..8]);
                        let offset_count = u64::from_le_bytes(offset_count_buf) as usize;
                        cursor += 8;
                        
                        let offsets: Vec<usize> = unsafe {
                            std::slice::from_raw_parts(data_raw[cursor..cursor + offset_count * 8].as_ptr() as *const usize, offset_count).to_vec()
                        };
                        cursor += offset_count * 8;
                        
                        let str_data = data_raw[cursor..].to_vec();
                        ColumnData::Str { data: Arc::new(str_data), offsets: Arc::new(offsets) }
                    }
                    _ => ColumnData::Bool(Arc::new(vec![])),
                };
                
                let mut val_size_buf = [0u8; 8];
                file.read_exact(&mut val_size_buf)?;
                let val_size = u64::from_le_bytes(val_size_buf) as usize;
                let validity = if val_size > 0 {
                    let mut val_raw = vec![0u8; val_size];
                    file.read_exact(&mut val_raw)?;
                    Some(Bitmap { data: Arc::new(val_raw), len: row_count })
                } else {
                    None
                };
                
                columns.push(Column::new(field.name.clone(), column_data, validity));
            }
            chunks.push(DataChunk::new(columns, row_count));
        }
        
        Ok((schema, chunks))
    }
}
