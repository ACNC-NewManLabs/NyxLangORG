use crate::surn::lexer::Token;

pub struct Diagnostic;

impl Diagnostic {
    pub fn error(input: &str, token: &Token, msg: &str) -> String {
        let lines: Vec<&str> = input.lines().collect();
        let line_idx = if token.line > 0 { token.line - 1 } else { 0 };
        
        let line_content = lines.get(line_idx).unwrap_or(&"");
        let col = token.col;
        
        let mut error = format!("Error at line {}, col {}: {}\n", token.line, token.col, msg);
        error.push_str(line_content);
        error.push('\n');
        
        for _ in 0..(col.saturating_sub(1)) {
            error.push(' ');
        }
        error.push_str("^\n");
        
        error
    }
}
