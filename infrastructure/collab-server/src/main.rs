/*
 * Nyx Collaboration Server™ - Global Developer Analytics & Support
 * Copyright (c) 2026 Surya. All rights reserved.
 * PROPRIETARY AND CONFIDENTIAL.
 */

use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Connection>>,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

#[derive(Debug, Deserialize)]
struct SubmitInsightRequest {
    insight_type: String,
    pattern: String,
    recommendation: String,
    impact_score: f64,
    anonymous_id: String,
}

#[derive(Debug, Deserialize)]
struct SolveQuery {
    q: String,
}

#[derive(Debug, Deserialize)]
struct DiscoverQuery {
    skill: Option<String>,
    tag: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContributeRequest {
    alias: String,
    skills: Vec<String>,
    contact_hint: String,
}

#[derive(Debug, Deserialize)]
struct PublishKnowledgeRequest {
    kind: String,
    title: String,
    body: String,
    tags: Vec<String>,
    author_alias: String,
}

#[derive(Debug, Serialize)]
struct InsightItem {
    insight_type: String,
    recommendation: String,
    impact_score: f64,
}

#[derive(Debug, Serialize)]
struct InsightsResponse {
    top_insights: Vec<InsightItem>,
}

#[derive(Debug, Serialize)]
struct SolveItem {
    title: String,
    solution: String,
    confidence: f64,
}

#[derive(Debug, Serialize)]
struct SolveResponse {
    query: String,
    solutions: Vec<SolveItem>,
}

#[derive(Debug, Serialize)]
struct ProjectItem {
    name: String,
    summary: String,
    tags: Vec<String>,
    maintainer_alias: String,
}

#[derive(Debug, Serialize)]
struct DiscoverResponse {
    projects: Vec<ProjectItem>,
    contributors: Vec<ContributorItem>,
}

#[derive(Debug, Serialize)]
struct ContributorItem {
    alias: String,
    skills: Vec<String>,
    contact_hint: String,
}

#[derive(Debug, Serialize)]
struct AnalyticsResponse {
    project_count: i64,
    contributor_count: i64,
    insight_count: i64,
    top_tags: Vec<String>,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let db_path =
        std::env::var("NYX_COLLAB_DB").unwrap_or_else(|_| "infrastructure/schemas/collab.db".to_string());
    let bind_addr = std::env::var("NYX_COLLAB_BIND").unwrap_or_else(|_| "127.0.0.1:8092".to_string());

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    let schema = std::fs::read_to_string("infrastructure/collab-server/schema.sql").map_err(|e| e.to_string())?;
    conn.execute_batch(&schema).map_err(|e| e.to_string())?;
    seed_defaults(&conn)?;

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/api/v1/ecosystem/insights", get(ecosystem_insights))
        .route("/api/v1/insights/submit", post(submit_insight))
        .route("/api/v1/help/solve", get(help_solve))
        .route("/api/v1/discover/projects", get(discover_projects))
        .route("/api/v1/contribute", post(contribute))
        .route("/api/v1/knowledge/publish", post(publish_knowledge))
        .route("/api/v1/analytics/summary", get(analytics_summary))
        .with_state(AppState {
            db: Arc::new(Mutex::new(conn)),
        });

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| format!("bind failed {bind_addr}: {e}"))?;
    println!("nyx collab server listening on http://{bind_addr}");

    axum::serve(listener, app).await.map_err(|e| e.to_string())
}

async fn healthz() -> impl IntoResponse {
    Json(serde_json::json!({"status":"ok"}))
}

async fn ecosystem_insights(
    State(state): State<AppState>,
) -> Result<Json<InsightsResponse>, (StatusCode, Json<ApiError>)> {
    let conn = state.db.lock().map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "db lock poisoned"))?;

    let mut stmt = conn
        .prepare(
            "SELECT insight_type, recommendation, impact_score
             FROM insights ORDER BY impact_score DESC, created_at DESC LIMIT 20",
        )
        .map_err(sql_err)?;

    let rows = stmt
        .query_map([], |r| {
            Ok(InsightItem {
                insight_type: r.get(0)?,
                recommendation: r.get(1)?,
                impact_score: r.get(2)?,
            })
        })
        .map_err(sql_err)?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(sql_err)?);
    }

    Ok(Json(InsightsResponse { top_insights: out }))
}

async fn submit_insight(
    State(state): State<AppState>,
    Json(req): Json<SubmitInsightRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    if req.insight_type.trim().is_empty() || req.pattern.trim().is_empty() || req.recommendation.trim().is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "insight_type/pattern/recommendation required"));
    }
    if !(0.0..=1.0).contains(&req.impact_score) {
        return Err(api_err(StatusCode::BAD_REQUEST, "impact_score must be in [0,1]"));
    }

    let now = Utc::now().to_rfc3339();
    let pattern_hash = sha256_hex(req.pattern.as_bytes());
    let conn = state.db.lock().map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "db lock poisoned"))?;
    conn.execute(
        "INSERT INTO insights (insight_type, pattern_hash, recommendation, impact_score, source_anonymous_id, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![req.insight_type, pattern_hash, req.recommendation, req.impact_score, req.anonymous_id, now],
    )
    .map_err(sql_err)?;

    Ok(Json(serde_json::json!({"accepted": true})))
}

async fn help_solve(
    State(state): State<AppState>,
    Query(q): Query<SolveQuery>,
) -> Result<Json<SolveResponse>, (StatusCode, Json<ApiError>)> {
    // Escape special LIKE characters to prevent SQL injection
    let escaped = q.q.replace('%', "\\%").replace('_', "\\_");
    let like = format!("%{}%", escaped);
    let conn = state.db.lock().map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "db lock poisoned"))?;

    let mut stmt = conn
        .prepare(
            "SELECT title, solution, confidence FROM issues
             WHERE title LIKE ? OR normalized_pattern LIKE ? OR tags LIKE ?
             ORDER BY confidence DESC LIMIT 15",
        )
        .map_err(sql_err)?;

    let rows = stmt
        .query_map(params![like, like, like], |r| {
            Ok(SolveItem {
                title: r.get(0)?,
                solution: r.get(1)?,
                confidence: r.get(2)?,
            })
        })
        .map_err(sql_err)?;

    let mut solutions = Vec::new();
    for row in rows {
        solutions.push(row.map_err(sql_err)?);
    }

    Ok(Json(SolveResponse {
        query: q.q,
        solutions,
    }))
}

async fn discover_projects(
    State(state): State<AppState>,
    Query(q): Query<DiscoverQuery>,
) -> Result<Json<DiscoverResponse>, (StatusCode, Json<ApiError>)> {
    let skill = q.skill.unwrap_or_default();
    let tag = q.tag.unwrap_or_default();
    let like_skill = format!("%{}%", skill);
    let like_tag = format!("%{}%", tag);

    let conn = state.db.lock().map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "db lock poisoned"))?;

    let mut p_stmt = conn
        .prepare(
            "SELECT name, summary, tags, maintainer_alias FROM projects
             WHERE (? = '' OR tags LIKE ?)
             ORDER BY created_at DESC LIMIT 30",
        )
        .map_err(sql_err)?;
    let p_rows = p_stmt
        .query_map(params![tag, like_tag], |r| {
            let tags: String = r.get(2)?;
            Ok(ProjectItem {
                name: r.get(0)?,
                summary: r.get(1)?,
                tags: split_csv(&tags),
                maintainer_alias: r.get(3)?,
            })
        })
        .map_err(sql_err)?;
    let mut projects = Vec::new();
    for row in p_rows {
        projects.push(row.map_err(sql_err)?);
    }

    let mut c_stmt = conn
        .prepare(
            "SELECT alias, skills, contact_hint FROM contributors
             WHERE (? = '' OR skills LIKE ?)
             ORDER BY created_at DESC LIMIT 30",
        )
        .map_err(sql_err)?;
    let c_rows = c_stmt
        .query_map(params![skill, like_skill], |r| {
            let skills: String = r.get(1)?;
            Ok(ContributorItem {
                alias: r.get(0)?,
                skills: split_csv(&skills),
                contact_hint: r.get(2)?,
            })
        })
        .map_err(sql_err)?;
    let mut contributors = Vec::new();
    for row in c_rows {
        contributors.push(row.map_err(sql_err)?);
    }

    Ok(Json(DiscoverResponse {
        projects,
        contributors,
    }))
}

async fn contribute(
    State(state): State<AppState>,
    Json(req): Json<ContributeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    if req.alias.trim().is_empty() || req.contact_hint.trim().is_empty() || req.skills.is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "alias/skills/contact_hint required"));
    }

    let now = Utc::now().to_rfc3339();
    let skills = req.skills.join(",");
    let conn = state.db.lock().map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "db lock poisoned"))?;
    conn.execute(
        "INSERT INTO contributors (alias, skills, contact_hint, created_at) VALUES (?, ?, ?, ?)",
        params![req.alias, skills, req.contact_hint, now],
    )
    .map_err(sql_err)?;

    Ok(Json(serde_json::json!({"registered": true})))
}

async fn publish_knowledge(
    State(state): State<AppState>,
    Json(req): Json<PublishKnowledgeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    if req.kind.trim().is_empty() || req.title.trim().is_empty() || req.body.trim().is_empty() || req.author_alias.trim().is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "kind/title/body/author_alias required"));
    }

    let now = Utc::now().to_rfc3339();
    let tags = req.tags.join(",");
    let conn = state.db.lock().map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "db lock poisoned"))?;
    conn.execute(
        "INSERT INTO knowledge_resources (kind, title, body, tags, author_alias, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![req.kind, req.title, req.body, tags, req.author_alias, now],
    )
    .map_err(sql_err)?;

    Ok(Json(serde_json::json!({"published": true})))
}

async fn analytics_summary(
    State(state): State<AppState>,
) -> Result<Json<AnalyticsResponse>, (StatusCode, Json<ApiError>)> {
    let conn = state.db.lock().map_err(|_| api_err(StatusCode::INTERNAL_SERVER_ERROR, "db lock poisoned"))?;

    let project_count = count(&conn, "projects")?;
    let contributor_count = count(&conn, "contributors")?;
    let insight_count = count(&conn, "insights")?;

    let mut top_tags = Vec::new();
    let mut stmt = conn.prepare("SELECT tags FROM projects").map_err(sql_err)?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(sql_err)?;

    let mut freq = std::collections::HashMap::<String, i64>::new();
    for row in rows {
        for t in split_csv(&row.map_err(sql_err)?) {
            *freq.entry(t).or_insert(0) += 1;
        }
    }
    let mut items = freq.into_iter().collect::<Vec<_>>();
    items.sort_by(|a, b| b.1.cmp(&a.1));
    for (tag, _) in items.into_iter().take(10) {
        top_tags.push(tag);
    }

    Ok(Json(AnalyticsResponse {
        project_count,
        contributor_count,
        insight_count,
        top_tags,
    }))
}

fn count(conn: &Connection, table: &str) -> Result<i64, (StatusCode, Json<ApiError>)> {
    // Validate table name against whitelist to prevent SQL injection
    let valid_tables = ["projects", "contributors", "insights"];
    if !valid_tables.contains(&table) {
        return Err(api_err(StatusCode::BAD_REQUEST, "invalid table name"));
    }
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .map_err(sql_err)
}

fn seed_defaults(conn: &Connection) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let existing: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if existing > 0 {
        return Ok(());
    }

    conn.execute(
        "INSERT INTO projects (name, summary, tags, maintainer_alias, created_at)
         VALUES ('nyx-net', 'Networking primitives for Nyx', 'network,async,performance', 'alice', ?)",
        params![now],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO projects (name, summary, tags, maintainer_alias, created_at)
         VALUES ('nyx-crypto-plus', 'Cryptographic toolkit', 'crypto,security,hash', 'bob', ?)",
        params![now],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO issues (title, normalized_pattern, solution, tags, confidence, created_at)
         VALUES ('undefined identifier in function body', 'undefined identifier', 'declare variable using let before use', 'parser,semantic', 0.94, ?)",
        params![now],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO issues (title, normalized_pattern, solution, tags, confidence, created_at)
         VALUES ('slow build due to large module', 'slow build large module', 'split modules and enable build optimize caching', 'build,performance', 0.89, ?)",
        params![now],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO insights (insight_type, pattern_hash, recommendation, impact_score, source_anonymous_id, created_at)
         VALUES ('build', ?, 'parallelize checks and use incremental cache keying', 0.91, 'seed', ?)",
        params![sha256_hex(b"build-cache"), now],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO insights (insight_type, pattern_hash, recommendation, impact_score, source_anonymous_id, created_at)
         VALUES ('runtime', ?, 'reduce scheduler churn by batching task dispatch', 0.84, 'seed', ?)",
        params![sha256_hex(b"scheduler-batch"), now],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn split_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn api_err(code: StatusCode, msg: &str) -> (StatusCode, Json<ApiError>) {
    (code, Json(ApiError { error: msg.to_string() }))
}

fn sql_err(err: rusqlite::Error) -> (StatusCode, Json<ApiError>) {
    api_err(StatusCode::INTERNAL_SERVER_ERROR, &format!("db error: {err}"))
}
