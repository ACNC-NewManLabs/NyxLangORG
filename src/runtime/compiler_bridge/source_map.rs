use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub module_id: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourceMapIndex {
    pub entries: Vec<SourceLocation>,
}
