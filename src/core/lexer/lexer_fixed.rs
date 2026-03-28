//! Nyx Language Lexer
//!
//! A high-performance, deterministic lexer for the Nyx programming language.
//! Supports UTF-8 source, comments, keywords, operators, and all literals.

use crate::core::lexer::token::{lookup_keyword, Token, TokenKind, Position, Span};

/// Lexer error type
#[derive(Debug, Clone)]
pub struct LexerError {
    pub message: String,
    pub span: Span,
}

impl LexerError {
    pub fn new(message: String, span: Span) -> Self {
        Self { message, span }
    }
}

impl std::fmt::Display for LexerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lexer error at {}:{}: {}", 
            self.span.start.line, self.span.start.column, self.message)
    }
}

impl std::error::Error for LexerError {}

/// Lexer configuration
#[derive(Debug, Clone)]
pub struct LexerConfig {
    pub allow_multiline_comments: bool,
    pub allow_doc_comments: bool,
    pub allow_unicode_identifiers: bool,
}

impl Default for LexerConfig {
    fn default() -> Self {
        Self {
            allow_multiline_comments: true,
            allow_doc_comments: true,
            allow_unicode_identifiers: true,
        }
    }
}

/// High-performance Nyx lexer
#[derive(Debug)]
pub struct Lexer {
    source: String,
    chars: Vec<char>,
    #[allow(dead_code)]
    config: LexerConfig,
    /// Current position
    pos: Position,
    /// Token start position (for span calculation)
    start: Position,
    /// Whether we're at end of input
    at_end: bool,
}

impl Lexer {
    /// Create a new lexer from source code
    pub fn new(source: String, config: LexerConfig) -> Self {
        let chars: Vec<char> = source.chars().collect();
        Self {
            source,
            chars,
            config,
            pos: Position::new(1, 1, 0),
            start: Position::new(1, 1, 0),
            at_end: false,
        }
    }

    /// Create lexer with default config
    pub fn from_source(source: String) -> Self {
        Self::new(source, LexerConfig::default())
    }

    /// Tokenize the entire source and return a vector of tokens
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexerError> {
        let mut tokens = Vec::new();

        while !self.at_end {
            self.skip_whitespace();

            if self.at_end {
                break;
            }

            self.start = self.pos;
            
            match self.scan_token() {
                Ok(Some(token)) => tokens.push(token),
                Ok(None) => {}  // Skip whitespace-only tokens
                Err(e) => return Err(e),
            }
        }

        tokens.push(Token::eof(self.pos));
        Ok(tokens)
    }

    /// Check if at end of input
    fn is_at_end(&self) -> bool {
        self.pos.offset >= self.source.len()
    }

    /// Peek current character without advancing
    fn peek(&self) -> char {
        self.chars.get(self.pos.offset).copied().unwrap_or('\0')
    }

    /// Peek at next character without advancing
    fn peek_next(&self) -> char {
        self.chars.get(self.pos.offset + 1).copied().unwrap_or('\0')
    }

    /// Advance position by one character and return it
    fn advance(&mut self) -> char {
        let ch = self.peek();
        if ch != '\0' {
            self.pos.advance(ch);
        } else {
            self.at_end = true;
        }
        ch
    }

    /// Create a token with current span
    fn make_token(&self, kind: TokenKind, lexeme: String) -> Token {
        Token::new(kind, lexeme, Span::new(self.start, self.pos))
    }

    /// Skip whitespace characters (including newlines)
    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            let ch = self.peek();
            match ch {
                ' ' | '\t' | '\r' | '\n' => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    /// Scan the next token
    fn scan_token(&mut self) -> Result<Option<Token>, LexerError> {
        // Main scanning loop - handles whitespace and comments before each token
        while !self.is_at_end() {
            let start_pos = self.pos;
            let ch = self.peek();

            // Handle whitespace
            if ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' {
                self.advance();
                continue;
            }

            // Single-line comment
            if ch == '/' && self.peek_next() == '/' {
                self.single_line_comment();
                continue;
            }

            // Multi-line comment
            if ch == '/' && self.peek_next() == '*' {
                self.multi_line_comment()?;
                continue;
            }

            // String literals
            if ch == '"' {
                return Ok(Some(self.string_literal()?));
            }

            // Character literal
            if ch == '\'' {
                return Ok(Some(self.char_literal()?));
            }

            // Numbers
            if ch.is_ascii_digit() {
                return Ok(Some(self.number_literal()?));
            }

            // Identifiers and keywords
            if ch.is_ascii_alphabetic() || ch == '_' {
                return Ok(Some(self.identifier_or_keyword()?));
            }

            // Operators and delimiters
            return Ok(Some(self.operator_or_delimiter()?));
        }

        Ok(None)
    }

    /// Handle single-line comments (// ...)
    fn single_line_comment(&mut self) {
        self.advance();
        self.advance();
        
        while self.peek() != '\n' && !self.is_at_end() {
            self.advance();
        }
    }

    /// Handle multi-line comments (/* ... */)
    fn multi_line_comment(&mut self) -> Result<(), LexerError> {
        self.advance(); // consume '/'
        self.advance(); // consume '*'

        let mut depth = 1;
        while depth > 0 && !self.is_at_end() {
            let ch = self.peek();
            if ch == '\0' {
                break;
            }
            
            self.advance(); // consume the character
            
            if ch == '/' && self.peek() == '*' {
                depth += 1;
            } else if ch == '*' && self.peek() == '/' {
                depth -= 1;
            }
        }

        if depth > 0 {
            return Err(LexerError::new(
                "unterminated multi-line comment".to_string(),
                Span::new(self.start, self.pos),
            ));
        }

        Ok(())
    }

    /// Scan string literal
    fn string_literal(&mut self) -> Result<Token, LexerError> {
        self.advance(); // consume opening "
        let mut value = String::new();

        while self.peek() != '"' && !self.is_at_end() {
            let ch = self.advance();
            if ch == '\\' {
                let escaped = self.peek();
                match escaped {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    '\'' => value.push('\''),
                    '0' => value.push('\0'),
                    _ => value.push(escaped),
                }
                self.advance();
            } else if ch == '\n' {
                return Err(LexerError::new(
                    "unterminated string literal".to_string(),
                    Span::new(self.start, self.pos),
                ));
            } else {
                value.push(ch);
            }
        }

        if self.is_at_end() {
            return Err(LexerError::new(
                "unterminated string literal".to_string(),
                Span::new(self.start, self.pos),
            ));
        }

        self.advance(); // consume closing "

        Ok(self.make_token(TokenKind::String, value))
    }

    /// Scan character literal
    fn char_literal(&mut self) -> Result<Token, LexerError> {
        self.advance(); // consume opening '
        
        let ch = if self.peek() == '\\' {
            self.advance();
            match self.peek() {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                '\'' => '\'',
                '0' => '\0',
                c => c,
            }
        } else {
            self.advance()
        };

        if self.peek() != '\'' {
            return Err(LexerError::new(
                "expected closing ' for character literal".to_string(),
                Span::new(self.start, self.pos),
            ));
        }

        self.advance(); // consume closing '

        Ok(self.make_token(TokenKind::Char, ch.to_string()))
    }

    /// Scan number literal (integer or float)
    fn number_literal(&mut self) -> Result<Token, LexerError> {
        let mut value = String::new();
        let mut is_float = false;

        while self.peek().is_ascii_digit() {
            value.push(self.advance());
        }

        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            value.push(self.advance());
            while self.peek().is_ascii_digit() {
                value.push(self.advance());
            }
        }

        if self.peek() == 'e' || self.peek() == 'E' {
            is_float = true;
            value.push(self.advance());
            if self.peek() == '+' || self.peek() == '-' {
                value.push(self.advance());
            }
            while self.peek().is_ascii_digit() {
                value.push(self.advance());
            }
        }

        let kind = if is_float {
            TokenKind::Float
        } else {
            TokenKind::Integer
        };

        Ok(self.make_token(kind, value))
    }

    /// Scan identifier or keyword
    fn identifier_or_keyword(&mut self) -> Result<Token, LexerError> {
        let mut value = String::new();

        while self.peek().is_ascii_alphanumeric() || self.peek() == '_' {
            value.push(self.advance());
        }

        let kind = if value == "true" {
            TokenKind::Boolean
        } else if value == "false" {
            TokenKind::Boolean
        } else if value == "null" {
            TokenKind::Null
        } else {
            lookup_keyword(&value)
        };

        Ok(self.make_token(kind, value))
    }

    /// Scan operator or delimiter
    fn operator_or_delimiter(&mut self) -> Result<Token, LexerError> {
        let ch = self.advance();
        let kind = match ch {
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            '?' => TokenKind::Question,
            '~' => TokenKind::Tilde,
            '=' => self.equals_or_arrow(),
            '!' => self.bang_or_not_equal(),
            '<' => self.less_than(),
            '>' => self.greater_than(),
            '&' => self.ampersand(),
            '|' => self.pipe(),
            '^' => self.caret(),
            '+' => self.plus(),
            '-' => self.minus(),
            '*' => self.star(),
            '/' => self.slash(),
            '%' => self.percent(),
            ':' => self.colon(),
            '.' => self.dot(),
            _ => {
                return Err(LexerError::new(
                    format!("unexpected character '{}'", ch),
                    Span::new(self.start, self.pos),
                ));
            }
        };

        Ok(self.make_token(kind, self.source[self.start.offset..self.pos.offset].to_string()))
    }

    fn equals_or_arrow(&mut self) -> TokenKind {
        match self.peek() {
            '=' => { self.advance(); TokenKind::EqEq }
            '>' => { self.advance(); TokenKind::FatArrow }
            _ => TokenKind::Equal,
        }
    }

    fn bang_or_not_equal(&mut self) -> TokenKind {
        if self.peek() == '=' { self.advance(); TokenKind::BangEqual }
        else { TokenKind::Bang }
    }

    fn less_than(&mut self) -> TokenKind {
        match self.peek() {
            '=' => { self.advance(); TokenKind::LessEqual }
            '<' => { 
                self.advance(); 
                if self.peek() == '=' { self.advance(); TokenKind::LessLessEqual }
                else { TokenKind::LessLess }
            }
            '-' => { self.advance(); TokenKind::ThinArrow }
            ':' => { self.advance(); TokenKind::ColonColon }
            _ => TokenKind::Less,
        }
    }

    fn greater_than(&mut self) -> TokenKind {
        match self.peek() {
            '=' => { self.advance(); TokenKind::GreaterEqual }
            '>' => {
                self.advance();
                if self.peek() == '=' { self.advance(); TokenKind::GreaterGreaterEqual }
                else { TokenKind::GreaterGreater }
            }
            _ => TokenKind::Greater,
        }
    }

    fn ampersand(&mut self) -> TokenKind {
        match self.peek() {
            '&' => { self.advance(); TokenKind::AmpersandAmpersand }
            '=' => { self.advance(); TokenKind::AmpersandEqual }
            _ => TokenKind::Ampersand,
        }
    }

    fn pipe(&mut self) -> TokenKind {
        match self.peek() {
            '|' => { self.advance(); TokenKind::PipePipe }
            '=' => { self.advance(); TokenKind::PipeEqual }
            _ => TokenKind::Pipe,
        }
    }

    fn caret(&mut self) -> TokenKind {
        if self.peek() == '=' { self.advance(); TokenKind::CaretEqual }
        else { TokenKind::Caret }
    }

    fn plus(&mut self) -> TokenKind {
        if self.peek() == '=' { self.advance(); TokenKind::PlusEqual }
        else { TokenKind::Plus }
    }

    fn minus(&mut self) -> TokenKind {
        match self.peek() {
            '=' => { self.advance(); TokenKind::MinusEqual }
            '>' => { self.advance(); TokenKind::Arrow }
            _ => TokenKind::Minus,
        }
    }

    fn star(&mut self) -> TokenKind {
        if self.peek() == '=' { self.advance(); TokenKind::StarEqual }
        else { TokenKind::Star }
    }

    fn slash(&mut self) -> TokenKind {
        if self.peek() == '=' { self.advance(); TokenKind::SlashEqual }
        else { TokenKind::Slash }
    }

    fn percent(&mut self) -> TokenKind {
        if self.peek() == '=' { self.advance(); TokenKind::PercentEqual }
        else { TokenKind::Percent }
    }

    fn colon(&mut self) -> TokenKind {
        if self.peek() == ':' { self.advance(); TokenKind::ColonColon }
        else { TokenKind::Colon }
    }

    fn dot(&mut self) -> TokenKind {
        if self.peek() == '.' {
            self.advance();
            if self.peek() == '=' { self.advance(); TokenKind::DotDotEq }
            else { TokenKind::DotDot }
        } else {
            TokenKind::Dot
        }
    }
    
    /// Create lexer from language registry (for compatibility)
    pub fn from_registry(registry: &crate::core::registry::language_registry::LanguageRegistry) -> Self {
        let _ = registry;
        Self::new(String::new(), LexerConfig::default())
    }
}

/// Convenience function to tokenize source code
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexerError> {
    let mut lexer = Lexer::from_source(source.to_string());
    lexer.tokenize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokens() {
        let source = "fn main() { let x = 10; }";
        let mut lexer = Lexer::from_source(source.to_string());
        let tokens = lexer.tokenize().unwrap();
        
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwFn));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Identifier && t.lexeme == "main"));
    }

    #[test]
    fn test_operators() {
        let source = "+ - * / == != < > <= >=";
        let mut lexer = Lexer::from_source(source.to_string());
        let tokens = lexer.tokenize().unwrap();
        
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Plus));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::EqEq));
    }

    #[test]
    fn test_strings() {
        let source = r#""hello world""#;
        let mut lexer = Lexer::from_source(source.to_string());
        let tokens = lexer.tokenize().unwrap();
        
        assert!(tokens.iter().any(|t| t.kind == TokenKind::String && t.lexeme == "hello world"));
    }

    #[test]
    fn test_comments() {
        let source = "// this is a comment\nlet x = 10;";
        let mut lexer = Lexer::from_source(source.to_string());
        let tokens = lexer.tokenize().unwrap();
        
        assert!(!tokens.iter().any(|t| t.kind == TokenKind::Comment));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwLet));
    }

    #[test]
    fn test_multiline_comments() {
        let source = "/* multi\nline\ncomment */ let x = 10;";
        let mut lexer = Lexer::from_source(source.to_string());
        let tokens = lexer.tokenize().unwrap();
        
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwLet));
    }
}

