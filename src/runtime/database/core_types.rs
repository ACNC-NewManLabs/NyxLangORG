use std::sync::Arc;
use serde::{Serialize, Deserialize};
use rayon::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bitmap {
    pub data: Arc<Vec<u8>>,
    pub len: usize,
}

impl Bitmap {
    pub fn new_all_valid(len: usize) -> Self {
        let byte_len = len.div_ceil(8);
        Self {
            data: Arc::new(vec![0xFF; byte_len]),
            len,
        }
    }

    pub fn get(&self, i: usize) -> bool {
        if i >= self.len { return false; }
        let byte_idx = i / 8;
        let bit_idx = i % 8;
        (self.data[byte_idx] & (1 << bit_idx)) != 0
    }

    pub fn filter(&self, indices: &[usize]) -> Self {
        let byte_len = indices.len().div_ceil(8);
        let mut new_data = vec![0u8; byte_len];
        for (new_idx, &old_idx) in indices.iter().enumerate() {
            if self.get(old_idx) {
                new_data[new_idx / 8] |= 1 << (new_idx % 8);
            }
        }
        Self { data: Arc::new(new_data), len: indices.len() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnData {
    F64(Arc<Vec<f64>>),
    I64(Arc<Vec<i64>>),
    Bool(Arc<Vec<bool>>),
    Bitmap(Bitmap),
    Str {
        data: Arc<Vec<u8>>,
        offsets: Arc<Vec<usize>>, // start index of each string
    },
    Categorical {
        codes: Arc<Vec<u32>>,
        dict: Arc<Vec<String>>,
    },
}

impl ColumnData {
    pub fn get_at(&self, i: usize) -> crate::runtime::execution::nyx_vm::Value {
        use crate::runtime::execution::nyx_vm::Value;
        match self {
            ColumnData::F64(v) => Value::Float(v[i]),
            ColumnData::I64(v) => Value::Int(v[i]),
            ColumnData::Bool(v) => Value::Bool(v[i]),
            ColumnData::Bitmap(bm) => Value::Bool(bm.get(i)),
            ColumnData::Str { data, offsets } => {
                let start = offsets[i];
                let end = offsets[i + 1];
                Value::Str(String::from_utf8_lossy(&data[start..end]).to_string())
            }
            ColumnData::Categorical { codes, dict } => {
                let code = codes[i] as usize;
                Value::Str(dict[code].clone())
            }
        }
    }

    pub fn set_at(&mut self, i: usize, val: crate::runtime::execution::nyx_vm::Value) {
        use crate::runtime::execution::nyx_vm::Value;
        match self {
            ColumnData::F64(v) => {
                let vec = Arc::make_mut(v);
                if let Value::Float(f) = val { vec[i] = f; }
                else if let Value::Int(m) = val { vec[i] = m as f64; }
            }
            ColumnData::I64(v) => {
                let vec = Arc::make_mut(v);
                if let Value::Int(m) = val { vec[i] = m; }
                else if let Value::Float(f) = val { vec[i] = f as i64; }
            }
            ColumnData::Bool(v) => {
                let vec = Arc::make_mut(v);
                if let Value::Bool(b) = val { vec[i] = b; }
            }
            ColumnData::Str { data, offsets } => {
                let arc_data = Arc::make_mut(data);
                let arc_offsets = Arc::make_mut(offsets);
                if let Value::Str(s) = val {
                    let new_bytes = s.as_bytes();
                    let old_start = arc_offsets[i];
                    let old_end = arc_offsets[i + 1];
                    let old_len = old_end - old_start;
                    let new_len = new_bytes.len();
                    
                    if old_len != new_len {
                        let diff = new_len as isize - old_len as isize;
                        if diff > 0 {
                            arc_data.splice(old_end..old_end, std::iter::repeat_n(0, diff as usize));
                            arc_data[old_start..old_start + new_len].copy_from_slice(new_bytes);
                        } else {
                            arc_data.drain(old_start..old_start + (-diff) as usize);
                            arc_data[old_start..old_start + new_len].copy_from_slice(new_bytes);
                        }
                        for offset in arc_offsets.iter_mut().skip(i + 1) {
                            *offset = (*offset as isize + diff) as usize;
                        }
                    } else {
                         arc_data[old_start..old_end].copy_from_slice(new_bytes);
                    }
                }
            }
            ColumnData::Categorical { codes, dict } => {
                let arc_codes = Arc::make_mut(codes);
                let arc_dict = Arc::make_mut(dict);
                if let Value::Str(s) = val {
                    if let Some(pos) = arc_dict.iter().position(|x| x == &s) {
                        arc_codes[i] = pos as u32;
                    } else {
                        arc_dict.push(s.clone());
                        arc_codes[i] = (arc_dict.len() - 1) as u32;
                    }
                }
            }
            ColumnData::Bitmap(_) => {}
        }
    }

    pub fn filter(&self, indices: &[usize]) -> Self {
        self.take(indices)
    }

    pub fn take(&self, indices: &[usize]) -> Self {
        match self {
            ColumnData::F64(v) => {
                let res: Vec<f64> = indices.par_iter().map(|&i| if i == usize::MAX { 0.0 } else { v[i] }).collect();
                ColumnData::F64(Arc::new(res))
            }
            ColumnData::I64(v) => {
                let res: Vec<i64> = indices.par_iter().map(|&i| if i == usize::MAX { 0 } else { v[i] }).collect();
                ColumnData::I64(Arc::new(res))
            }
            ColumnData::Bool(v) => {
                let res: Vec<bool> = indices.par_iter().map(|&i| if i == usize::MAX { false } else { v[i] }).collect();
                ColumnData::Bool(Arc::new(res))
            }
            ColumnData::Bitmap(bm) => {
                let byte_len = indices.len().div_ceil(8);
                let data: Vec<u8> = (0..byte_len).into_par_iter().map(|byte_idx| {
                    let mut byte = 0u8;
                    for bit in 0..8 {
                        let i = byte_idx * 8 + bit;
                        if i < indices.len() {
                            let idx = indices[i];
                            if idx != usize::MAX && bm.get(idx) {
                                byte |= 1 << bit;
                            }
                        }
                    }
                    byte
                }).collect();
                ColumnData::Bitmap(Bitmap { data: Arc::new(data), len: indices.len() })
            }
            ColumnData::Str { data, offsets } => {
                let mut new_data = Vec::new();
                let mut new_offsets = Vec::with_capacity(indices.len() + 1);
                for &i in indices {
                    new_offsets.push(new_data.len());
                    if i != usize::MAX {
                        let start = offsets[i];
                        let end = if i + 1 < offsets.len() { offsets[i+1] } else { data.len() };
                        new_data.extend_from_slice(&data[start..end]);
                    }
                }
                new_offsets.push(new_data.len());
                ColumnData::Str { data: Arc::new(new_data), offsets: Arc::new(new_offsets) }
            }
            ColumnData::Categorical { codes, dict } => {
                let res_codes: Vec<u32> = indices.par_iter().map(|&i| if i == usize::MAX { u32::MAX } else { codes[i] }).collect();
                ColumnData::Categorical { codes: Arc::new(res_codes), dict: dict.clone() }
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColumnMetadata {
    pub null_count: usize,
    pub is_sorted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data: ColumnData,
    pub validity: Option<Bitmap>, // Bit-packed: 1 means valid, 0 means null
    pub metadata: ColumnMetadata,
}

impl Column {
    pub fn new(name: String, data: ColumnData, validity: Option<Bitmap>) -> Self {
        Self {
            name,
            data,
            validity,
            metadata: ColumnMetadata::default(),
        }
    }
    pub fn from_values(name: String, values: Vec<crate::runtime::execution::nyx_vm::Value>) -> Self {
        use crate::runtime::execution::nyx_vm::Value;
        if values.is_empty() {
             return Self::new(name, ColumnData::F64(Arc::new(vec![])), None);
        }
        
        // Detect type from first non-null
        let first = values.iter().find(|v| !matches!(v, Value::Null)).unwrap_or(&Value::Null);
        match first {
            Value::Int(_) => {
                let data: Vec<i64> = values.iter().map(|v| match v { Value::Int(i) => *i, _ => 0 }).collect();
                Self::new(name, ColumnData::I64(Arc::new(data)), None)
            }
            Value::Float(_) => {
                let data: Vec<f64> = values.iter().map(|v| match v { Value::Float(f) => *f, _ => 0.0 }).collect();
                Self::new(name, ColumnData::F64(Arc::new(data)), None)
            }
            Value::Str(_) => {
                let mut data = Vec::new();
                let mut offsets = vec![0];
                for v in &values {
                    if let Value::Str(s) = v {
                        data.extend_from_slice(s.as_bytes());
                    }
                    offsets.push(data.len());
                }
                Self::new(name, ColumnData::Str { data: Arc::new(data), offsets: Arc::new(offsets) }, None)
            }
            _ => {
                let data: Vec<f64> = values.iter().map(|v| match v { Value::Float(f) => *f, _ => 0.0 }).collect();
                Self::new(name, ColumnData::F64(Arc::new(data)), None)
            }
        }
    }

    pub fn new_dummy(len: usize) -> Self {
        Self::new("dummy".to_string(), ColumnData::F64(Arc::new(vec![0.0; len])), None)
    }

    pub fn len(&self) -> usize {
        match &self.data {
            ColumnData::F64(v) => v.len(),
            ColumnData::I64(v) => v.len(),
            ColumnData::Bool(v) => v.len(),
            ColumnData::Bitmap(bm) => bm.len,
            ColumnData::Str { offsets, .. } => {
                if offsets.is_empty() { 0 } else { offsets.len() - 1 }
            }
            ColumnData::Categorical { codes, .. } => codes.len(),
        }
    }

    pub fn estimated_size_bytes(&self) -> usize {
        let base = self.name.len() + 16;
        let data_size = match &self.data {
            ColumnData::F64(v) => v.len() * 8,
            ColumnData::I64(v) => v.len() * 8,
            ColumnData::Bool(v) => v.len(),
            ColumnData::Bitmap(bm) => bm.len.div_ceil(8),
            ColumnData::Str { data, offsets } => data.len() + offsets.len() * 8,
            ColumnData::Categorical { codes, dict } => codes.len() * 4 + dict.iter().map(|s| s.len()).sum::<usize>(),
        };
        let valid_size = self.validity.as_ref().map(|b| b.len / 8).unwrap_or(0);
        base + data_size + valid_size
    }

    pub fn get_value(&self, row: usize) -> crate::runtime::execution::nyx_vm::Value {
        use crate::runtime::execution::nyx_vm::Value;
        
        // Check validity first
        if let Some(validity) = &self.validity {
            if !validity.get(row) {
                return Value::Null;
            }
        }

        match &self.data {
            ColumnData::F64(v) => Value::Float(v[row]),
            ColumnData::I64(v) => Value::Int(v[row]),
            ColumnData::Bool(v) => Value::Bool(v[row]),
            ColumnData::Bitmap(bm) => Value::Bool(bm.get(row)),
            ColumnData::Str { data, offsets } => {
                let start = offsets[row];
                let end = offsets[row + 1];
                Value::Str(String::from_utf8_lossy(&data[start..end]).to_string())
            }
            ColumnData::Categorical { codes, dict } => {
                let code = codes[row] as usize;
                Value::Str(dict[code].clone())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub dtype: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub fields: Vec<Field>,
}

impl Schema {
    pub fn new(fields: Vec<Field>) -> Self {
        Self { fields }
    }
    pub fn from_tuples(tuples: Vec<(String, String)>) -> Self {
        Self {
            fields: tuples.into_iter().map(|(name, dtype)| Field { name, dtype, nullable: true }).collect()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataChunk {
    pub columns: Vec<Column>,
    pub size: usize,
    pub created_at: u64, // Unix timestamp in seconds
}

impl DataChunk {
    pub fn new(columns: Vec<Column>, size: usize) -> Self {
        Self { 
            columns, 
            size,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn estimated_size_bytes(&self) -> usize {
        self.columns.iter().map(|c| c.estimated_size_bytes()).sum()
    }
    pub fn num_rows(&self) -> usize {
        self.size
    }

    pub fn num_cols(&self) -> usize {
        self.columns.len()
    }

    pub fn get_row_key(&self, row_idx: usize, col_idx: usize) -> HashKey {
        HashKey::from_value(&self.columns[col_idx].get_value(row_idx))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum HashKey {
    Null,
    Int(i64),
    Float(u64), // bit-pattern
    Bool(bool),
    Str(String),
}

impl HashKey {
    pub fn from_value(v: &crate::runtime::execution::nyx_vm::Value) -> Self {
        use crate::runtime::execution::nyx_vm::Value;
        match v {
            Value::Null => HashKey::Null,
            Value::Int(i) => HashKey::Int(*i),
            Value::Float(f) => HashKey::Float(f.to_bits()),
            Value::Bool(b) => HashKey::Bool(*b),
            Value::Str(s) => HashKey::Str(s.clone()),
            _ => HashKey::Null,
        }
    }
}
