use serde::{Deserialize, Serialize};
use crate::surn::{Serializer, Document, Statement, Value as SurnValue};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NyxLock {
    pub version: u32,
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub checksum: String,
    pub dependencies: Vec<String>, // "name version" format for clarity
}

// ---------------------------------------------------------------------------
// NyxLock implementation
// ---------------------------------------------------------------------------

impl NyxLock {
    /// Parse a `.bolt` lockfile.
    ///
    /// The lockfile format is a strict subset of TOML:
    /// - Top-level `version = N` (integer)
    /// - `[[package]]` array-of-tables blocks, each with string/array fields
    pub fn parse(content: &str) -> Result<Self, String> {
        let mut version: u32 = 1;
        let mut packages: Vec<LockedPackage> = Vec::new();
        let mut current: Option<LockedPackage> = None;

        for (lineno, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();
            // Skip comments and blank lines.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // [[package]] / [[package ]] — starts a new package block.
            // The SURN Serializer may add a trailing space before `]]`.
            let is_pkg_table = line.starts_with("[[") && line.ends_with("]]")
                && line[2..line.len() - 2].trim() == "package";
            if is_pkg_table {
                if let Some(pkg) = current.take() {
                    if !pkg.name.is_empty() { packages.push(pkg); }
                }
                current = Some(LockedPackage {
                    name: String::new(),
                    version: String::new(),
                    checksum: String::new(),
                    source: None,
                    dependencies: Vec::new(),
                });
                continue;
            }

            // key = value assignments.
            if let Some(eq) = line.find('=') {
                let key = line[..eq].trim();
                let val_raw = line[eq + 1..].trim();

                // Parse a TOML-like string value (strips surrounding quotes).
                let parse_string = |s: &str| -> String {
                    let s = s.trim();
                    if s.len() >= 2 {
                        let first = s.as_bytes()[0];
                        let last = s.as_bytes()[s.len() - 1];
                        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
                            return s[1..s.len() - 1].to_string();
                        }
                    }
                    s.to_string()
                };

                let parse_array = |s: &str| -> Vec<String> {
                    let s = s.trim();
                    if !s.starts_with('[') || !s.ends_with(']') {
                        return vec![];
                    }
                    let inner = &s[1..s.len() - 1];
                    inner
                        .split(',')
                        .map(|item| {
                            let t = item.trim();
                            if t.len() >= 2 {
                                let first = t.as_bytes()[0];
                                let last = t.as_bytes()[t.len() - 1];
                                if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
                                    return t[1..t.len() - 1].to_string();
                                }
                            }
                            t.to_string()
                        })
                        .filter(|s| !s.is_empty())
                        .collect()
                };

                if let Some(ref mut pkg) = current {
                    match key {
                        "name"     => pkg.name     = parse_string(val_raw),
                        "version"  => pkg.version  = parse_string(val_raw),
                        "checksum" => pkg.checksum = parse_string(val_raw),
                        "source"   => pkg.source   = Some(parse_string(val_raw)),
                        "dependencies" => pkg.dependencies = parse_array(val_raw),
                        _ => {}
                    }
                } else {
                    // Top-level assignment.
                    if key == "version" {
                        version = val_raw.trim().parse().unwrap_or(1);
                    }
                }
            } else {
                // Unrecognised line — ignore for forward-compatibility.
                eprintln!("Warning: unrecognised lockfile line {}: {}", lineno + 1, line);
            }
        }

        // Flush the last block.
        if let Some(pkg) = current {
            if !pkg.name.is_empty() { packages.push(pkg); }
        }

        Ok(NyxLock { version, packages })
    }

    /// Generate a **deterministic** lockfile string.
    ///
    /// Guarantee:
    /// - Packages are sorted alphabetically by name.
    /// - Within each package, fields appear in a fixed canonical order.
    /// - Two calls with equal data always produce byte-identical output.
    pub fn to_string(&self) -> String {
        let mut output = String::new();
        output.push_str("# This file is automatically @generated by NYX.\n");
        output.push_str("# It is not intended for manual editing.\n");
        output.push_str(&format!("version = {}\n\n", self.version));

        // Sort packages by name for canonical, deterministic output.
        let mut sorted: Vec<&LockedPackage> = self.packages.iter().collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));

        let mut statements = Vec::new();
        for pkg in sorted {
            let mut assignments = vec![
                Statement::Assignment {
                    key: "name".to_string(),
                    value: SurnValue::String(pkg.name.clone()),
                },
                Statement::Assignment {
                    key: "version".to_string(),
                    value: SurnValue::String(pkg.version.clone()),
                },
            ];

            if let Some(src) = &pkg.source {
                assignments.push(Statement::Assignment {
                    key: "source".to_string(),
                    value: SurnValue::String(src.clone()),
                });
            }

            assignments.push(Statement::Assignment {
                key: "checksum".to_string(),
                value: SurnValue::String(pkg.checksum.clone()),
            });

            if !pkg.dependencies.is_empty() {
                let mut sorted_deps = pkg.dependencies.clone();
                sorted_deps.sort();
                assignments.push(Statement::Assignment {
                    key: "dependencies".to_string(),
                    value: SurnValue::Array(
                        sorted_deps.into_iter().map(SurnValue::String).collect(),
                    ),
                });
            }

            statements.push(Statement::Table {
                header: vec!["package".to_string()],
                assignments,
                is_double: true,
            });
        }
        output.push_str(&Serializer::serialize(&Document { statements }));
        output
    }

    /// Return whether a specific package+version is locked.
    #[allow(dead_code)]
    pub fn is_locked(&self, name: &str, version: &str) -> bool {
        self.packages.iter().any(|p| p.name == name && p.version == version)
    }

    /// Look up a locked package by name.
    #[allow(dead_code)]
    pub fn get_locked(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|p| p.name == name)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lock() -> NyxLock {
        NyxLock {
            version: 1,
            packages: vec![
                LockedPackage {
                    name: "zebra".to_string(),
                    version: "1.0.0".to_string(),
                    source: Some("registry+https://registry.nyx-lang.org".to_string()),
                    checksum: "blake3:aabbcc".to_string(),
                    dependencies: vec![],
                },
                LockedPackage {
                    name: "apple".to_string(),
                    version: "2.0.0".to_string(),
                    source: Some("registry+https://registry.nyx-lang.org".to_string()),
                    checksum: "blake3:ddeeff".to_string(),
                    dependencies: vec!["zebra 1.0.0".to_string()],
                },
            ],
        }
    }

    #[test]
    fn test_lockfile_serialization_order() {
        let lock = make_lock();
        let s = lock.to_string();
        // apple must appear before zebra (alphabetical).
        let apple_pos = s.find("apple").unwrap();
        let zebra_pos = s.find("zebra").unwrap();
        assert!(apple_pos < zebra_pos, "packages must be sorted alphabetically");
    }

    #[test]
    fn test_lockfile_determinism() {
        let lock = make_lock();
        let s1 = lock.to_string();
        let s2 = lock.to_string();
        assert_eq!(s1, s2, "two calls to to_string() must produce byte-identical output");
    }

    #[test]
    fn test_parse_roundtrip() {
        let lock = make_lock();
        let serialized = lock.to_string();
        let parsed = NyxLock::parse(&serialized).expect("should parse successfully");
        assert_eq!(parsed.packages.len(), 2);
        assert!(parsed.get_locked("apple").is_some());
        assert!(parsed.get_locked("zebra").is_some());
        assert_eq!(parsed.get_locked("apple").unwrap().version, "2.0.0");
    }
}
