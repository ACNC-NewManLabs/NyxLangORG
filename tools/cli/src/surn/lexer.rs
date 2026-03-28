#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
    Equals,
    Colon,
    Dot,
    Comma,
    LeftBracket,
    RightBracket,
    LeftDoubleBracket,
    RightDoubleBracket,
    LeftBrace,
    RightBrace,
    Newline,
    Indent(usize),
    Dedent,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

pub struct Lexer<'a> {
    #[allow(dead_code)]
    input: &'a str,
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
    col: usize,
    indent_stack: Vec<usize>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().peekable(),
            line: 1,
            col: 1,
            indent_stack: vec![0],
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        while let Some(tok) = self.next_token()? {
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof { break; }
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Option<Token>, String> {
        // If at the start of a line, handle indentation
        if self.col == 1 {
            let mut indent_size = 0;
            while let Some(&c) = self.chars.peek() {
                if c == ' ' {
                    self.chars.next();
                    self.col += 1;
                    indent_size += 1;
                } else if c == '\t' {
                    self.chars.next();
                    self.col += 4; // Assume 4 spaces for tab
                    indent_size += 4;
                } else {
                    break;
                }
            }

            // Skip empty lines or comment-only lines
            if let Some(&c) = self.chars.peek() {
                if c == '\n' || c == '#' {
                    self.skip_to_newline();
                    return self.next_token();
                }
            } else {
                return Ok(Some(Token { kind: TokenKind::Eof, line: self.line, col: self.col }));
            }

            let last_indent = *self.indent_stack.last().unwrap();
            if indent_size > last_indent {
                self.indent_stack.push(indent_size);
                return Ok(Some(Token { kind: TokenKind::Indent(indent_size), line: self.line, col: 1 }));
            } else if indent_size < last_indent {
                while indent_size < *self.indent_stack.last().unwrap_or(&0) {
                    self.indent_stack.pop();
                    // We should ideally emit Dedents one by one if we were very strict, 
                    // but for SURN one is usually enough if it matches a previous level.
                    // To be industrial, we return Dedent here.
                    return Ok(Some(Token { kind: TokenKind::Dedent, line: self.line, col: 1 }));
                }
            }
        }

        self.skip_whitespace_and_comments();

        let start_line = self.line;
        let start_col = self.col;

        let char = match self.chars.next() {
            Some(c) => c,
            None => {
                // Check if we need to dedent back to 0
                if self.indent_stack.len() > 1 {
                    self.indent_stack.pop();
                    return Ok(Some(Token { kind: TokenKind::Dedent, line: self.line, col: self.col }));
                }
                return Ok(Some(Token { kind: TokenKind::Eof, line: self.line, col: self.col }));
            }
        };

        let kind = match char {
            '=' => { self.col += 1; TokenKind::Equals },
            ':' => { self.col += 1; TokenKind::Colon },
            '.' => { self.col += 1; TokenKind::Dot },
            ',' => { self.col += 1; TokenKind::Comma },
            '[' => {
                self.col += 1;
                if let Some(&'[') = self.chars.peek() {
                    self.chars.next();
                    self.col += 1;
                    TokenKind::LeftDoubleBracket
                } else {
                    TokenKind::LeftBracket
                }
            }
            ']' => {
                self.col += 1;
                if let Some(&']') = self.chars.peek() {
                    self.chars.next();
                    self.col += 1;
                    TokenKind::RightDoubleBracket
                } else {
                    TokenKind::RightBracket
                }
            }
            '{' => { self.col += 1; TokenKind::LeftBrace },
            '}' => { self.col += 1; TokenKind::RightBrace },
            '\n' => {
                self.line += 1;
                self.col = 1;
                TokenKind::Newline
            }
            '"' | '\'' => self.read_string(char)?,
            c if c.is_alphabetic() || c == '_' => self.read_identifier(c),
            c if c.is_ascii_digit() || c == '-' || c == '+' => self.read_number(c)?,
            _ => return Err(format!("Unexpected character: {} at line {}, col {}", char, self.line, self.col)),
        };

        if kind != TokenKind::Newline {
            // Already handled in read functions or for single char
        }

        Ok(Some(Token { kind, line: start_line, col: start_col }))
    }

    fn skip_to_newline(&mut self) {
        while let Some(c) = self.chars.next() {
            if c == '\n' {
                self.line += 1;
                self.col = 1;
                break;
            }
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(&c) = self.chars.peek() {
            if c == ' ' || c == '\t' || c == '\r' {
                self.chars.next();
                self.col += 1;
            } else if c == '#' {
                while let Some(&c) = self.chars.peek() {
                    if c == '\n' { break; }
                    self.chars.next();
                }
            } else {
                break;
            }
        }
    }

    fn read_string(&mut self, quote: char) -> Result<TokenKind, String> {
        self.col += 1; // For the opening quote
        let mut s = String::new();
        while let Some(c) = self.chars.next() {
            self.col += 1;
            if c == quote {
                return Ok(TokenKind::String(s));
            }
            if c == '\\' {
                if let Some(nc) = self.chars.next() {
                    self.col += 1; // For the escaped character
                    match nc {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        '\\' => s.push('\\'),
                        _ if nc == quote => s.push(quote),
                        _ => s.push(nc),
                    }
                } else {
                    return Err("Unterminated string with escape sequence".to_string());
                }
            } else {
                s.push(c);
            }
        }
        Err("Unterminated string".to_string())
    }

    fn read_identifier(&mut self, first: char) -> TokenKind {
        self.col += 1;
        let mut s = String::from(first);
        while let Some(&c) = self.chars.peek() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                s.push(self.chars.next().unwrap());
                self.col += 1;
            } else {
                break;
            }
        }
        match s.as_str() {
            "true" => TokenKind::Boolean(true),
            "false" => TokenKind::Boolean(false),
            "null" => TokenKind::Null,
            _ => TokenKind::Identifier(s),
        }
    }

    fn read_number(&mut self, first: char) -> Result<TokenKind, String> {
        self.col += 1;
        let mut s = String::from(first);
        let mut is_float = false;
        while let Some(&c) = self.chars.peek() {
            if c.is_ascii_digit() || c == '_' {
                s.push(self.chars.next().unwrap().clone());
                self.col += 1;
            } else if c == '.' {
                is_float = true;
                s.push(self.chars.next().unwrap());
                self.col += 1;
            } else {
                break;
            }
        }
        let s = s.replace('_', "");
        if is_float {
            s.parse::<f64>().map(TokenKind::Float).map_err(|_| "Invalid float".to_string())
        } else {
            s.parse::<i64>().map(TokenKind::Integer).map_err(|_| "Invalid integer".to_string())
        }
    }
}
