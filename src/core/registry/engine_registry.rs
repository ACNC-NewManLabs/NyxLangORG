use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineDescriptor {
    pub name: String,
    pub path: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EngineRegistry {
    pub engines: Vec<EngineDescriptor>,
}

impl EngineRegistry {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::load_from_str(&text)
    }

    pub fn load_from_str(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    pub fn discover(&self) -> Vec<EngineDescriptor> {
        self.engines.clone()
    }
}
