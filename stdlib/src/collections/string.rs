//! NYX String Module

use std::string::String as StdString;

/// A UTF-8 string
pub struct String {
    inner: StdString,
}

impl String {
    /// Create a new empty string
    pub fn new() -> String {
        String { inner: StdString::new() }
    }

    /// Create from a &str
    pub fn from(s: &str) -> String {
        String { inner: StdString::from(s) }
    }

    /// Get length in bytes
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Push a string slice
    pub fn push_str(&mut self, s: &str) {
        self.inner.push_str(s);
    }

    /// Push a char
    pub fn push(&mut self, c: char) {
        self.inner.push(c);
    }

    /// Clear the string
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Borrow as &str
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Industrial Safe Slice: Returns Result with NyxError on OOB or invalid UTF-8 boundary
    pub fn safe_slice(&self, start: usize, end: usize) -> Result<&str, crate::error::NyxError> {
        if start > end || end > self.inner.len() {
            return Err(crate::error::NyxError::new(
                "STD002",
                format!("String slice out of bounds: {}..{} for length {}", start, end, self.inner.len()),
                crate::error::ErrorCategory::Runtime
            ));
        }
        if !self.inner.is_char_boundary(start) || !self.inner.is_char_boundary(end) {
            return Err(crate::error::NyxError::new(
                "STD003",
                "String slice falls on non-char boundary",
                crate::error::ErrorCategory::Runtime
            ));
        }
        Ok(&self.inner[start..end])
    }

    /// Industrial Expect: Panics with formatted NyxError report
    pub fn expect_slice(&self, start: usize, end: usize) -> &str {
        match self.safe_slice(start, end) {
            Ok(s) => s,
            Err(e) => panic!("\n{}", e),
        }
    }
}

impl std::fmt::Display for String {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::fmt::Debug for String {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl std::hash::Hash for String {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl PartialEq for String {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for String {}

impl Default for String {
    fn default() -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::String;

    #[test]
    fn string_basic_ops() {
        let mut s = String::new();
        assert!(s.is_empty());
        s.push_str("nyx");
        s.push('!');
        assert_eq!(s.len(), 4);
        assert_eq!(s.as_str(), "nyx!");
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn string_from() {
        let s = String::from("hello");
        assert_eq!(s.as_str(), "hello");
    }
}
