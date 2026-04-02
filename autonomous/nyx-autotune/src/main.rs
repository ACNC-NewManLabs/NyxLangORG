use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Parser)]
#[command(name = "nyx-autotune", about = "Autonomous optimization layer for Nyx")]
struct Args {
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Debug, Subcommand)]
enum CommandKind {
    Analyze {
        #[arg(long, default_value = "engines")]
        path: PathBuf,
    },
    OptimizeAuto {
        #[arg(long, default_value = "engines")]
        path: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    Profile {
        #[arg(required = true, trailing_var_arg = true)]
        cmd: Vec<String>,
    },
    BuildOptimize {
        #[arg(long)]
        execute: bool,
        #[arg(trailing_var_arg = true)]
        cmd: Vec<String>,
    },
    EcosystemHealth,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileMetrics {
    path: String,
    lines: usize,
    functions: usize,
    lets: usize,
    max_nesting: usize,
    binary_ops: usize,
    hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnalysisReport {
    generated_at: String,
    analyzed_files: usize,
    total_lines: usize,
    average_lines: f64,
    high_complexity_files: Vec<String>,
    duplicate_groups: Vec<Vec<String>>,
    suggestions: Vec<String>,
    dependency_findings: Vec<String>,
    compatibility_risks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct BuildCache {
    file_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BuildOptimizationReport {
    generated_at: String,
    total_files: usize,
    changed_files: usize,
    recommended_jobs: usize,
    recommended_cache_key: String,
    command: Vec<String>,
    executed: bool,
    duration_ms: Option<u128>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProfileReport {
    generated_at: String,
    command: Vec<String>,
    exit_code: i32,
    wall_time_ms: u128,
    cpu_time_seconds: Option<f64>,
    peak_rss_kb: Option<u64>,
    peak_threads: Option<u64>,
    io_read_bytes: Option<u64>,
    io_write_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HealthReport {
    generated_at: String,
    score: u32,
    checks: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EngineRegistry {
    engines: Vec<EngineDescriptor>,
}

#[derive(Debug, Deserialize)]
struct EngineDescriptor {
    name: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct PackageRegistry {
    packages: Vec<PackageMeta>,
}

#[derive(Debug, Deserialize)]
struct PackageMeta {
    name: String,
    version: String,
    #[serde(default)]
    dependencies: Vec<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let root = args.project_root;
    ensure_autotune_dirs(&root)?;

    match args.command {
        CommandKind::Analyze { path } => {
            let report = analyze_project(&root.join(path), &root)?;
            let out = root.join(".nyx-autotune/reports/analysis_latest.json");
            write_json(&out, &report)?;
            println!("analysis written {}", out.display());
            println!(
                "{} files, {} total lines",
                report.analyzed_files, report.total_lines
            );
            for s in report.suggestions.iter().take(8) {
                println!("- {s}");
            }
        }
        CommandKind::OptimizeAuto { path, apply } => {
            let report = analyze_project(&root.join(path), &root)?;
            let mut actions = Vec::new();
            actions.extend(
                report
                    .suggestions
                    .iter()
                    .map(|s| format!("suggestion: {s}")),
            );
            actions.extend(
                report
                    .dependency_findings
                    .iter()
                    .map(|s| format!("dependency: {s}")),
            );
            actions.extend(
                report
                    .compatibility_risks
                    .iter()
                    .map(|s| format!("compatibility: {s}")),
            );

            let plan_path = root.join(".nyx-autotune/plans/optimization_plan_latest.json");
            write_json(
                &plan_path,
                &serde_json::json!({
                    "generated_at": Utc::now().to_rfc3339(),
                    "actions": actions,
                    "apply_mode": apply,
                    "reversible": true
                }),
            )?;

            if apply {
                let applied = root.join("autonomous/config/optimization_applied.json");
                write_json(
                    &applied,
                    &serde_json::json!({
                        "applied_at": Utc::now().to_rfc3339(),
                        "note": "No source mutation; optimization guidance applied as config only.",
                        "revert": "delete autonomous/config/optimization_applied.json",
                        "actions": actions
                    }),
                )?;
                println!("applied optimization metadata {}", applied.display());
            }

            println!("optimization plan {}", plan_path.display());
        }
        CommandKind::Profile { cmd } => {
            let report = profile_command(&cmd)?;
            let out = root.join(format!(
                ".nyx-autotune/data/profile_{}.json",
                Utc::now().format("%Y%m%d_%H%M%S")
            ));
            write_json(&out, &report)?;
            append_jsonl(
                &root.join(".nyx-autotune/data/profile_history.jsonl"),
                &report,
            )?;
            println!("profile written {}", out.display());
            println!("wall={}ms exit={}", report.wall_time_ms, report.exit_code);
        }
        CommandKind::BuildOptimize { execute, cmd } => {
            let report = build_optimize(&root, execute, cmd)?;
            let out = root.join(".nyx-autotune/reports/build_optimize_latest.json");
            write_json(&out, &report)?;
            append_jsonl(
                &root.join(".nyx-autotune/data/build_history.jsonl"),
                &report,
            )?;
            println!("build optimization report {}", out.display());
            println!(
                "changed_files={} recommended_jobs={} executed={}",
                report.changed_files, report.recommended_jobs, report.executed
            );
        }
        CommandKind::EcosystemHealth => {
            let report = ecosystem_health(&root)?;
            let out = root.join(".nyx-autotune/reports/health_latest.json");
            write_json(&out, &report)?;
            println!("health score={} report={}", report.score, out.display());
            for c in &report.checks {
                println!("- {c}");
            }
            for w in &report.warnings {
                println!("warning: {w}");
            }
        }
    }

    Ok(())
}

fn analyze_project(path: &Path, root: &Path) -> Result<AnalysisReport, String> {
    let mut files = Vec::new();
    collect_nyx_files(path, &mut files)?;

    let mut total_lines = 0usize;
    let mut metrics = Vec::new();
    let mut fingerprints: HashMap<String, Vec<String>> = HashMap::new();

    for f in &files {
        let m = analyze_file(f)?;
        total_lines += m.lines;
        let norm = normalize_file(f)?;
        fingerprints.entry(norm).or_default().push(m.path.clone());
        metrics.push(m);
    }

    let avg = if files.is_empty() {
        0.0
    } else {
        total_lines as f64 / files.len() as f64
    };

    let high_complexity_files = metrics
        .iter()
        .filter(|m| m.max_nesting >= 4 || m.binary_ops > 25)
        .map(|m| m.path.clone())
        .collect::<Vec<_>>();

    let duplicate_groups = fingerprints
        .values()
        .filter(|paths| paths.len() > 1)
        .cloned()
        .collect::<Vec<_>>();

    let mut suggestions = Vec::new();
    if !high_complexity_files.is_empty() {
        suggestions.push("split high-complexity files into smaller modules".to_string());
    }
    if !duplicate_groups.is_empty() {
        suggestions.push("extract duplicated module logic into shared library files".to_string());
    }
    if avg > 120.0 {
        suggestions
            .push("reduce average file size to improve incremental build performance".to_string());
    }
    if suggestions.is_empty() {
        suggestions.push("no major structural issues detected".to_string());
    }

    let dependency_findings = analyze_dependencies(root)?;
    let compatibility_risks = compatibility_risks(root, &files)?;

    Ok(AnalysisReport {
        generated_at: Utc::now().to_rfc3339(),
        analyzed_files: files.len(),
        total_lines,
        average_lines: avg,
        high_complexity_files,
        duplicate_groups,
        suggestions,
        dependency_findings,
        compatibility_risks,
    })
}

fn analyze_file(path: &Path) -> Result<FileMetrics, String> {
    let src = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let lines = src.lines().count();
    let functions = src
        .lines()
        .filter(|l| l.trim_start().starts_with("fn "))
        .count();
    let lets = src
        .lines()
        .filter(|l| l.trim_start().starts_with("let "))
        .count();

    let mut depth = 0usize;
    let mut max_depth = 0usize;
    for ch in src.chars() {
        if ch == '{' {
            depth += 1;
            if depth > max_depth {
                max_depth = depth;
            }
        }
        if ch == '}' {
            depth = depth.saturating_sub(1);
        }
    }

    let binary_ops = src.matches('+').count() + src.matches('*').count() + src.matches('-').count();
    let hash = sha256_hex(src.as_bytes());

    Ok(FileMetrics {
        path: path.display().to_string(),
        lines,
        functions,
        lets,
        max_nesting: max_depth,
        binary_ops,
        hash,
    })
}

fn analyze_dependencies(root: &Path) -> Result<Vec<String>, String> {
    let mut findings = Vec::new();
    let pkg_path = root.join("registry/packages.json");
    if !pkg_path.exists() {
        findings.push("registry/packages.json not found".to_string());
        return Ok(findings);
    }

    let txt = fs::read_to_string(pkg_path).map_err(|e| e.to_string())?;
    let reg: PackageRegistry = serde_json::from_str(&txt).map_err(|e| e.to_string())?;

    let mut names = BTreeSet::new();
    for p in &reg.packages {
        if !names.insert(p.name.clone()) {
            findings.push(format!("duplicate package entry: {}", p.name));
        }
        if p.dependencies.is_empty() && p.version.starts_with("0.") {
            findings.push(format!(
                "{} is pre-1.0 and dependency-light; validate stability guarantees",
                p.name
            ));
        }
    }

    let installed = root.join("package_manager/nyxpkg/installed.json");
    if installed.exists() {
        let s = fs::read_to_string(installed).map_err(|e| e.to_string())?;
        let val: serde_json::Value = serde_json::from_str(&s).map_err(|e| e.to_string())?;
        let installed_count = val
            .get("installed")
            .and_then(|v| v.as_object())
            .map(|m| m.len())
            .unwrap_or(0);
        findings.push(format!("installed dependency count: {installed_count}"));
    }

    if findings.is_empty() {
        findings.push("dependency graph has no immediate redundancy signals".to_string());
    }

    Ok(findings)
}

fn compatibility_risks(root: &Path, files: &[PathBuf]) -> Result<Vec<String>, String> {
    let dep_cfg = root.join("autonomous/config/deprecations.json");
    let deprecated = if dep_cfg.exists() {
        let txt = fs::read_to_string(dep_cfg).map_err(|e| e.to_string())?;
        serde_json::from_str::<Vec<String>>(&txt).map_err(|e| e.to_string())?
    } else {
        vec!["unsafe_api".to_string(), "legacy_thread_spawn".to_string()]
    };

    let mut risks = Vec::new();
    for f in files {
        let src = fs::read_to_string(f).map_err(|e| e.to_string())?;
        for api in &deprecated {
            if src.contains(api) {
                risks.push(format!(
                    "{} uses deprecated API token '{}', recommend migration",
                    f.display(),
                    api
                ));
            }
        }
    }

    let pkg_path = root.join("registry/packages.json");
    if pkg_path.exists() {
        let txt = fs::read_to_string(pkg_path).map_err(|e| e.to_string())?;
        let reg: PackageRegistry = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
        for p in reg.packages {
            if p.version.starts_with("0.") {
                risks.push(format!(
                    "package '{}' is pre-1.0; monitor breaking-change risk",
                    p.name
                ));
            }
        }
    }

    if risks.is_empty() {
        risks.push("no immediate long-term compatibility risks detected".to_string());
    }

    Ok(risks)
}

fn profile_command(cmd: &[String]) -> Result<ProfileReport, String> {
    if cmd.is_empty() {
        return Err("profile requires a command".to_string());
    }

    let mut child = Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| e.to_string())?;

    let start = Instant::now();
    #[cfg(target_os = "linux")]
    let pid = child.id();

    #[cfg(target_os = "linux")]
    let sampler = thread::spawn(move || sample_linux_process(pid));

    let status = child.wait().map_err(|e| e.to_string())?;
    let wall = start.elapsed().as_millis();

    #[cfg(target_os = "linux")]
    let (cpu_time_seconds, peak_rss_kb, peak_threads, io_read_bytes, io_write_bytes) =
        sampler.join().unwrap_or((None, None, None, None, None));

    #[cfg(not(target_os = "linux"))]
    let (cpu_time_seconds, peak_rss_kb, peak_threads, io_read_bytes, io_write_bytes) =
        (None, None, None, None, None);

    Ok(ProfileReport {
        generated_at: Utc::now().to_rfc3339(),
        command: cmd.to_vec(),
        exit_code: status.code().unwrap_or(-1),
        wall_time_ms: wall,
        cpu_time_seconds,
        peak_rss_kb,
        peak_threads,
        io_read_bytes,
        io_write_bytes,
    })
}

#[cfg(target_os = "linux")]
fn sample_linux_process(
    pid: u32,
) -> (
    Option<f64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
) {
    let mut peak_rss_kb = 0u64;
    let mut peak_threads = 0u64;
    let mut start_cpu_ticks: Option<u64> = None;
    let mut end_cpu_ticks: Option<u64> = None;
    let mut start_io: Option<(u64, u64)> = None;
    let mut end_io: Option<(u64, u64)> = None;

    loop {
        let proc_dir = format!("/proc/{pid}");
        if !Path::new(&proc_dir).exists() {
            break;
        }

        if let Ok((rss, threads)) = read_proc_status(pid) {
            peak_rss_kb = peak_rss_kb.max(rss);
            peak_threads = peak_threads.max(threads);
        }
        if let Ok(ticks) = read_proc_cpu_ticks(pid) {
            if start_cpu_ticks.is_none() {
                start_cpu_ticks = Some(ticks);
            }
            end_cpu_ticks = Some(ticks);
        }
        if let Ok(io) = read_proc_io(pid) {
            if start_io.is_none() {
                start_io = Some(io);
            }
            end_io = Some(io);
        }

        thread::sleep(Duration::from_millis(80));
    }

    let cpu_time_seconds = match (start_cpu_ticks, end_cpu_ticks) {
        (Some(s), Some(e)) if e >= s => Some((e - s) as f64 / 100.0),
        _ => None,
    };

    let (io_read_bytes, io_write_bytes) = match (start_io, end_io) {
        (Some((sr, sw)), Some((er, ew))) if er >= sr && ew >= sw => (Some(er - sr), Some(ew - sw)),
        _ => (None, None),
    };

    (
        cpu_time_seconds,
        if peak_rss_kb > 0 {
            Some(peak_rss_kb)
        } else {
            None
        },
        if peak_threads > 0 {
            Some(peak_threads)
        } else {
            None
        },
        io_read_bytes,
        io_write_bytes,
    )
}

#[cfg(target_os = "linux")]
fn read_proc_status(pid: u32) -> Result<(u64, u64), String> {
    let text = fs::read_to_string(format!("/proc/{pid}/status")).map_err(|e| e.to_string())?;
    let mut rss = 0u64;
    let mut threads = 0u64;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("VmRSS:") {
            rss = v
                .split_whitespace()
                .next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
        }
        if let Some(v) = line.strip_prefix("Threads:") {
            threads = v.trim().parse().unwrap_or(0);
        }
    }
    Ok((rss, threads))
}

#[cfg(target_os = "linux")]
fn read_proc_cpu_ticks(pid: u32) -> Result<u64, String> {
    let text = fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|e| e.to_string())?;
    let parts = text.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 16 {
        return Err("/proc stat too short".to_string());
    }
    let utime: u64 = parts[13].parse().unwrap_or(0);
    let stime: u64 = parts[14].parse().unwrap_or(0);
    Ok(utime + stime)
}

#[cfg(target_os = "linux")]
fn read_proc_io(pid: u32) -> Result<(u64, u64), String> {
    let text = fs::read_to_string(format!("/proc/{pid}/io")).map_err(|e| e.to_string())?;
    let mut read_bytes = 0u64;
    let mut write_bytes = 0u64;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("read_bytes:") {
            read_bytes = v.trim().parse().unwrap_or(0);
        }
        if let Some(v) = line.strip_prefix("write_bytes:") {
            write_bytes = v.trim().parse().unwrap_or(0);
        }
    }
    Ok((read_bytes, write_bytes))
}

fn build_optimize(
    root: &Path,
    execute: bool,
    cmd: Vec<String>,
) -> Result<BuildOptimizationReport, String> {
    let mut files = Vec::new();
    collect_nyx_files(&root.join("engines"), &mut files)?;

    let mut new_hashes = BTreeMap::new();
    for f in &files {
        let data = fs::read(f).map_err(|e| e.to_string())?;
        new_hashes.insert(f.display().to_string(), sha256_hex(&data));
    }

    let cache_path = root.join(".nyx-autotune/cache/build_cache.json");
    let old_cache: BuildCache = if cache_path.exists() {
        let txt = fs::read_to_string(&cache_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&txt).map_err(|e| e.to_string())?
    } else {
        BuildCache::default()
    };

    let mut changed = 0usize;
    for (k, v) in &new_hashes {
        if old_cache.file_hashes.get(k) != Some(v) {
            changed += 1;
        }
    }

    let mut hasher = Sha256::new();
    for (k, v) in &new_hashes {
        hasher.update(k.as_bytes());
        hasher.update(v.as_bytes());
    }
    let cache_key = format!("{:x}", hasher.finalize());

    let jobs = std::cmp::max(1, std::cmp::min(num_cpus::get(), 8));

    let command = if cmd.is_empty() {
        vec!["cargo".to_string(), "check".to_string(), "-q".to_string()]
    } else {
        cmd
    };

    let mut duration_ms = None;
    if execute {
        let start = Instant::now();
        let status = Command::new(&command[0])
            .args(&command[1..])
            .current_dir(root)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("build command failed with {status}"));
        }
        duration_ms = Some(start.elapsed().as_millis());
    }

    let cache = BuildCache {
        file_hashes: new_hashes,
    };
    write_json(&cache_path, &cache)?;

    Ok(BuildOptimizationReport {
        generated_at: Utc::now().to_rfc3339(),
        total_files: files.len(),
        changed_files: changed,
        recommended_jobs: jobs,
        recommended_cache_key: cache_key,
        command,
        executed: execute,
        duration_ms,
    })
}

fn ecosystem_health(root: &Path) -> Result<HealthReport, String> {
    let mut score = 100u32;
    let mut checks = Vec::new();
    let mut warnings = Vec::new();

    let engines_path = root.join("registry/engines.json");
    if !engines_path.exists() {
        score = score.saturating_sub(40);
        warnings.push("registry/engines.json missing".to_string());
    } else {
        let txt = fs::read_to_string(&engines_path).map_err(|e| e.to_string())?;
        let reg: EngineRegistry = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
        checks.push(format!("engine count: {}", reg.engines.len()));
        for e in reg.engines {
            let p = root.join(e.path.trim_start_matches("./"));
            if !p.exists() {
                score = score.saturating_sub(5);
                warnings.push(format!("engine path missing: {} ({})", e.name, p.display()));
            }
        }
    }

    let analysis_path = root.join(".nyx-autotune/reports/analysis_latest.json");
    if !analysis_path.exists() {
        score = score.saturating_sub(10);
        warnings.push("analysis report missing; run nyx analyze".to_string());
    } else {
        checks.push("analysis report present".to_string());
    }

    let build_history = root.join(".nyx-autotune/data/build_history.jsonl");
    if !build_history.exists() {
        score = score.saturating_sub(10);
        warnings.push("build history missing; run nyx build optimize".to_string());
    } else {
        checks.push("build optimization history present".to_string());
    }

    let profile_history = root.join(".nyx-autotune/data/profile_history.jsonl");
    if !profile_history.exists() {
        score = score.saturating_sub(10);
        warnings.push("profile history missing; run nyx profile".to_string());
    } else {
        checks.push("profile history present".to_string());
    }

    if warnings.is_empty() {
        checks.push("ecosystem health checks passed".to_string());
    }

    Ok(HealthReport {
        generated_at: Utc::now().to_rfc3339(),
        score,
        checks,
        warnings,
    })
}

fn collect_nyx_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for ent in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let path = ent.path();
        if path.is_dir() {
            if path.ends_with("target") || path.ends_with(".git") || path.ends_with("build") {
                continue;
            }
            collect_nyx_files(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("nyx") {
            out.push(path);
        }
    }
    Ok(())
}

fn normalize_file(path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ensure_autotune_dirs(root: &Path) -> Result<(), String> {
    for p in [
        ".nyx-autotune/reports",
        ".nyx-autotune/data",
        ".nyx-autotune/cache",
        ".nyx-autotune/plans",
    ] {
        fs::create_dir_all(root.join(p)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

fn append_jsonl(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut existing = String::new();
    if path.exists() {
        let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
        f.read_to_string(&mut existing).map_err(|e| e.to_string())?;
    }
    let line = serde_json::to_string(value).map_err(|e| e.to_string())?;
    existing.push_str(&line);
    existing.push('\n');
    fs::write(path, existing).map_err(|e| e.to_string())
}
