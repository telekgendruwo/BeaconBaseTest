use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{self, AgentManifestRow, DbPool};
use crate::farcaster::github_scanner;
use crate::generator;
use crate::inferrer;
use crate::models::AgentsManifest;
use crate::validator;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
}

/// Build the Axum router with all API routes.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/agents", get(list_agents))
        .route("/api/agents/{id}", get(get_agent))
        .route("/api/generate", post(generate))
        .route("/api/validate", post(validate))
        .route("/api/farcaster/webhook", post(farcaster_webhook))
        .with_state(state)
}

// ─── Health ──────────────────────────────────────────────

async fn health() -> &'static str {
    "ok"
}

// ─── Agent Search / Browse ───────────────────────────────

#[derive(Deserialize)]
struct ListAgentsQuery {
    q: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Serialize)]
struct ListAgentsResponse {
    agents: Vec<AgentManifestRow>,
    count: usize,
}

async fn list_agents(
    State(state): State<AppState>,
    Query(params): Query<ListAgentsQuery>,
) -> Result<Json<ListAgentsResponse>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let agents = db::search_agents(&state.pool, params.q.as_deref(), limit, offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let count = agents.len();
    Ok(Json(ListAgentsResponse { agents, count }))
}

async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentManifestRow>, (StatusCode, String)> {
    let agent = db::get_agent(&state.pool, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Agent not found".to_string()))?;

    Ok(Json(agent))
}

// ─── Generate ────────────────────────────────────────────

#[derive(Deserialize)]
struct GenerateRequest {
    github_url: String,
    provider: Option<String>,
}

#[derive(Serialize)]
struct GenerateResponse {
    manifest: AgentsManifest,
    agents_md: String,
    id: Option<Uuid>,
}

async fn generate(
    State(state): State<AppState>,
    Json(body): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, (StatusCode, String)> {
    let provider = body.provider.as_deref().unwrap_or("gemini");
    let github_token = std::env::var("GITHUB_TOKEN").ok();

    // Scan the remote repo
    let ctx = github_scanner::scan_remote(&body.github_url, github_token.as_deref())
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Scan failed: {}", e)))?;

    // Infer capabilities
    let manifest = inferrer::infer_capabilities(&ctx, provider, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Inference failed: {}", e),
            )
        })?;

    // Generate markdown
    let agents_md = generator::render_markdown(&manifest);

    // Store in database
    let id = db::insert_agent_manifest(&state.pool, &manifest, None, 0)
        .await
        .ok();

    Ok(Json(GenerateResponse {
        manifest,
        agents_md,
        id,
    }))
}

// ─── Validate ────────────────────────────────────────────

#[derive(Deserialize)]
struct ValidateRequest {
    content: String,
}

async fn validate(
    Json(body): Json<ValidateRequest>,
) -> Result<Json<crate::models::ValidationResult>, (StatusCode, String)> {
    let result = validator::validate_content(&body.content)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(result))
}

// ─── Farcaster Webhook ───────────────────────────────────

async fn farcaster_webhook(
    body: axum::body::Bytes,
) -> StatusCode {
    // Log webhook events for now
    tracing::info!(
        "Farcaster webhook received: {}",
        String::from_utf8_lossy(&body)
    );
    StatusCode::OK
}
