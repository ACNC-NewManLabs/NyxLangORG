use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Parser)]
#[command(name = "nyx-collab")]
struct Args {
    #[arg(long, env = "NYX_COLLAB_URL", default_value = "http://127.0.0.1:8092")]
    base_url: String,
    #[arg(long, env = "NYX_COLLAB_OPT_IN", default_value_t = false)]
    opt_in: bool,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    EcosystemInsights,
    HelpSolve {
        #[arg(long)]
        query: String,
    },
    DiscoverProjects {
        #[arg(long)]
        skill: Option<String>,
        #[arg(long)]
        tag: Option<String>,
    },
    Contribute {
        #[arg(long)]
        alias: String,
        #[arg(long)]
        skills: String,
        #[arg(long)]
        contact: String,
    },
    ShareInsight {
        #[arg(long)]
        insight_type: String,
        #[arg(long)]
        pattern_file: PathBuf,
        #[arg(long)]
        recommendation: String,
        #[arg(long)]
        impact: f64,
    },
    PublishKnowledge {
        #[arg(long)]
        kind: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body_file: PathBuf,
        #[arg(long)]
        tags: String,
        #[arg(long)]
        author: String,
    },
    Analytics,
}

#[derive(Debug, Deserialize)]
struct InsightsResponse {
    top_insights: Vec<InsightItem>,
}

#[derive(Debug, Deserialize)]
struct InsightItem {
    insight_type: String,
    recommendation: String,
    impact_score: f64,
}

#[derive(Debug, Deserialize)]
struct SolveResponse {
    solutions: Vec<SolveItem>,
}

#[derive(Debug, Deserialize)]
struct SolveItem {
    title: String,
    solution: String,
    confidence: f64,
}

#[derive(Debug, Deserialize)]
struct DiscoverResponse {
    projects: Vec<ProjectItem>,
    contributors: Vec<ContributorItem>,
}

#[derive(Debug, Deserialize)]
struct ProjectItem {
    name: String,
    summary: String,
    tags: Vec<String>,
    maintainer_alias: String,
}

#[derive(Debug, Deserialize)]
struct ContributorItem {
    alias: String,
    skills: Vec<String>,
    contact_hint: String,
}

#[derive(Debug, Serialize)]
struct SubmitInsightRequest {
    insight_type: String,
    pattern: String,
    recommendation: String,
    impact_score: f64,
    anonymous_id: String,
}

#[derive(Debug, Serialize)]
struct ContributeRequest {
    alias: String,
    skills: Vec<String>,
    contact_hint: String,
}

#[derive(Debug, Serialize)]
struct PublishKnowledgeRequest {
    kind: String,
    title: String,
    body: String,
    tags: Vec<String>,
    author_alias: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let client = reqwest::blocking::Client::new();

    match args.cmd {
        Command::EcosystemInsights => {
            let url = format!("{}/api/v1/ecosystem/insights", trim_base(&args.base_url));
            let resp = client.get(url).send().map_err(|e| e.to_string())?;
            ensure_ok(resp.status())?;
            let data: InsightsResponse = resp.json().map_err(|e| e.to_string())?;
            for item in data.top_insights {
                println!(
                    "[{}] impact={:.2} {}",
                    item.insight_type, item.impact_score, item.recommendation
                );
            }
        }
        Command::HelpSolve { query } => {
            let url = format!("{}/api/v1/help/solve?q={}", trim_base(&args.base_url), query);
            let resp = client.get(url).send().map_err(|e| e.to_string())?;
            ensure_ok(resp.status())?;
            let data: SolveResponse = resp.json().map_err(|e| e.to_string())?;
            for s in data.solutions {
                println!("{:.2} {} -> {}", s.confidence, s.title, s.solution);
            }
        }
        Command::DiscoverProjects { skill, tag } => {
            let mut url = format!("{}/api/v1/discover/projects", trim_base(&args.base_url));
            let mut qs = Vec::new();
            if let Some(s) = skill {
                qs.push(format!("skill={s}"));
            }
            if let Some(t) = tag {
                qs.push(format!("tag={t}"));
            }
            if !qs.is_empty() {
                url.push('?');
                url.push_str(&qs.join("&"));
            }
            let resp = client.get(url).send().map_err(|e| e.to_string())?;
            ensure_ok(resp.status())?;
            let data: DiscoverResponse = resp.json().map_err(|e| e.to_string())?;
            println!("projects:");
            for p in data.projects {
                println!("- {} [{}] by {}: {}", p.name, p.tags.join(","), p.maintainer_alias, p.summary);
            }
            println!("contributors:");
            for c in data.contributors {
                println!("- {} [{}] contact={} ", c.alias, c.skills.join(","), c.contact_hint);
            }
        }
        Command::Contribute {
            alias,
            skills,
            contact,
        } => {
            let req = ContributeRequest {
                alias,
                skills: split_csv(&skills),
                contact_hint: contact,
            };
            let url = format!("{}/api/v1/contribute", trim_base(&args.base_url));
            let resp = client.post(url).json(&req).send().map_err(|e| e.to_string())?;
            ensure_ok(resp.status())?;
            println!("contributor profile submitted");
        }
        Command::ShareInsight {
            insight_type,
            pattern_file,
            recommendation,
            impact,
        } => {
            if !args.opt_in {
                return Err("sharing insights requires --opt-in or NYX_COLLAB_OPT_IN=true".to_string());
            }
            let raw = fs::read_to_string(pattern_file).map_err(|e| e.to_string())?;
            let anon_id = anonymous_id()?;
            let req = SubmitInsightRequest {
                insight_type,
                pattern: anonymize_pattern(&raw),
                recommendation,
                impact_score: impact,
                anonymous_id: anon_id,
            };
            let url = format!("{}/api/v1/insights/submit", trim_base(&args.base_url));
            let resp = client.post(url).json(&req).send().map_err(|e| e.to_string())?;
            ensure_ok(resp.status())?;
            println!("insight shared (anonymized)");
        }
        Command::PublishKnowledge {
            kind,
            title,
            body_file,
            tags,
            author,
        } => {
            let body = fs::read_to_string(body_file).map_err(|e| e.to_string())?;
            let req = PublishKnowledgeRequest {
                kind,
                title,
                body,
                tags: split_csv(&tags),
                author_alias: author,
            };
            let url = format!("{}/api/v1/knowledge/publish", trim_base(&args.base_url));
            let resp = client.post(url).json(&req).send().map_err(|e| e.to_string())?;
            ensure_ok(resp.status())?;
            println!("knowledge resource published");
        }
        Command::Analytics => {
            let url = format!("{}/api/v1/analytics/summary", trim_base(&args.base_url));
            let resp = client.get(url).send().map_err(|e| e.to_string())?;
            ensure_ok(resp.status())?;
            let value: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?);
        }
    }

    Ok(())
}

fn trim_base(input: &str) -> &str {
    input.trim_end_matches('/')
}

fn ensure_ok(status: reqwest::StatusCode) -> Result<(), String> {
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("request failed: {status}"))
    }
}

fn split_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn anonymize_pattern(raw: &str) -> String {
    let normalized = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| l.replace(|c: char| c.is_ascii_alphabetic(), "x"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("pattern:{}", hex::encode(hasher.finalize()))
}

fn anonymous_id() -> Result<String, String> {
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "nyx-dev".to_string());
    let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let mut hasher = Sha256::new();
    hasher.update(host.as_bytes());
    hasher.update(b":");
    hasher.update(user.as_bytes());
    let id = hex::encode(hasher.finalize());
    Ok(format!("anon-{}", &id[..16]))
}
