use crate::surn::ast::*;
use crate::surn::lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    input: String,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, input: String) -> Self {
        Self { tokens, pos: 0, input }
    }

    pub fn parse(&mut self) -> Result<Document, String> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
             if let Some(stmt) = self.parse_statement()? {
                 statements.push(stmt);
             }
             self.consume_optional_newlines();
        }
        Ok(Document { statements })
    }

    fn parse_statement(&mut self) -> Result<Option<Statement>, String> {
        self.consume_optional_newlines();
        if self.is_at_end() { return Ok(None); }

        match self.peek().kind {
            TokenKind::LeftBracket => Ok(Some(self.parse_table(false)?)),
            TokenKind::LeftDoubleBracket => Ok(Some(self.parse_table(true)?)),
            TokenKind::Identifier(_) => {
                if self.peek_next().kind == TokenKind::Equals {
                    Ok(Some(self.parse_assignment()?))
                } else if self.peek_next().kind == TokenKind::Colon {
                    Ok(Some(self.parse_block()?))
                } else {
                    // Could be a solo identifier in some modes, but here it's an error
                    let tok = self.peek();
                    Err(format!("Expected '=' or ':' after identifier '{}' at line {}", self.consume_identifier()?, tok.line))
                }
            }
            _ => {
                let tok = self.peek();
                let err = crate::surn::diagnostics::Diagnostic::error(&self.input, &tok, &format!("Unexpected token {:?}", tok.kind));
                Err(err)
            }
        }
    }

    fn parse_assignment(&mut self) -> Result<Statement, String> {
        let key = self.consume_identifier()?;
        self.consume_optional_newlines();
        let op = self.advance();
        if op.kind != TokenKind::Equals && op.kind != TokenKind::Colon {
            return Err(format!("Expected '=' or ':' after identifier '{}' at line {}", key, op.line));
        }
        self.consume_optional_newlines();
        let value = self.parse_value()?;
        Ok(Statement::Assignment { key, value })
    }

    fn parse_table(&mut self, double: bool) -> Result<Statement, String> {
        if double {
            self.consume(TokenKind::LeftDoubleBracket, "Expected '[['")?;
        } else {
            self.consume(TokenKind::LeftBracket, "Expected '['")?;
        }
        
        let mut header = Vec::new();
        header.push(self.consume_identifier()?);
        while self.match_token(TokenKind::Dot) {
            header.push(self.consume_identifier()?);
        }

        if double {
            self.consume(TokenKind::RightDoubleBracket, "Expected ']]'")?;
        } else {
            self.consume(TokenKind::RightBracket, "Expected ']'")?;
        }
        self.consume_optional_newlines();
        
        // Assignments belonging to this table
        let mut assignments = Vec::new();
        while !self.is_at_end() && 
              self.peek().kind != TokenKind::LeftBracket && 
              self.peek().kind != TokenKind::LeftDoubleBracket 
        {
             self.consume_optional_newlines();
             if self.is_at_end() || matches!(self.peek().kind, TokenKind::LeftBracket) || matches!(self.peek().kind, TokenKind::LeftDoubleBracket) {
                 break;
             }

             // In a table, rows are either assignments or blocks. For simplicity, assume assignments.
             // SURN allows identifiers. We parse anything that looks like an assignment.
             if matches!(self.peek().kind, TokenKind::Identifier(_)) {
                 assignments.push(self.parse_assignment()?);
             } else {
                 // If there's something else we don't understand, break out to let the outer loop handle it
                 break;
             }
             self.consume_optional_newlines();
        }

        Ok(Statement::Table { header, assignments, is_double: double })
    }

    fn parse_block(&mut self) -> Result<Statement, String> {
        let key = self.consume_identifier()?;
        self.consume(TokenKind::Colon, "Expected ':'")?;
        
        // Check for single-line assignment: key: value
        if self.peek().kind != TokenKind::Newline && self.peek().kind != TokenKind::Indent(0) {
             // In SURN, key: val is syntactically an assignment
             let value = self.parse_value()?;
             return Ok(Statement::Assignment { key, value });
        }

        self.consume_optional_newlines();
        
        // Strict indentation check
        if let TokenKind::Indent(_) = self.peek().kind {
            self.advance();
        } else {
             return Err(format!("Expected indentation after block parent '{}' at line {}", key, self.peek().line));
        }

        let mut children = Vec::new();
        while !self.is_at_end() && self.peek().kind != TokenKind::Dedent {
            if let Some(stmt) = self.parse_statement()? {
                children.push(stmt);
            }
            self.consume_optional_newlines();
        }

        if !self.is_at_end() {
            self.consume(TokenKind::Dedent, "Expected Dedent at end of block")?;
        }

        Ok(Statement::Block { key, children })
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::String(s) => Ok(Value::String(s)),
            TokenKind::Integer(i) => Ok(Value::Integer(i)),
            TokenKind::Float(f) => Ok(Value::Float(f)),
            TokenKind::Boolean(b) => Ok(Value::Boolean(b)),
            TokenKind::Null => Ok(Value::Null),
            TokenKind::LeftBracket => self.parse_array(),
            TokenKind::LeftBrace => self.parse_object(),
            _ => Err(format!("Expected value, found {:?} at line {}", tok.kind, tok.line)),
        }
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        let mut elements = Vec::new();
        self.consume_optional_newlines();
        if self.peek().kind != TokenKind::RightBracket {
            loop {
                self.consume_optional_newlines();
                elements.push(self.parse_value()?);
                self.consume_optional_newlines();
                if !self.match_token(TokenKind::Comma) { break; }
            }
        }
        self.consume_optional_newlines();
        self.consume(TokenKind::RightBracket, "Expected ']'")?;
        Ok(Value::Array(elements))
    }

    fn parse_object(&mut self) -> Result<Value, String> {
        let mut map = std::collections::HashMap::new();
        self.consume_optional_newlines();
        if self.peek().kind != TokenKind::RightBrace {
            loop {
                self.consume_optional_newlines();
                let key = self.consume_identifier()?;
                self.consume_optional_newlines();
                let op = self.advance();
                if op.kind != TokenKind::Colon && op.kind != TokenKind::Equals {
                    return Err(format!("Expected ':' or '=' after key '{}' inside object at line {}", key, op.line));
                }
                self.consume_optional_newlines();
                let value = self.parse_value()?;
                map.insert(key, value);
                self.consume_optional_newlines();
                if !self.match_token(TokenKind::Comma) { break; }
            }
        }
        self.consume_optional_newlines();
        self.consume(TokenKind::RightBrace, "Expected '}'")?;
        Ok(Value::Object(map))
    }

    // Helpers
    fn advance(&mut self) -> Token {
        if !self.is_at_end() { self.pos += 1; }
        self.tokens[self.pos - 1].clone()
    }

    fn peek(&self) -> Token {
        self.tokens[self.pos].clone()
    }

    fn peek_next(&self) -> Token {
        let mut n = self.pos + 1;
        while n < self.tokens.len() && self.tokens[n].kind == TokenKind::Newline {
            n += 1;
        }
        if n >= self.tokens.len() {
            return self.tokens[self.tokens.len() - 1].clone();
        }
        self.tokens[n].clone()
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, kind: TokenKind) -> bool {
        if self.is_at_end() { return false; }
        // Simplistic kind comparison (ignores values in enum variants for simplicity here)
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(&kind)
    }
    fn consume(&mut self, kind: TokenKind, msg: &str) -> Result<Token, String> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            let tok = self.peek();
            let err = crate::surn::diagnostics::Diagnostic::error(&self.input, &tok, msg);
            Err(err)
        }
    }

    fn consume_identifier(&mut self) -> Result<String, String> {
        let tok = self.advance();
        if let TokenKind::Identifier(s) = tok.kind {
            Ok(s)
        } else {
            Err(format!("Expected identifier at line {}", tok.line))
        }
    }

    fn consume_optional_newlines(&mut self) {
        while !self.is_at_end() {
             let k = &self.peek().kind;
             if *k == TokenKind::Newline {
                 self.advance();
             } else {
                 break;
             }
        }
    }
}
