//! Nyx Language Server Protocol (LSP) Implementation
//! 
//! Provides IDE integration for Nyx including:
//! - Syntax highlighting (via semantic tokens)
//! - Intelligent autocomplete
//! - Diagnostics (errors/warnings)
//! - Go to definition
//! - Find references
//! - Hover information

use std::collections::HashMap;
use std::sync::Arc;

use lsp_server::{Connection, Message, Request, Response};
use lsp_types::Url;

mod analyzer;
mod completion;
mod index;

use analyzer::DocumentAnalyzer;
use index::GlobalIndex;

fn main() {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Starting Nyx Language Server...");

    // Create connection (stdio)
    let (connection, io_threads) = Connection::stdio();
    
    // Document store
    let documents: Arc<parking_lot::RwLock<HashMap<Url, DocumentAnalyzer>>> = 
        Arc::new(parking_lot::RwLock::new(HashMap::new()));
    
    // Global symbol index
    let index = Arc::new(parking_lot::RwLock::new(GlobalIndex::new()));

    // Main message loop
    for msg in &connection.receiver {
        match msg {
            Message::Request(request) => {
                let documents = documents.clone();
                let index = index.clone();
                let response = handle_request(request, documents, index);
                if let Some(resp) = response {
                    if let Err(e) = connection.sender.send(Message::Response(resp)) {
                        log::error!("Failed to send response: {}", e);
                    }
                }
            }
            Message::Response(_) => {}
            Message::Notification(notification) => {
                let documents = documents.clone();
                let index = index.clone();
                handle_notification(notification, documents, index);
            }
        }
    }

    let _ = io_threads.join();
}

fn handle_request(
    request: Request, 
    documents: Arc<parking_lot::RwLock<HashMap<Url, DocumentAnalyzer>>>,
    index: Arc<parking_lot::RwLock<GlobalIndex>>
) -> Option<Response> {
    let id = request.id.clone();
    
    match request.method.as_str() {
        "textDocument/completion" => {
            let params: lsp_types::CompletionParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(_) => return None,
            };
            let docs = documents.read();
            if let Some(doc) = docs.get(&params.text_document_position.text_document.uri) {
                let index_read = index.read();
                let items = completion::get_completions(doc.source(), params.text_document_position.position, &index_read, doc);
                return Some(Response::new_ok(id, items));
            }
            None
        }
        "textDocument/definition" => {
            let params: lsp_types::TextDocumentPositionParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(_) => return None,
            };
            let docs = documents.read();
            if let Some(doc) = docs.get(&params.text_document.uri) {
                let index_read = index.read();
                if let Some(loc) = doc.find_definition(params.position, &index_read) {
                    return Some(Response::new_ok(id, loc));
                }
            }
            let null: serde_json::Value = serde_json::Value::Null;
            Some(Response::new_ok(id, null))
        }
        "textDocument/references" => {
            let params: lsp_types::TextDocumentPositionParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(_) => return None,
            };
            let docs = documents.read();
            if let Some(doc) = docs.get(&params.text_document.uri) {
                let index_read = index.read();
                let locations = doc.find_references(params.position, &index_read);
                let arr: Vec<serde_json::Value> = locations.into_iter().map(|l| serde_json::to_value(l).unwrap_or(serde_json::Value::Null)).collect();
                return Some(Response::new_ok(id, arr));
            }
            let arr: Vec<serde_json::Value> = vec![];
            Some(Response::new_ok(id, arr))
        }
        "textDocument/hover" => {
            let params: lsp_types::TextDocumentPositionParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(_) => return None,
            };
            let docs = documents.read();
            if let Some(doc) = docs.get(&params.text_document.uri) {
                let index_read = index.read();
                if let Some(hover) = doc.get_hover(params.position, &index_read) {
                    return Some(Response::new_ok(id, hover));
                }
            }
            let null: serde_json::Value = serde_json::Value::Null;
            Some(Response::new_ok(id, null))
        }
        "textDocument/codeAction" => {
            let params: lsp_types::CodeActionParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(_) => return None,
            };
            let docs = documents.read();
            if let Some(doc) = docs.get(&params.text_document.uri) {
                let actions = doc.get_code_actions(params.range);
                let arr: Vec<serde_json::Value> = actions.into_iter().map(|a| serde_json::to_value(a).unwrap_or(serde_json::Value::Null)).collect();
                return Some(Response::new_ok(id, arr));
            }
            let arr: Vec<serde_json::Value> = vec![];
            Some(Response::new_ok(id, arr))
        }
        "textDocument/semanticTokens/full" => {
            let params: lsp_types::SemanticTokensParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(_) => return None,
            };
            let docs = documents.read();
            if let Some(doc) = docs.get(&params.text_document.uri) {
                let tokens = doc.get_semantic_tokens();
                return Some(Response::new_ok(id, tokens));
            }
            None
        }
        "workspace/symbol" => {
            let params: lsp_types::WorkspaceSymbolParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(_) => return None,
            };
            let index_read = index.read();
            let symbols = index_read.all_symbols();
            
            // Filter by query
            let query = params.query.to_lowercase();
            let mut matches = Vec::new();
            for s in symbols {
                if query.is_empty() || s.name.to_lowercase().contains(&query) {
                    matches.push(lsp_types::SymbolInformation {
                        name: s.name.clone(),
                        kind: s.kind,
                        location: s.location.clone(),
                        container_name: None,
                        #[allow(deprecated)]
                        deprecated: None,
                        tags: None,
                    });
                }
            }
            return Some(Response::new_ok(id, matches));
        }
        _ => None
    }
}

fn handle_notification(
    notification: lsp_server::Notification,
    documents: Arc<parking_lot::RwLock<HashMap<Url, DocumentAnalyzer>>>,
    index: Arc<parking_lot::RwLock<GlobalIndex>>
) {
    match notification.method.as_str() {
        "initialized" => {
            log::info!("LSP initialized, starting workspace indexing...");
            spawn_indexer(index);
        }
        "textDocument/didOpen" => {
            let params: lsp_types::DidOpenTextDocumentParams = 
                match serde_json::from_value(notification.params) {
                    Ok(p) => p,
                    Err(_) => return,
                };
            
            let uri = params.text_document.uri;
            let source = params.text_document.text;
            
            log::info!("Opening document: {}", uri);
            
            let mut docs = documents.write();
            let analyzer = DocumentAnalyzer::new(uri.clone(), &source);
            
            // Update index
            let symbols = analyzer.extract_symbols();
            index.write().update_file(uri.clone(), symbols);
            
            docs.insert(uri.clone(), analyzer);
        }
        "textDocument/didChange" => {
            let params: lsp_types::DidChangeTextDocumentParams = 
                match serde_json::from_value(notification.params) {
                    Ok(p) => p,
                    Err(_) => return,
                };
            
            let uri = params.text_document.uri;
            
            if let Some(change) = params.content_changes.into_iter().last() {
                log::info!("Changing document: {}", uri);
                
                let mut docs = documents.write();
                if let Some(doc) = docs.get_mut(&uri) {
                    doc.update(&change.text);
                    // Update index incrementally
                    let symbols = doc.extract_symbols();
                    index.write().update_file(uri.clone(), symbols);
                }
            }
        }
        "textDocument/didSave" => {
            let params: lsp_types::DidSaveTextDocumentParams = 
                match serde_json::from_value(notification.params) {
                    Ok(p) => p,
                    Err(_) => return,
                };
            
            let uri = params.text_document.uri;
            log::info!("Saving document: {}", uri);
            
            let mut docs = documents.write();
            if let Some(doc) = docs.get_mut(&uri) {
                if let Some(text) = params.text {
                    doc.update(&text);
                }
                
                // Update index on save
                let symbols = doc.extract_symbols();
                index.write().update_file(uri.clone(), symbols);

                let diagnostics = doc.get_diagnostics();
                log::info!("Diagnostics: {} issues found", diagnostics.len());
            }
        }
        _ => {}
    }
}

fn spawn_indexer(index: Arc<parking_lot::RwLock<GlobalIndex>>) {
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut files = Vec::new();
        
        let walker = ignore::WalkBuilder::new("./").build();
        for result in walker {
            if let Ok(entry) = result {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    if entry.path().extension().map(|ex| ex == "nyx").unwrap_or(false) {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        }
        
        log::info!("Found {} Nyx files for indexing", files.len());
        
        use rayon::prelude::*;
        let results: Vec<(Url, Vec<index::Symbol>)> = files.into_par_iter().filter_map(|path| {
            let source = std::fs::read_to_string(&path).ok()?;
            let uri = Url::from_file_path(std::fs::canonicalize(&path).ok()?).ok()?;
            let analyzer = DocumentAnalyzer::new(uri.clone(), &source);
            Some((uri, analyzer.extract_symbols()))
        }).collect();
        
        let mut idx = index.write();
        for (uri, symbols) in results {
            idx.update_file(uri, symbols);
        }
        
        log::info!("Workspace indexing complete in {:?}", start.elapsed());
    });
}
