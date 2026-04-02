//! Security Scanner Implementation

use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use nyx::core::ast::ast_nodes::{Expr, ItemKind, Stmt};
use nyx::core::lexer::lexer::Lexer;
use nyx::core::lexer::token::TokenKind;
use nyx::core::parser::grammar_engine::GrammarEngine;
use nyx::core::parser::neuro_parser::NeuroParser;
use nyx::core::registry::language_registry::LanguageRegistry;

/// Vulnerability severity levels
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

/// A detected vulnerability
#[derive(Debug, Clone, serde::Serialize)]
pub struct Vulnerability {
    pub title: String,
    pub severity: Severity,
    pub file: String,
    pub line: usize,
    pub column: Option<usize>,
    pub cwe_id: String,
    pub description: String,
    pub recommendation: String,
    pub code_snippet: String,
}

/// A detected secret
#[derive(Debug, Clone, serde::Serialize)]
pub struct Secret {
    pub pattern: String,
    pub file: String,
    pub line: usize,
    pub value: String,
}

/// Security scan report
#[derive(Debug, Clone, serde::Serialize)]
pub struct SecurityReport {
    pub files_scanned: usize,
    pub vulnerabilities: Vec<Vulnerability>,
    pub secrets: Vec<Secret>,
    pub scan_time_ms: u128,
}

/// Security scanner
pub struct SecurityScanner {
    /// Known vulnerability patterns
    vulnerability_patterns: Vec<VulnerabilityPattern>,
    /// Secret detection patterns
    secret_patterns: Vec<SecretPattern>,
    /// CWE information
    cwe_database: HashMap<&'static str, CweInfo>,
}

/// A vulnerability pattern to detect
struct VulnerabilityPattern {
    pattern: Regex,
    severity: Severity,
    cwe_id: &'static str,
    title: &'static str,
    description: &'static str,
    recommendation: &'static str,
}

/// A secret pattern to detect
struct SecretPattern {
    name: &'static str,
    pattern: Regex,
}

/// CWE information
struct CweInfo {
    name: &'static str,
    description: &'static str,
}

impl SecurityScanner {
    /// Create a new security scanner
    pub fn new() -> Self {
        let vulnerability_patterns = vec![
            // Command Injection
            VulnerabilityPattern {
                pattern: Regex::new(r#"^(system|exec|spawn|shell_exec|popen)$"#).unwrap(),
                severity: Severity::Critical,
                cwe_id: "CWE-78",
                title: "Command Injection",
                description: "Possible command injection vulnerability",
                recommendation:
                    "Avoid using user input in system calls. Use parameterized commands.",
            },
            // Path Traversal
            VulnerabilityPattern {
                pattern: Regex::new(r#"^(read_file|write_file|open)$"#).unwrap(),
                severity: Severity::High,
                cwe_id: "CWE-22",
                title: "Path Traversal",
                description: "Possible path traversal vulnerability",
                recommendation: "Validate and sanitize file paths. Use allowlists.",
            },
            // SQL Injection (for future use)
            VulnerabilityPattern {
                pattern: Regex::new(r#"^(query|execute|exec_sql)$"#).unwrap(),
                severity: Severity::Critical,
                cwe_id: "CWE-89",
                title: "SQL Injection",
                description: "Possible SQL injection vulnerability",
                recommendation: "Use parameterized queries instead of string concatenation.",
            },
            // Weak Cryptography
            VulnerabilityPattern {
                pattern: Regex::new(r#"^(md5|sha1|des|rc4)$"#).unwrap(),
                severity: Severity::Medium,
                cwe_id: "CWE-327",
                title: "Weak Cryptography",
                description: "Weak cryptographic algorithm detected",
                recommendation: "Use strong algorithms like SHA-256, AES, or ChaCha20.",
            },
        ];

        let secret_patterns = vec![
            SecretPattern {
                name: "AWS Key",
                pattern: Regex::new(r#"AKIA[0-9A-Z]{16}"#).unwrap(),
            },
            SecretPattern {
                name: "GitHub Token",
                pattern: Regex::new(r#"gh[pousr]_[A-Za-z0-9_]{36,}"#).unwrap(),
            },
            SecretPattern {
                name: "Generic API Key",
                pattern: Regex::new(r#"(?i)(api[_-]?key|apikey|password|secret|token)\s*[=:]\s*["'][A-Za-z0-9_\-]{8,}["']"#).unwrap(),
            },
            SecretPattern {
                name: "Private Key",
                pattern: Regex::new(r#"-----BEGIN (RSA |EC |DSA )?PRIVATE KEY-----"#).unwrap(),
            },
            SecretPattern {
                name: "JWT Token",
                pattern: Regex::new(r#"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"#).unwrap(),
            },
        ];

        let cwe_database = HashMap::from([
            ("CWE-78", CweInfo {
                name: "OS Command Injection",
                description: "The application constructs part of an OS command using externally-influenced input",
            }),
            ("CWE-22", CweInfo {
                name: "Path Traversal",
                description: "The application uses external input to construct a pathname",
            }),
            ("CWE-798", CweInfo {
                name: "Use of Hard-coded Credentials",
                description: "The program contains embedded credentials",
            }),
            ("CWE-89", CweInfo {
                name: "SQL Injection",
                description: "The application constructs an SQL query using externally-influenced input",
            }),
            ("CWE-327", CweInfo {
                name: "Use of Weak Cryptographic Algorithm",
                description: "The use of a broken or weak cryptographic algorithm",
            }),
            ("CWE-242", CweInfo {
                name: "Use of Inherently Dangerous Function",
                description: "The program calls a potentially dangerous function",
            }),
        ]);

        Self {
            vulnerability_patterns,
            secret_patterns,
            cwe_database,
        }
    }

    /// Scan a project for security issues
    pub fn scan_project(&self, path: &Path) -> Result<SecurityReport, String> {
        let start = std::time::Instant::now();

        let mut files_scanned = 0;
        let mut vulnerabilities = Vec::new();
        let mut secrets = Vec::new();

        // Collect files to scan
        let files = self.collect_files(path)?;

        for file_path in &files {
            // Skip certain files
            if let Some(ext) = file_path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if !["nyx"].contains(&ext.as_str()) {
                    continue;
                }
            }
            files_scanned += 1;

            // Read file
            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // AST based scanning
            self.scan_ast(&content, file_path, &mut vulnerabilities, &mut secrets);
        }

        let scan_time_ms = start.elapsed().as_millis();

        Ok(SecurityReport {
            files_scanned,
            vulnerabilities,
            secrets,
            scan_time_ms,
        })
    }

    fn scan_ast(
        &self,
        content: &str,
        file_path: &Path,
        vulnerabilities: &mut Vec<Vulnerability>,
        secrets: &mut Vec<Secret>,
    ) {
        let registry = LanguageRegistry::default();
        let mut lexer = Lexer::from_source(content.to_string());

        // Before parser: quick check of comments for unsafe api usage or TODOs and tokens for raw string secrets
        if let Ok(tokens) = lexer.tokenize() {
            let file_str = file_path.display().to_string();
            for tok in &tokens {
                // Secrets in strings
                if tok.kind == TokenKind::String {
                    for secret_pattern in &self.secret_patterns {
                        if secret_pattern.pattern.is_match(&tok.lexeme) {
                            secrets.push(Secret {
                                pattern: secret_pattern.name.to_string(),
                                file: file_str.clone(),
                                line: tok.span.start.line,
                                value: "[REDACTED]".to_string(),
                            });
                        }
                    }
                }

                // Comments unsafe api
                if tok.kind == TokenKind::Comment || tok.kind == TokenKind::MultiLineComment {
                    if tok.lexeme.contains("unsafe") {
                        vulnerabilities.push(Vulnerability {
                            title: "Unsafe API Usage".to_string(),
                            severity: Severity::Medium,
                            file: file_str.clone(),
                            line: tok.span.start.line,
                            column: None,
                            cwe_id: "CWE-242".to_string(),
                            description: "Usage of unsafe API detected".to_string(),
                            recommendation: "Review unsafe code and minimize its usage."
                                .to_string(),
                            code_snippet: tok.lexeme.trim().to_string(),
                        });
                    }
                    if Regex::new(r"#\s*TODO.*(?:hack|fixme|bug|issue|problem)")
                        .unwrap()
                        .is_match(&tok.lexeme)
                    {
                        vulnerabilities.push(Vulnerability {
                            title: "Incomplete TODO".to_string(),
                            severity: Severity::Low,
                            file: file_str.clone(),
                            line: tok.span.start.line,
                            column: None,
                            cwe_id: "CWE-1004".to_string(),
                            description: "Incomplete TODO comment suggesting a known issue"
                                .to_string(),
                            recommendation: "Address the TODO or create a tracking issue."
                                .to_string(),
                            code_snippet: tok.lexeme.trim().to_string(),
                        });
                    }
                }
            }

            let grammar = GrammarEngine::from_registry(&registry);
            let mut parser = NeuroParser::new(grammar);

            if let Ok(ast) = parser.parse(&tokens) {
                for item in &ast.items {
                    if let ItemKind::Function(func) = &item.kind {
                        self.scan_stmts(&func.body, file_path, vulnerabilities);
                    }
                }
            }
        }
    }

    fn scan_stmts(
        &self,
        stmts: &[Stmt],
        file_path: &Path,
        vulnerabilities: &mut Vec<Vulnerability>,
    ) {
        for stmt in stmts {
            match stmt {
                Stmt::Expr(expr) => self.scan_expr(expr, file_path, vulnerabilities),
                Stmt::Let { expr, .. } => {
                    self.scan_expr(expr, file_path, vulnerabilities);
                }
                Stmt::If {
                    branches,
                    else_body,
                    ..
                } => {
                    for branch in branches {
                        self.scan_expr(&branch.condition, file_path, vulnerabilities);
                        self.scan_stmts(&branch.body, file_path, vulnerabilities);
                    }
                    if let Some(eb) = else_body {
                        self.scan_stmts(eb, file_path, vulnerabilities);
                    }
                }
                Stmt::While {
                    condition, body, ..
                } => {
                    self.scan_expr(condition, file_path, vulnerabilities);
                    self.scan_stmts(body, file_path, vulnerabilities);
                }
                Stmt::Loop { body, .. } => self.scan_stmts(body, file_path, vulnerabilities),
                Stmt::Return { expr, .. } => {
                    if let Some(val) = expr {
                        self.scan_expr(val, file_path, vulnerabilities);
                    }
                }
                _ => {}
            }
        }
    }

    fn scan_expr(&self, expr: &Expr, file_path: &Path, vulnerabilities: &mut Vec<Vulnerability>) {
        let file_str = file_path.display().to_string();
        match expr {
            Expr::Call { callee, args, .. } => {
                // Check if callee is an identifier
                if let Expr::Identifier { name: ident, .. } = &**callee {
                    for pattern in &self.vulnerability_patterns {
                        if pattern.pattern.is_match(ident) {
                            let description =
                                if let Some(info) = self.cwe_database.get(pattern.cwe_id) {
                                    format!(
                                        "{}: {}. {}",
                                        info.name, info.description, pattern.description
                                    )
                                } else {
                                    pattern.description.to_string()
                                };

                            vulnerabilities.push(Vulnerability {
                                title: pattern.title.to_string(),
                                severity: pattern.severity,
                                file: file_str.clone(),
                                line: 0, // span not available on Expr::Call in this AST
                                column: None,
                                cwe_id: pattern.cwe_id.to_string(),
                                description,
                                recommendation: pattern.recommendation.to_string(),
                                code_snippet: format!("{}(...)", ident),
                            });
                        }
                    }
                }

                for arg in args {
                    self.scan_expr(arg, file_path, vulnerabilities);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.scan_expr(left, file_path, vulnerabilities);
                self.scan_expr(right, file_path, vulnerabilities);
            }
            Expr::Unary { right: inner, .. } => {
                self.scan_expr(inner, file_path, vulnerabilities);
            }
            Expr::Block { stmts: body, .. } => {
                self.scan_stmts(body, file_path, vulnerabilities);
            }
            Expr::IfExpr {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    self.scan_expr(&branch.condition, file_path, vulnerabilities);
                    self.scan_stmts(&branch.body, file_path, vulnerabilities);
                }
                if let Some(eb) = else_body {
                    self.scan_expr(eb, file_path, vulnerabilities);
                }
            }
            Expr::Match {
                expr: mat_expr,
                arms,
                ..
            } => {
                self.scan_expr(mat_expr, file_path, vulnerabilities);
                for arm in arms {
                    match &arm.body {
                        nyx::core::ast::ast_nodes::MatchBody::Expr(e) => {
                            self.scan_expr(e, file_path, vulnerabilities)
                        }
                        nyx::core::ast::ast_nodes::MatchBody::Stmt(s) => {
                            self.scan_stmts(&[s.clone()], file_path, vulnerabilities)
                        }
                        nyx::core::ast::ast_nodes::MatchBody::Block(b) => {
                            self.scan_stmts(b, file_path, vulnerabilities)
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Scan dependencies for vulnerabilities
    pub fn scan_dependencies(&self, path: &Path) -> Result<Vec<Vulnerability>, String> {
        let mut vulnerabilities = Vec::new();

        // Check for Cargo.toml (Rust dependencies)
        let cargo_toml = path.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = fs::read_to_string(&cargo_toml).map_err(|e| e.to_string())?;

            // Extract dependencies (simplified - just check for known vulnerable ones)
            let vulnerable_deps = [
                ("rand 0.7", "CWE-337", "Predictable Seed", "Use rand 0.8+"),
                ("md5", "CWE-327", "Weak Hash", "Use sha2 or blake2"),
                ("sha1", "CWE-327", "Weak Hash", "Use sha2 or blake2"),
            ];

            for (dep, cwe, issue, fix) in vulnerable_deps {
                if content.contains(dep) {
                    vulnerabilities.push(Vulnerability {
                        title: format!("Vulnerable dependency: {}", dep),
                        severity: Severity::High,
                        file: "Cargo.toml".to_string(),
                        line: 1,
                        column: None,
                        cwe_id: cwe.to_string(),
                        description: issue.to_string(),
                        recommendation: fix.to_string(),
                        code_snippet: dep.to_string(),
                    });
                }
            }
        }

        // Check for packages.json
        let packages_json = path.join("registry/packages.json");
        if packages_json.exists() {
            let content = fs::read_to_string(&packages_json).map_err(|e| e.to_string())?;

            // Check for vulnerable packages
            let vulnerable_packages = [(
                "old-package",
                "CWE-1104",
                "Abandoned Package",
                "Use maintained alternative",
            )];

            for (pkg, cwe, issue, fix) in vulnerable_packages {
                if content.contains(pkg) {
                    vulnerabilities.push(Vulnerability {
                        title: format!("Vulnerable package: {}", pkg),
                        severity: Severity::Medium,
                        file: "registry/packages.json".to_string(),
                        line: 1,
                        column: None,
                        cwe_id: cwe.to_string(),
                        description: issue.to_string(),
                        recommendation: fix.to_string(),
                        code_snippet: pkg.to_string(),
                    });
                }
            }
        }

        Ok(vulnerabilities)
    }

    /// Scan for secrets in a path
    pub fn scan_secrets(&self, path: &Path) -> Result<Vec<Secret>, String> {
        let mut secrets = Vec::new();

        let files = self.collect_files(path)?;

        for file_path in &files {
            // Skip certain files
            if let Some(ext) = file_path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if !["nyx", "rs", "toml", "json", "env"].contains(&ext.as_str()) {
                    continue;
                }
            }

            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // We just re-use the file regex patterns specifically for strings in the AST
            let mut vulns = vec![];
            self.scan_ast(&content, file_path, &mut vulns, &mut secrets);
        }

        Ok(secrets)
    }

    /// Collect all files in a directory
    fn collect_files(&self, path: &Path) -> Result<Vec<PathBuf>, String> {
        let mut files = Vec::new();

        if !path.exists() {
            return Err(format!("Path does not exist: {}", path.display()));
        }

        if path.is_file() {
            return Ok(vec![path.to_path_buf()]);
        }

        self.collect_files_recursive(path, &mut files)?;

        Ok(files)
    }

    fn collect_files_recursive(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
        if !dir.is_dir() {
            return Ok(());
        }

        // Skip certain directories
        let dir_name = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        if dir_name.starts_with('.') || dir_name == "target" || dir_name == "node_modules" {
            return Ok(());
        }

        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.is_dir() {
                self.collect_files_recursive(&path, files)?;
            } else {
                files.push(path);
            }
        }

        Ok(())
    }
}

impl Default for SecurityScanner {
    fn default() -> Self {
        Self::new()
    }
}
