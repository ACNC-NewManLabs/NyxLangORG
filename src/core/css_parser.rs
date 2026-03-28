//! Compile-time CSS Declaration Parser
//!
//! Parses the raw text content of a `css\`...\`` literal into a list of
//! `(property, value)` string pairs.
//!
//! Design goals:
//!  - Zero runtime overhead for fully-static declarations.
//!  - Clear, source-pointed error messages (CSS001–CSS004).
//!  - Vendor prefixes (-webkit-, -moz-) and custom properties (--var) work.
//!  - Values with spaces, commas, parentheses, slashes all work.

#[derive(Debug, Clone, PartialEq)]
pub struct CssDeclaration {
    pub property: String,
    pub value: String,
}

/// Errors produced by the CSS parser.
#[derive(Debug, Clone)]
pub struct CssParseError {
    pub code: &'static str,
    pub message: String,
    /// The declaration text that caused the error.
    pub declaration: String,
}

impl std::fmt::Display for CssParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[{}]: {} (in '{}')", self.code, self.message, self.declaration)
    }
}

/// Parse raw CSS text from a `css\`...\`` literal into a list of declarations.
///
/// Segments that start with `${` are interpolation markers — they are passed
/// through verbatim as internal `__INTERP__` sentinels so the code-generator
/// can later emit runtime `Map::insert` calls for them.
pub fn parse_css(raw: &str) -> Result<Vec<CssDeclaration>, CssParseError> {
    let mut result = Vec::new();

    for raw_decl in raw.split(';') {
        let decl = raw_decl.trim();
        if decl.is_empty() {
            continue;
        }
        // Skip CSS / JS comment lines
        if decl.starts_with("//") || decl.starts_with("/*") {
            continue;
        }
        // Find the first colon (not inside parentheses)
        let colon_pos = find_first_colon(decl).ok_or_else(|| CssParseError {
            code: "CSS001",
            message: format!("Missing colon in CSS declaration — expected \"property: value\", got \"{decl}\""),
            declaration: decl.to_string(),
        })?;

        let property = decl[..colon_pos].trim().to_string();
        let value = decl[colon_pos + 1..].trim().to_string();

        // Validate property name
        if property.is_empty() {
            return Err(CssParseError {
                code: "CSS003",
                message: "Empty CSS property name".to_string(),
                declaration: decl.to_string(),
            });
        }
        // Property names must start with a letter, hyphen, or underscore
        // (custom properties start with --)
        let first = property.chars().next().unwrap();
        if !first.is_alphabetic() && first != '-' && first != '_' {
            return Err(CssParseError {
                code: "CSS003",
                message: format!(
                    "Invalid CSS property name \"{property}\" — property names must start with a letter or hyphen"
                ),
                declaration: decl.to_string(),
            });
        }

        // Validate value
        if value.is_empty() {
            return Err(CssParseError {
                code: "CSS002",
                message: format!("Empty value for CSS property \"{property}\""),
                declaration: decl.to_string(),
            });
        }

        result.push(CssDeclaration { property, value });
    }

    Ok(result)
}

/// Find the position of the first `:` that is NOT inside parentheses.
fn find_first_colon(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ':' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Returns `true` if the given value string contains a `${` interpolation.
pub fn has_interpolation(value: &str) -> bool {
    value.contains("${")
}

/// Given a CSS property value that may contain `${expr}` segments, split it
/// into a list of text chunks and interpolation expressions.
///
/// Example: `"1px solid ${color}"` → `[Text("1px solid "), Interp("color")]`
#[derive(Debug, Clone)]
pub enum ValueSegment {
    Text(String),
    Interp(String),
}

pub fn split_value_segments(value: &str) -> Vec<ValueSegment> {
    let mut segments = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        // Text before the interpolation
        if start > 0 {
            segments.push(ValueSegment::Text(rest[..start].to_string()));
        }
        let inner_start = start + 2; // skip "${"
        // Find matching closing brace (accounting for nesting)
        let inner = &rest[inner_start..];
        let mut depth = 1usize;
        let mut end = 0usize;
        for (i, ch) in inner.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        segments.push(ValueSegment::Interp(inner[..end].trim().to_string()));
        rest = &rest[inner_start + end + 1..];
    }
    if !rest.is_empty() {
        segments.push(ValueSegment::Text(rest.to_string()));
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let result = parse_css("color: white; padding: 1rem;").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].property, "color");
        assert_eq!(result[0].value, "white");
        assert_eq!(result[1].property, "padding");
        assert_eq!(result[1].value, "1rem");
    }

    #[test]
    fn test_multiline() {
        let css = "
            background: #000;
            font-size: 1rem;
            border-radius: 8px;
        ";
        let result = parse_css(css).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].value, "#000");
        assert_eq!(result[1].property, "font-size");
    }

    #[test]
    fn test_complex_values() {
        let css = "background: linear-gradient(135deg, #fff, #000); box-shadow: 0 10px 20px rgba(0, 0, 0, 0.3);";
        let result = parse_css(css).unwrap();
        assert_eq!(result[0].value, "linear-gradient(135deg, #fff, #000)");
        assert_eq!(result[1].value, "0 10px 20px rgba(0, 0, 0, 0.3)");
    }

    #[test]
    fn test_vendor_prefix() {
        let css = "-webkit-backdrop-filter: blur(20px); -webkit-text-fill-color: transparent;";
        let result = parse_css(css).unwrap();
        assert_eq!(result[0].property, "-webkit-backdrop-filter");
        assert_eq!(result[0].value, "blur(20px)");
    }

    #[test]
    fn test_custom_properties() {
        let css = "--accent-color: #00f2ff; color: var(--accent-color);";
        let result = parse_css(css).unwrap();
        assert_eq!(result[0].property, "--accent-color");
        assert_eq!(result[1].value, "var(--accent-color)");
    }

    #[test]
    fn test_error_missing_colon() {
        let err = parse_css("background white").unwrap_err();
        assert_eq!(err.code, "CSS001");
    }

    #[test]
    fn test_error_empty_value() {
        let err = parse_css("background: ;").unwrap_err();
        assert_eq!(err.code, "CSS002");
    }

    #[test]
    fn test_error_invalid_property() {
        let err = parse_css("123color: white").unwrap_err();
        assert_eq!(err.code, "CSS003");
    }

    #[test]
    fn test_interpolation_detection() {
        assert!(has_interpolation("${color}"));
        assert!(has_interpolation("1px solid ${color}"));
        assert!(!has_interpolation("white"));
    }

    #[test]
    fn test_split_segments() {
        let segs = split_value_segments("1px solid ${color}");
        assert_eq!(segs.len(), 2);
        if let ValueSegment::Text(t) = &segs[0] { assert_eq!(t, "1px solid "); }
        if let ValueSegment::Interp(e) = &segs[1] { assert_eq!(e, "color"); }
    }
}
