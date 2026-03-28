use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
    DateTime(String),
    Binary(Vec<u8>),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Statement {
    Assignment { key: String, value: Value },
    Table { header: Vec<String>, assignments: Vec<Statement>, is_double: bool },
    Block { key: String, children: Vec<Statement> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub statements: Vec<Statement>,
}
