//! Nyx Language Lexer
//!
//! A high-performance, deterministic lexer for the Nyx programming language.
//! Supports UTF-8 source, `//` and `#` comments, all keywords, operators,
//! numeric/string/char literals, and generics punctuation.
//!
//! # Correctness guarantee
//! The lexer stores the source as a `Vec<char>` and uses a single `char_pos`
//! cursor that indexes that vector — never mixing byte offsets with char
//! indices.  The byte offset in `Position` is maintained separately.

use crate::core::diagnostics::{codes, Diagnostic};
use crate::core::lexer::token::{lookup_keyword, Position, Span, Token, TokenKind};

// ─── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LexerError {
    diagnostic: Diagnostic,
}

impl LexerError {
    fn new(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            diagnostic: Diagnostic::error(code, message).with_span(span),
        }
    }

    fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.diagnostic = self.diagnostic.clone().with_suggestion(suggestion);
        self
    }

    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

impl std::fmt::Display for LexerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diagnostic)
    }
}

impl std::error::Error for LexerError {}

// ─── Config ──────────────────────────────────────────────────────────────────

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

// ─── Lexer ───────────────────────────────────────────────────────────────────

/// The Nyx lexer.  Create with `Lexer::from_source(src)` then call
/// `lexer.tokenize()`.
pub struct Lexer {
    /// Raw source (kept for substring slicing)
    source: String,
    /// Source decoded into chars for O(1) random access
    chars: Vec<char>,
    #[allow(dead_code)]
    config: LexerConfig,

    /// Index into `self.chars` — this is the authoritative cursor.
    char_pos: usize,
    /// Byte offset in `self.source` corresponding to `char_pos`.
    byte_pos: usize,

    /// Current (1-based) line number.
    line: usize,
    /// Current (1-based) column number (chars, not bytes).
    column: usize,
}

impl std::fmt::Debug for Lexer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lexer")
            .field("char_pos", &self.char_pos)
            .field("line", &self.line)
            .finish()
    }
}

impl Lexer {
    // ── Constructors ────────────────────────────────────────────────────────

    pub fn new(source: String, config: LexerConfig) -> Self {
        let chars: Vec<char> = source.chars().collect();
        Self {
            source,
            chars,
            config,
            char_pos: 0,
            byte_pos: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn from_source(source: String) -> Self {
        Self::new(source, LexerConfig::default())
    }

    /// Kept for registry-layer compatibility (returns an empty-source lexer).
    pub fn from_registry(
        registry: &crate::core::registry::language_registry::LanguageRegistry,
    ) -> Self {
        let _ = registry;
        Self::from_source(String::new())
    }

    // ── Public API ──────────────────────────────────────────────────────────

    /// Tokenize the entire source and return all meaningful tokens plus EOF.
    /// Whitespace and comments are consumed but not emitted.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexerError> {
        let mut tokens = Vec::with_capacity(512);
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    // ── Core helpers ────────────────────────────────────────────────────────

    fn is_at_end(&self) -> bool {
        self.char_pos >= self.chars.len()
    }

    fn current_position(&self) -> Position {
        Position::new(self.line, self.column, self.byte_pos)
    }

    /// Peek at the current character without consuming it.
    fn peek(&self) -> char {
        self.chars.get(self.char_pos).copied().unwrap_or('\0')
    }

    /// Peek at the character one ahead of current.
    fn peek_next(&self) -> char {
        self.chars.get(self.char_pos + 1).copied().unwrap_or('\0')
    }

    /// Peek two characters ahead.
    fn _peek2(&self) -> char {
        self.chars.get(self.char_pos + 2).copied().unwrap_or('\0')
    }

    /// Consume the current character and update all position tracking.
    fn advance(&mut self) -> char {
        let ch = self.chars[self.char_pos];
        self.char_pos += 1;
        self.byte_pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        ch
    }

    /// Try to match the next char.  If it matches, consume it and return true.
    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    // ── Token creation ───────────────────────────────────────────────────────

    fn make_token(&self, kind: TokenKind, lexeme: String, start: Position) -> Token {
        Token::new(kind, lexeme, Span::new(start, self.current_position()))
    }

    // ── Lexer driver ─────────────────────────────────────────────────────────

    fn next_token(&mut self) -> Result<Token, LexerError> {
        // Drop whitespace
        loop {
            if self.is_at_end() {
                return Ok(Token::eof(self.current_position()));
            }
            let ch = self.peek();
            if ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' {
                self.advance();
            } else {
                break;
            }
        }

        if self.is_at_end() {
            return Ok(Token::eof(self.current_position()));
        }

        let start = self.current_position();
        let ch = self.peek();

        // ── `//` or `#` single-line comments ─────────────────────────────
        if (ch == '/' && self.peek_next() == '/') || ch == '#' {
            self.skip_line_comment();
            return self.next_token(); // tail-call avoidance: just recurse once
        }

        // ── `/* … */` block comments ──────────────────────────────────────
        if ch == '/' && self.peek_next() == '*' {
            self.skip_block_comment(start)?;
            return self.next_token();
        }

        // ── String literal ────────────────────────────────────────────────
        if ch == '"' {
            return self.scan_string(start);
        }
        if ch == 'r' && self.peek_next() == '"' {
            self.advance(); // consume 'r'
            return self.scan_string(start);
        }

        // ── Backtick string (raw / multiline) ─────────────────────────────
        if ch == '`' {
            return self.scan_backtick_string(start);
        }

        // ── Char literal ──────────────────────────────────────────────────
        if ch == '\'' {
            return self.scan_char(start);
        }

        // ── Numeric literal ────────────────────────────────────────────────
        if ch.is_ascii_digit()
            || (ch == '0'
                && (self.peek_next() == 'x' || self.peek_next() == 'b' || self.peek_next() == 'o'))
        {
            return self.scan_number(start);
        }

        // ── Identifier / keyword ───────────────────────────────────────────
        if ch.is_alphabetic() || ch == '_' {
            return Ok(self.scan_identifier(start));
        }

        // ── Punctuation & operators ────────────────────────────────────────
        self.scan_punctuation(start)
    }

    // ── Comment skippers ─────────────────────────────────────────────────────

    fn skip_line_comment(&mut self) {
        // Consume everything up to (but not including) the newline.
        while !self.is_at_end() && self.peek() != '\n' {
            self.advance();
        }
    }

    fn skip_block_comment(&mut self, start: Position) -> Result<(), LexerError> {
        self.advance(); // '/'
        self.advance(); // '*'
        let mut depth = 1usize;
        while depth > 0 {
            if self.is_at_end() {
                return Err(LexerError::new(
                    codes::LEXER_UNTERMINATED_COMMENT,
                    "unterminated block comment",
                    Span::new(start, self.current_position()),
                )
                .with_suggestion("add a closing '*/' to end the block comment"));
            }
            let c = self.advance();
            match c {
                '/' if self.peek() == '*' => {
                    self.advance();
                    depth += 1;
                }
                '*' if self.peek() == '/' => {
                    self.advance();
                    depth -= 1;
                }
                _ => {}
            }
        }
        Ok(())
    }

    // ── String literal ────────────────────────────────────────────────────────

    fn scan_string(&mut self, start: Position) -> Result<Token, LexerError> {
        self.advance(); // opening "
        let mut value = String::new();
        loop {
            if self.is_at_end() {
                return Err(LexerError::new(
                    codes::LEXER_UNTERMINATED_STRING,
                    "unterminated string literal",
                    Span::new(start, self.current_position()),
                )
                .with_suggestion("add a closing '\"' to end the string literal"));
            }
            let c = self.advance();
            match c {
                '"' => break,
                '\n' => {
                    return Err(LexerError::new(
                        codes::LEXER_EOF_IN_STRING,
                        "newline in string literal",
                        Span::new(start, self.current_position()),
                    )
                    .with_suggestion("close the string before the newline or use '\\n'"));
                }
                '\\' => {
                    if self.is_at_end() {
                        return Err(LexerError::new(
                            codes::LEXER_INVALID_ESCAPE,
                            "unterminated escape sequence in string literal",
                            Span::new(start, self.current_position()),
                        )
                        .with_suggestion(
                            "complete the escape sequence (e.g. \\\\n, \\\\t, \\\\u{...})",
                        ));
                    }
                    let esc = self.advance();
                    match esc {
                        'n' => value.push('\n'),
                        't' => value.push('\t'),
                        'r' => value.push('\r'),
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        '\'' => value.push('\''),
                        '0' => value.push('\0'),
                        'u' => {
                            // Unicode escape \u{XXXX}
                            if !self.match_char('{') {
                                return Err(LexerError::new(
                                    codes::LEXER_INVALID_ESCAPE,
                                    "invalid unicode escape: expected '{' after \\u",
                                    Span::new(start, self.current_position()),
                                )
                                .with_suggestion("use a unicode escape like \\u{1F600}"));
                            }
                            let mut hex = String::new();
                            while !self.is_at_end() && self.peek() != '}' {
                                hex.push(self.advance());
                            }
                            if !self.match_char('}') {
                                return Err(LexerError::new(
                                    codes::LEXER_INVALID_ESCAPE,
                                    "unterminated unicode escape",
                                    Span::new(start, self.current_position()),
                                )
                                .with_suggestion("close the unicode escape with '}'"));
                            }
                            if hex.is_empty() {
                                return Err(LexerError::new(
                                    codes::LEXER_INVALID_ESCAPE,
                                    "empty unicode escape",
                                    Span::new(start, self.current_position()),
                                )
                                .with_suggestion(
                                    "provide at least one hex digit inside \\u{...}",
                                ));
                            }
                            let code = u32::from_str_radix(&hex, 16).map_err(|_| {
                                LexerError::new(
                                    codes::LEXER_INVALID_ESCAPE,
                                    format!("invalid unicode escape '\\\\u{{{hex}}}'"),
                                    Span::new(start, self.current_position()),
                                )
                                .with_suggestion("use only hex digits in \\u{...}")
                            })?;
                            let ch = char::from_u32(code).ok_or_else(|| {
                                LexerError::new(
                                    codes::LEXER_INVALID_ESCAPE,
                                    format!("unicode escape out of range '\\\\u{{{hex}}}'"),
                                    Span::new(start, self.current_position()),
                                )
                                .with_suggestion("use a valid Unicode scalar value")
                            })?;
                            value.push(ch);
                        }
                        other => {
                            // Permissive: keep unknown escapes as literal characters (useful for regexes).
                            value.push(other);
                        }
                    }
                }
                c => value.push(c),
            }
        }
        Ok(self.make_token(TokenKind::String, value, start))
    }

    fn scan_backtick_string(&mut self, start: Position) -> Result<Token, LexerError> {
        self.advance(); // opening `
        let mut value = String::new();
        while !self.is_at_end() && self.peek() != '`' {
            value.push(self.advance());
        }
        if self.is_at_end() {
            return Err(LexerError::new(
                codes::LEXER_UNTERMINATED_STRING,
                "unterminated backtick string",
                Span::new(start, self.current_position()),
            )
            .with_suggestion("add a closing '`' to end the raw string"));
        }
        self.advance(); // closing `
        Ok(self.make_token(TokenKind::String, value, start))
    }

    // ── Char literal ──────────────────────────────────────────────────────────

    fn scan_char(&mut self, start: Position) -> Result<Token, LexerError> {
        self.advance(); // opening '
        let ch = if self.peek() == '\\' {
            self.advance();
            let esc = self.advance();
            match esc {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                '\'' => '\'',
                '0' => '\0',
                c => c,
            }
        } else if self.is_at_end() {
            return Err(LexerError::new(
                codes::LEXER_UNTERMINATED_STRING,
                "unterminated char literal",
                Span::new(start, self.current_position()),
            )
            .with_suggestion("add a closing '\\'' to end the character literal"));
        } else {
            self.advance()
        };

        if self.peek() != '\'' {
            return Err(LexerError::new(
                codes::LEXER_MALFORMED_TOKEN,
                "expected closing ' after char literal",
                Span::new(start, self.current_position()),
            )
            .with_suggestion("add a closing '\\'' after the character"));
        }
        self.advance(); // closing '
        Ok(self.make_token(TokenKind::Char, ch.to_string(), start))
    }

    // ── Number literal ────────────────────────────────────────────────────────

    fn scan_number(&mut self, start: Position) -> Result<Token, LexerError> {
        let mut value = String::new();
        let mut is_float = false;

        // Hex / binary / octal prefix
        if self.peek() == '0' {
            match self.peek_next() {
                'x' | 'X' => {
                    value.push(self.advance()); // '0'
                    value.push(self.advance()); // 'x'
                    let mut digits = 0usize;
                    while self.peek().is_ascii_hexdigit() || self.peek() == '_' {
                        let c = self.advance();
                        if c != '_' {
                            value.push(c);
                            digits += 1;
                        }
                    }
                    if digits == 0 {
                        return Err(LexerError::new(
                            codes::LEXER_INVALID_NUMBER,
                            "expected hex digits after 0x",
                            Span::new(start, self.current_position()),
                        )
                        .with_suggestion("add at least one hex digit after 0x"));
                    }
                    return Ok(self.make_token(TokenKind::Integer, value, start));
                }
                'b' | 'B' => {
                    value.push(self.advance());
                    value.push(self.advance());
                    let mut digits = 0usize;
                    while matches!(self.peek(), '0' | '1' | '_') {
                        let c = self.advance();
                        if c != '_' {
                            value.push(c);
                            digits += 1;
                        }
                    }
                    if digits == 0 {
                        return Err(LexerError::new(
                            codes::LEXER_INVALID_NUMBER,
                            "expected binary digits after 0b",
                            Span::new(start, self.current_position()),
                        )
                        .with_suggestion("add at least one binary digit after 0b"));
                    }
                    return Ok(self.make_token(TokenKind::Integer, value, start));
                }
                'o' | 'O' => {
                    value.push(self.advance());
                    value.push(self.advance());
                    let mut digits = 0usize;
                    while matches!(self.peek(), '0'..='7' | '_') {
                        let c = self.advance();
                        if c != '_' {
                            value.push(c);
                            digits += 1;
                        }
                    }
                    if digits == 0 {
                        return Err(LexerError::new(
                            codes::LEXER_INVALID_NUMBER,
                            "expected octal digits after 0o",
                            Span::new(start, self.current_position()),
                        )
                        .with_suggestion("add at least one octal digit after 0o"));
                    }
                    return Ok(self.make_token(TokenKind::Integer, value, start));
                }
                _ => {}
            }
        }

        // Decimal integer part (allow `_` separators)
        while self.peek().is_ascii_digit() || self.peek() == '_' {
            let c = self.advance();
            if c != '_' {
                value.push(c);
            }
        }

        // Optional fractional part
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            value.push(self.advance()); // '.'
            while self.peek().is_ascii_digit() || self.peek() == '_' {
                let c = self.advance();
                if c != '_' {
                    value.push(c);
                }
            }
        }

        // Optional exponent
        if matches!(self.peek(), 'e' | 'E') {
            is_float = true;
            value.push(self.advance());
            if matches!(self.peek(), '+' | '-') {
                value.push(self.advance());
            }
            let mut exp_digits = 0usize;
            while self.peek().is_ascii_digit() {
                value.push(self.advance());
                exp_digits += 1;
            }
            if exp_digits == 0 {
                return Err(LexerError::new(
                    codes::LEXER_INVALID_NUMBER,
                    "expected digits in exponent",
                    Span::new(start, self.current_position()),
                )
                .with_suggestion("add exponent digits, e.g. 1.0e10"));
            }
        }

        // Optional type suffix (i32, u64, f64, etc.) — consume but don't store
        while self.peek().is_alphanumeric() {
            self.advance();
        }

        let kind = if is_float {
            TokenKind::Float
        } else {
            TokenKind::Integer
        };
        Ok(self.make_token(kind, value, start))
    }

    // ── Identifier / keyword ─────────────────────────────────────────────────

    fn scan_identifier(&mut self, start: Position) -> Token {
        let mut value = String::new();
        while !self.is_at_end() && (self.peek().is_alphanumeric() || self.peek() == '_') {
            value.push(self.advance());
        }

        // css` prefix — NO space allowed between 'css' and the backtick.
        if value == "css" && self.peek() == '`' {
            return match self.scan_css_literal(start) {
                Ok(tok) => tok,
                Err(_) => self.make_token(TokenKind::Error, value, start),
            };
        }

        let kind = match value.as_str() {
            "true" | "false" => TokenKind::Boolean,
            "null" => TokenKind::Null,
            _ => lookup_keyword(&value),
        };

        self.make_token(kind, value, start)
    }

    /// Scan a `css\`...\`` template literal.
    ///
    /// Called right after "css" has been consumed and `self.peek() == '\`'`.
    /// The content between the backticks is stored verbatim as the token
    /// lexeme so the VM can parse it into a `Map<string, string>` at runtime.
    fn scan_css_literal(&mut self, start: Position) -> Result<Token, LexerError> {
        self.advance(); // consume opening backtick
        let mut raw = String::new();
        let mut depth = 0usize; // track ${ ... } nesting
        loop {
            if self.is_at_end() {
                return Err(LexerError::new(
                    codes::LEXER_UNTERMINATED_STRING,
                    "unterminated css`` literal",
                    Span::new(start, self.current_position()),
                )
                .with_suggestion("add a closing backtick to end the css literal"));
            }
            let c = self.peek();
            match c {
                // End of css literal (only at depth 0)
                '`' if depth == 0 => {
                    self.advance(); // consume closing backtick
                    break;
                }
                // Start of ${...} interpolation
                '$' if self.chars.get(self.char_pos + 1) == Some(&'{') => {
                    raw.push(self.advance()); // '$'
                    raw.push(self.advance()); // '{'
                    depth += 1;
                }
                // End of ${...} interpolation
                '}' if depth > 0 => {
                    raw.push(self.advance());
                    depth -= 1;
                }
                _ => {
                    raw.push(self.advance());
                }
            }
        }
        Ok(self.make_token(TokenKind::CssLiteral, raw, start))
    }

    // ── Punctuation / operators ───────────────────────────────────────────────

    fn scan_punctuation(&mut self, start: Position) -> Result<Token, LexerError> {
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
            '?' => match self.peek() {
                '?' => {
                    self.advance();
                    TokenKind::QuestionQuestion
                }
                _ => TokenKind::Question,
            },
            '~' => TokenKind::Tilde,
            '@' => TokenKind::At,

            '=' => match self.peek() {
                '=' => {
                    self.advance();
                    TokenKind::EqEq
                }
                '>' => {
                    self.advance();
                    TokenKind::FatArrow
                }
                _ => TokenKind::Equal,
            },

            '!' => {
                if self.match_char('=') {
                    TokenKind::BangEqual
                } else {
                    TokenKind::Bang
                }
            }

            '<' => match self.peek() {
                '=' => {
                    self.advance();
                    TokenKind::LessEqual
                }
                '<' => {
                    self.advance();
                    if self.match_char('=') {
                        TokenKind::LessLessEqual
                    } else {
                        TokenKind::LessLess
                    }
                }
                '-' => {
                    self.advance();
                    TokenKind::ThinArrow
                }
                _ => TokenKind::Less,
            },

            '>' => match self.peek() {
                '=' => {
                    self.advance();
                    TokenKind::GreaterEqual
                }
                '>' => {
                    self.advance();
                    if self.match_char('=') {
                        TokenKind::GreaterGreaterEqual
                    } else {
                        TokenKind::GreaterGreater
                    }
                }
                _ => TokenKind::Greater,
            },

            '&' => match self.peek() {
                '&' => {
                    self.advance();
                    TokenKind::AmpersandAmpersand
                }
                '=' => {
                    self.advance();
                    TokenKind::AmpersandEqual
                }
                _ => TokenKind::Ampersand,
            },

            '|' => match self.peek() {
                '|' => {
                    self.advance();
                    TokenKind::PipePipe
                }
                '=' => {
                    self.advance();
                    TokenKind::PipeEqual
                }
                _ => TokenKind::Pipe,
            },

            '^' => {
                if self.match_char('=') {
                    TokenKind::CaretEqual
                } else {
                    TokenKind::Caret
                }
            }

            '+' => {
                if self.match_char('=') {
                    TokenKind::PlusEqual
                } else {
                    TokenKind::Plus
                }
            }

            '-' => match self.peek() {
                '=' => {
                    self.advance();
                    TokenKind::MinusEqual
                }
                '>' => {
                    self.advance();
                    TokenKind::Arrow
                }
                _ => TokenKind::Minus,
            },

            '*' => {
                if self.match_char('=') {
                    TokenKind::StarEqual
                } else {
                    TokenKind::Star
                }
            }

            '/' => {
                if self.match_char('=') {
                    TokenKind::SlashEqual
                } else {
                    TokenKind::Slash
                }
            }

            '%' => {
                if self.match_char('=') {
                    TokenKind::PercentEqual
                } else {
                    TokenKind::Percent
                }
            }

            ':' => {
                if self.match_char(':') {
                    TokenKind::ColonColon
                } else {
                    TokenKind::Colon
                }
            }

            '.' => {
                if self.peek() == '.' {
                    self.advance();
                    match self.peek() {
                        '=' => {
                            self.advance();
                            TokenKind::DotDotEq
                        }
                        '.' => {
                            self.advance();
                            TokenKind::DotDotDot
                        }
                        _ => TokenKind::DotDot,
                    }
                } else {
                    TokenKind::Dot
                }
            }

            other => {
                return Err(LexerError::new(
                    codes::LEXER_ILLEGAL_CHARACTER,
                    format!("unexpected character '{other}'"),
                    Span::new(start, self.current_position()),
                )
                .with_suggestion("remove the character or replace it with valid syntax"));
            }
        };

        let lexeme = self.source[start.offset..self.byte_pos].to_string();
        Ok(self.make_token(kind, lexeme, start))
    }
}

// ─── Convenience function ─────────────────────────────────────────────────────

/// Tokenize `source` with default settings.  Convenience wrapper.
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexerError> {
    Lexer::from_source(source.to_string()).tokenize()
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(src: &str) -> Vec<Token> {
        Lexer::from_source(src.to_string())
            .tokenize()
            .expect("lexer must not fail")
    }

    #[test]
    fn test_simple_tokens() {
        let tokens = lex("fn main() { let x = 10 }");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwFn));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Identifier && t.lexeme == "main"));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Integer && t.lexeme == "10"));
        assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
    }

    #[test]
    fn test_operators() {
        let tokens = lex("+ - * / == != < > <= >=");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Plus));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::EqEq));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::BangEqual));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::LessEqual));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::GreaterEqual));
    }

    #[test]
    fn test_strings() {
        let tokens = lex(r#""hello world""#);
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::String && t.lexeme == "hello world"));
    }

    #[test]
    fn test_slash_slash_comment() {
        let tokens = lex("// this is a comment\nlet x = 10");
        assert!(!tokens.iter().any(|t| t.kind == TokenKind::Comment));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwLet));
    }

    #[test]
    fn test_hash_comment() {
        let tokens = lex("# hash comment\nlet x = 10");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwLet));
    }

    #[test]
    fn test_multiline_comments() {
        let tokens = lex("/* multi\nline\ncomment */ let x = 10");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwLet));
    }

    #[test]
    fn test_and_or_keywords() {
        let tokens = lex("and or not");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwAnd));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwOr));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::KwNot));
    }

    #[test]
    fn test_generics_tokens() {
        let tokens = lex("List<i32>");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Less));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Greater));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Identifier && t.lexeme == "i32"));
    }

    #[test]
    fn test_path_separator() {
        let tokens = lex("std::io::read");
        assert!(
            tokens
                .iter()
                .filter(|t| t.kind == TokenKind::ColonColon)
                .count()
                == 2
        );
    }

    #[test]
    fn test_float_literal() {
        let tokens = lex("3.14 2.0e-5");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Float));
    }

    #[test]
    fn test_hex_literal() {
        let tokens = lex("0xFF 0b1010 0o77");
        assert_eq!(
            tokens
                .iter()
                .filter(|t| t.kind == TokenKind::Integer)
                .count(),
            3
        );
    }

    #[test]
    fn test_at_token() {
        let tokens = lex("name @ pattern");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::At));
    }

    #[test]
    fn test_arrow_tokens() {
        let tokens = lex("-> =>");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Arrow));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::FatArrow));
    }

    #[test]
    fn test_range_tokens() {
        let tokens = lex("0..10 0..=10");
        assert!(tokens.iter().any(|t| t.kind == TokenKind::DotDot));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::DotDotEq));
    }

    #[test]
    fn test_hello_world_nyx() {
        let src = "fn main() {\nlet x = 10\nprint(x)\n}";
        let tokens = lex(src);
        assert_eq!(tokens[0].kind, TokenKind::KwFn);
        assert!(tokens.iter().any(|t| t.lexeme == "print"));
        assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
    }
}
