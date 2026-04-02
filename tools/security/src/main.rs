//! Nyx Security Scanner
//!
//! A comprehensive security scanning tool for Nyx packages and source code.
//! Features:
//! - Dependency vulnerability scanning
//! - Known vulnerability pattern detection
//! - CWE (Common Weakness Enumeration) detection
//! - Secret detection
//! - Package integrity verification

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod database;
mod scanner;

use database::VulnerabilityDatabase;
use scanner::{SecurityReport, SecurityScanner, Severity, Vulnerability};

/// Security scanner commands
#[derive(Debug, Subcommand)]
enum ScanCommand {
    /// Scan a project for security issues
    Scan {
        /// Path to project (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output format (json, text)
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Scan for vulnerabilities in dependencies
    Dependencies {
        /// Path to project
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Scan for secrets (API keys, passwords, etc.)
    Secrets {
        /// Path to scan
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Update vulnerability database
    UpdateDb,
    /// Show vulnerability database info
    DbInfo,
}

#[derive(Debug, Parser)]
#[command(name = "nyx-security", about = "Nyx Security Scanner")]
struct Args {
    #[command(subcommand)]
    command: ScanCommand,
}

fn main() {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    match args.command {
        ScanCommand::Scan { path, format } => run_scan(path, format),
        ScanCommand::Dependencies { path } => scan_dependencies(path),
        ScanCommand::Secrets { path } => scan_secrets(path),
        ScanCommand::UpdateDb => update_database(),
        ScanCommand::DbInfo => show_db_info(),
    }
}

/// Run a full security scan
fn run_scan(path: PathBuf, format: String) {
    println!("═══════════════════════════════════════════════════════════");
    println!("                    Nyx Security Scanner");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    if !path.exists() {
        eprintln!("Error: Path does not exist: {}", path.display());
        return;
    }

    println!("Scanning: {}", path.display());
    println!();

    // Initialize scanner
    let scanner = SecurityScanner::new();

    // Run scan
    match scanner.scan_project(&path) {
        Ok(report) => {
            // Print results
            if format == "json" {
                let json = serde_json::to_string_pretty(&report).unwrap();
                println!("{}", json);
            } else {
                print_report(&report);
            }

            // Exit with error code if vulnerabilities found
            if !report.vulnerabilities.is_empty() {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error during scan: {}", e);
            std::process::exit(1);
        }
    }
}

/// Scan dependencies for vulnerabilities
fn scan_dependencies(path: PathBuf) {
    println!("Scanning dependencies for: {}", path.display());
    println!();

    let scanner = SecurityScanner::new();

    match scanner.scan_dependencies(&path) {
        Ok(vulns) => {
            if vulns.is_empty() {
                println!("No dependency vulnerabilities found.");
            } else {
                println!("Found {} dependency vulnerabilities:", vulns.len());
                println!();
                for vuln in &vulns {
                    print_vulnerability(vuln);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}

/// Scan for secrets
fn scan_secrets(path: PathBuf) {
    println!("Scanning for secrets in: {}", path.display());
    println!();

    let scanner = SecurityScanner::new();

    match scanner.scan_secrets(&path) {
        Ok(secrets) => {
            if secrets.is_empty() {
                println!("No secrets detected.");
            } else {
                println!("Found {} potential secrets:", secrets.len());
                println!();
                for secret in &secrets {
                    println!("  - {} at {}:{}", secret.pattern, secret.file, secret.line);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}

/// Update the vulnerability database
fn update_database() {
    println!("Updating vulnerability database...");

    let db = VulnerabilityDatabase::new();
    match db.update() {
        Ok(_) => {
            println!("Database updated successfully.");
        }
        Err(e) => {
            eprintln!("Failed to update database: {}", e);
        }
    }
}

/// Show database info
fn show_db_info() {
    let db = VulnerabilityDatabase::new();
    let info = db.get_info();

    println!("Vulnerability Database Information");
    println!("====================================");
    println!("Database version: {}", info.version);
    println!("Last updated: {}", info.last_updated);
    println!("Total vulnerabilities: {}", info.total_vulnerabilities);
    println!("CWE entries: {}", info.cwe_count);

    if info.total_vulnerabilities > 0 {
        println!();
        println!("Database Entries");
        println!("================");

        for entry in db.all() {
            let details = db.lookup(&entry.cwe_id).unwrap_or(entry);
            println!(
                "{} - {} ({})",
                details.cwe_id, details.title, details.severity
            );
            println!("Description: {}", details.description);
            println!(
                "Affected Versions: {}",
                details.affected_versions.join(", ")
            );
            println!("Recommendation: {}", details.recommendation);
            println!();
        }
    }
}

/// Print a security report
fn print_report(report: &SecurityReport) {
    println!("Scan Summary");
    println!("============");
    println!("Files scanned: {}", report.files_scanned);
    println!("Vulnerabilities found: {}", report.vulnerabilities.len());
    println!();

    // Group by severity
    let mut critical = Vec::new();
    let mut high = Vec::new();
    let mut medium = Vec::new();
    let mut low = Vec::new();

    for vuln in &report.vulnerabilities {
        match vuln.severity {
            Severity::Critical => critical.push(vuln),
            Severity::High => high.push(vuln),
            Severity::Medium => medium.push(vuln),
            Severity::Low => low.push(vuln),
        }
    }

    if !critical.is_empty() {
        println!("─── CRITICAL ({}) ───", critical.len());
        for v in &critical {
            print_vulnerability(v);
        }
        println!();
    }

    if !high.is_empty() {
        println!("─── HIGH ({}) ───", high.len());
        for v in &high {
            print_vulnerability(v);
        }
        println!();
    }

    if !medium.is_empty() {
        println!("─── MEDIUM ({}) ───", medium.len());
        for v in &medium {
            print_vulnerability(v);
        }
        println!();
    }

    if !low.is_empty() {
        println!("─── LOW ({}) ───", low.len());
        for v in &low {
            print_vulnerability(v);
        }
        println!();
    }

    if report.vulnerabilities.is_empty() {
        println!("No security issues found. Your code looks good!");
    }
}

/// Print a vulnerability
fn print_vulnerability(v: &Vulnerability) {
    let severity_str = match v.severity {
        Severity::Critical => "CRITICAL",
        Severity::High => "HIGH",
        Severity::Medium => "MEDIUM",
        Severity::Low => "LOW",
    };

    println!("[{}] {}", severity_str, v.title);
    println!("  File: {}:{}", v.file, v.line);
    println!("  CWE: {}", v.cwe_id);
    println!("  Description: {}", v.description);
    if !v.recommendation.is_empty() {
        println!("  Recommendation: {}", v.recommendation);
    }
    println!();
}
