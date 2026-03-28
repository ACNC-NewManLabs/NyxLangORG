// Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.
// Nyx Modular Loading System™
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineDescriptor {
    pub name: String,
    pub path: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineRegistry {
    pub engines: Vec<EngineDescriptor>,
}

pub fn load_registry(path: impl AsRef<Path>) -> Result<EngineRegistry, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
