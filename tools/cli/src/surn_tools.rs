use crate::surn::{Lexer, Parser, Serializer};
use std::fs;

pub fn surn_fmt(input_path: &str) -> Result<(), String> {
    let content = fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let mut lexer = Lexer::new(&content);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens, content.to_string());
    let doc = parser.parse()?;
    
    let formatted = Serializer::serialize(&doc);
    fs::write(input_path, formatted).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn surn_convert(input_path: &str, _to_format: &str) -> Result<String, String> {
    let content = fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let mut lexer = Lexer::new(&content);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens, content.to_string());
    let doc = parser.parse()?;
    
    // For now, only JSON conversion is simulated
    let json = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    Ok(json)
}
