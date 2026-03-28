//! Global Symbol Index for Nyx LSP
//! 
//! Stores top-level symbols across the entire workspace to enable
//! cross-file navigation and completions.

use std::collections::HashMap;
pub use lsp_types::{Location, SymbolKind, Url};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub description: Option<String>,
    /// If this is a struct, these are its fields (name, type)
    pub fields: Option<Vec<(String, String)>>,
}

pub struct GlobalIndex {
    /// Maps symbol names to their definitions
    symbols: HashMap<String, Vec<Symbol>>,
    /// Maps URIs to the symbols defined in that file (for incremental updates)
    file_to_symbols: HashMap<Url, Vec<String>>,
}

impl GlobalIndex {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            file_to_symbols: HashMap::new(),
        }
    }

    /// Add or update symbols for a file
    pub fn update_file(&mut self, uri: Url, new_symbols: Vec<Symbol>) {
        // Remove old symbols for this file
        if let Some(old_names) = self.file_to_symbols.remove(&uri) {
            for name in old_names {
                if let Some(symbol_list) = self.symbols.get_mut(&name) {
                    symbol_list.retain(|s| s.location.uri != uri);
                }
            }
        }

        // Add new symbols
        let mut names = Vec::new();
        for symbol in new_symbols {
            names.push(symbol.name.clone());
            self.symbols.entry(symbol.name.clone()).or_default().push(symbol);
        }
        self.file_to_symbols.insert(uri, names);
    }

    /// Find all symbols matching a name
    pub fn find(&self, name: &str) -> Vec<Symbol> {
        self.symbols.get(name).cloned().unwrap_or_default()
    }

    /// List all global symbols for completion
    pub fn all_symbols(&self) -> Vec<Symbol> {
        self.symbols.values().flatten().cloned().collect()
    }
    
    /// Clear index for a specific URI
    #[allow(dead_code)]
    pub fn clear_file(&mut self, uri: &Url) {
         if let Some(names) = self.file_to_symbols.remove(uri) {
            for name in names {
                if let Some(symbol_list) = self.symbols.get_mut(&name) {
                    symbol_list.retain(|s| s.location.uri != *uri);
                }
            }
        }
    }
}
