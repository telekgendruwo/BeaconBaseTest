#![allow(dead_code)]

mod scanner;
mod inferrer;
mod generator;
mod validator;
mod models;
mod errors;
mod db;
mod verifier;

#[cfg(test)]
mod tests;

use anyhow::{Result as AnyResult, Context};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::SystemTime, net::SocketAddr};
use std::result::Result as StdResult;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct AppState {
    redis_client: Arc<redis::Client>,
}

const RATE_LIMIT_WINDOW_SECONDS: u64 = 60;
const RATE_LIMIT_MAX_REQUESTS: usize = 20;

fn random_emoji() -> &'static str {
    ["⬛", "⬜"].choose(&mut rand::thread_rng()).unwrap_or(&"⬛")
}

async fn check_rate_limit(state: &AppState, ip: &str) -> StdResult<(), StatusCode> {
    let key = format!("ratelimit:{}", ip);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut conn = state.redis_client
        .get_multiplexed_async_connection().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: StdResult<Vec<redis::Value>, _> = redis::pipe()
        .atomic()
        .zrembyscore(&key, 0, (now - RATE_LIMIT_WINDOW_SECONDS) as f64)
        .zadd(&key, now, now)
        .zcard(&key)
        .expire(&key, RATE_LIMIT_WINDOW_SECONDS as i64)
        .query_async(&mut conn)
        .await;

    let results = results.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let count: usize = if results.len() >= 3 {
        match &results[2] {
            redis::Value::Int(c) => *c as usize,
            _ => 0,
        }
    } else { 0 };

    if count > RATE_LIMIT_MAX_REQUESTS {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    Ok(())
}

#[derive(Parser)]
#[command(name = "beacon")]
#[command(about = "⬛ Make any repo agent-ready. Instantly.")]
#[command(version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Generate {
        target: String,
        #[arg(short, long, default_value = "AGENTS.md")]
        output: String,
        #[arg(long, default_value = "gemini")]
        provider: String,
        #[arg(long)]
        api_key: Option<String>,
    },
    Validate {
        file: String,
        #[arg(long)]
        check_endpoints: bool,
        #[arg(long)]
        provider: Option<String>,
    },
    Serve {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
}

#[derive(Deserialize)]
struct GenerateRequest {
    #[serde(flatten)]
    repo_context: models::RepoContext,
    provider: Option<String>,
    api_key: Option<String>,
}

#[derive(Serialize)]
struct GenerateResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    agents_md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest: Option<models::AgentsManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize)]
struct ValidateRequest {
    content: String,
    provider: Option<String>,
    api_key: Option<String>,
}

#[derive(Serialize)]
struct ValidateResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    valid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    warnings: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

async fn handle_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GenerateRequest>,
) -> StdResult<impl IntoResponse, errors::BeaconError> {
    let ip = headers.get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    if let Err(status) = check_rate_limit(&state, &ip).await {
        return Ok(status.into_response());
    }

    let is_cloud = req.api_key.is_none();
    let mut rid_final = None;
    let provider = req.provider.unwrap_or_else(|| "gemini".to_string());
    let mut actual_provider = provider.clone();

    if is_cloud {
        let txn_hash = headers.get("x-payment-txn-hash").and_then(|h| h.to_str().ok());
        let chain = headers.get("x-payment-chain").and_then(|h| h.to_str().ok());
        let run_id = headers.get("x-payment-run-id").and_then(|h| h.to_str().ok());

        if let (Some(txn), Some(ch), Some(rid)) = (txn_hash, chain, run_id) {
            rid_final = Some(rid.to_string());
            if db::payment_already_used(txn).await.unwrap_or(false) {
                return Err(errors::BeaconError::TransactionAlreadyUsed);
            }
            let amount = std::env::var("PAYMENT_AMOUNT_USDC").unwrap_or("0.09".to_string()).parse::<f64>().unwrap_or(0.09);
            let wallet = if ch == "base" { std::env::var("BEACON_WALLET_BASE").unwrap_or_default() } else { std::env::var("BEACON_WALLET_SOLANA").unwrap_or_default() };
            let verified = verifier::verify_payment(ch, txn, amount, &wallet).await.map_err(|e| errors::BeaconError::InferenceError(e.to_string()))?;
            if !verified {
                return Err(errors::BeaconError::CloudError { status: 402, message: "Payment not verified".into() });
            }
            db::mark_run_paid(rid, txn, ch).await.ok();
            db::record_payment(rid, txn, ch, None).await.ok();
            actual_provider = "gemini".to_string();
        } else {
            let rid = db::create_run(&req.repo_context.name).await.map_err(|e| errors::BeaconError::DatabaseError(e.to_string()))?;
            return Err(errors::BeaconError::PaymentRequired {
                run_id: rid,
                amount: "0.09".into(),
                base_addr: std::env::var("BEACON_WALLET_BASE").unwrap_or_default(),
                sol_addr: std::env::var("BEACON_WALLET_SOLANA").unwrap_or_default(),
            });
        }
    }

    let manifest = inferrer::infer_capabilities(&req.repo_context, &actual_provider, req.api_key.as_deref()).await.map_err(|e| errors::BeaconError::InferenceError(e.to_string()))?;
    let tmp_path = format!("/tmp/beacon_v2_{}.md", &req.repo_context.name);
    generator::generate_agents_md(&manifest, &tmp_path).map_err(|e| errors::BeaconError::Unknown(e.to_string()))?;
    let content = std::fs::read_to_string(&tmp_path).map_err(|e| errors::BeaconError::IoError(e))?;
    let _ = std::fs::remove_file(&tmp_path);

    if is_cloud {
        if let Some(rid) = rid_final {
            db::mark_run_complete(&rid, &content).await.ok();
        }
    }

    Ok(Json(GenerateResponse {
        success: true,
        agents_md: Some(content),
        manifest: Some(manifest),
        error: None,
    }).into_response())
}

async fn handle_validate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ValidateRequest>,
) -> StdResult<impl IntoResponse, errors::BeaconError> {
    let ip = headers.get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    if let Err(status) = check_rate_limit(&state, &ip).await {
        return Ok(status.into_response());
    }

    let is_cloud = req.api_key.is_none();

    if is_cloud {
        let txn_hash = headers.get("x-payment-txn-hash").and_then(|h| h.to_str().ok());
        let chain = headers.get("x-payment-chain").and_then(|h| h.to_str().ok());
        let run_id = headers.get("x-payment-run-id").and_then(|h| h.to_str().ok());

        if let (Some(txn), Some(ch), Some(rid)) = (txn_hash, chain, run_id) {
            if db::payment_already_used(txn).await.unwrap_or(false) {
                return Err(errors::BeaconError::TransactionAlreadyUsed);
            }
            let amount = std::env::var("PAYMENT_AMOUNT_USDC").unwrap_or("0.09".to_string()).parse::<f64>().unwrap_or(0.09);
            let wallet = if ch == "base" { std::env::var("BEACON_WALLET_BASE").unwrap_or_default() } else { std::env::var("BEACON_WALLET_SOLANA").unwrap_or_default() };
            let verified = verifier::verify_payment(ch, txn, amount, &wallet).await.map_err(|e| errors::BeaconError::ValidationError(e.to_string()))?;
            if !verified {
                return Err(errors::BeaconError::CloudError { status: 402, message: "Payment not verified".into() });
            }
            db::mark_run_paid(rid, txn, ch).await.ok();
            db::record_payment(rid, txn, ch, None).await.ok();
        } else {
            let rid = db::create_run("validate-only").await.map_err(|e| errors::BeaconError::DatabaseError(e.to_string()))?;
            return Err(errors::BeaconError::PaymentRequired {
                run_id: rid,
                amount: "0.09".into(),
                base_addr: std::env::var("BEACON_WALLET_BASE").unwrap_or_default(),
                sol_addr: std::env::var("BEACON_WALLET_SOLANA").unwrap_or_default(),
            });
        }
    }

    let result = validator::validate_content(&req.content).map_err(|e| errors::BeaconError::ValidationError(e.to_string()))?;

    Ok(Json(ValidateResponse {
        success: true,
        valid: Some(result.valid),
        errors: Some(result.errors),
        warnings: Some(result.warnings),
        error: None,
    }).into_response())
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok", "version": VERSION }))
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { target, output, provider, api_key } => {
            println!("{} Beacon — scanning {}...", random_emoji(), target);
            let ctx = scanner::scan_local(&target)?;
            println!("📦 Repo: {} ({} source files)", ctx.name, ctx.source_files.len());
            let manifest = inferrer::infer_capabilities(&ctx, &provider, api_key.as_deref()).await?;
            generator::generate_agents_md(&manifest, &output)?;
            println!("\n✅ Done! AGENTS.md written to: {}", output);
        }
        Commands::Validate { file } => {
            println!("{} Beacon — validating {}...", random_emoji(), file);
            let result = validator::validate_file(&file)?;
            println!("\n📋 Validation Report");
            println!("   Valid:    {}", if result.valid { "✅ Yes" } else { "❌ No" });
        }
        Commands::Serve { port } => {
            let redis_url = std::env::var("REDIS_URL").context("REDIS_URL not set")?;
            let state = AppState {
                redis_client: Arc::new(redis::Client::open(redis_url)?),
            };
            let app = Router::new()
                .route("/health", get(health))
                .route("/generate", post(handle_generate).with_state(state.clone()))
                .route("/validate", post(handle_validate).with_state(state));
            
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            println!("{} Beacon Cloud active on :{}", random_emoji(), port);
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}
