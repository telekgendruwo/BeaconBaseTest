use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::json;

const RUNS_TABLE: &str = "runs";
const PAYMENTS_TABLE: &str = "payments";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Run {
    pub id: String,
    pub repo_name: String,
    pub provider: String,
    pub status: String,
    pub txn_hash: Option<String>,
    pub chain: Option<String>,
    pub agents_md: Option<String>,
    pub error: Option<String>,
}



#[derive(Debug, Serialize, Deserialize)]
pub struct Payment {
    pub id: String,
    pub run_id: String,
    pub txn_hash: String,
    pub chain: String,
    pub amount_usdc: f64,
    pub from_address: Option<String>,
    pub confirmed: bool,
}

fn client() -> Result<postgrest::Postgrest> {
    let url = std::env::var("SUPABASE_URL")
        .context("SUPABASE_URL not set")?;
    let key = std::env::var("SUPABASE_SERVICE_KEY")
        .context("SUPABASE_SERVICE_KEY not set")?;

    Ok(postgrest::Postgrest::new(format!("{}/rest/v1", url))
        .insert_header("apikey", &key)
        .insert_header("Authorization", format!("Bearer {}", key)))
}




pub async fn create_run(repo_name: &str) -> Result<String> {
    let db = client()?;
    let run_id = uuid::Uuid::new_v4().to_string();

    db.from(RUNS_TABLE)
        .insert(json!([{
            "id": run_id,
            "repo_name": repo_name,
            "provider": "beacon-ai-cloud",
            "status": "pending"
        }]).to_string())
        .execute()
        .await
        .context("Failed to create run")?;

    Ok(run_id)
}

pub async fn mark_run_paid(run_id: &str, txn_hash: &str, chain: &str) -> Result<()> {
    let db = client()?;

    db.from(RUNS_TABLE)
        .eq("id", run_id)
        .update(json!({
            "status": "paid",
            "txn_hash": txn_hash,
            "chain": chain
        }).to_string())
        .execute()
        .await
        .context("Failed to mark run as paid")?;

    Ok(())
}



pub async fn mark_run_complete(run_id: &str, agents_md: &str) -> Result<()> {

    let db = client()?;

    db.from(RUNS_TABLE)
        .eq("id", run_id)
        .update(json!({
            "status": "complete",
            "agents_md": agents_md
        }).to_string())
        .execute()
        .await
        .context("Failed to mark run complete")?;

    Ok(())
}

pub async fn mark_run_failed(run_id: &str, error: &str) -> Result<()> {
    let db = client()?;

    db.from(RUNS_TABLE)
        .eq("id", run_id)
        .update(json!({
            "status": "failed",
            "error": error
        }).to_string())
        .execute()
        .await
        .context("Failed to mark run failed")?;

    Ok(())
}



pub async fn record_payment(
    run_id: &str,
    txn_hash: &str,
    chain: &str,
    from_address: Option<&str>,
) -> Result<()> {
    let db = client()?;
    let amount = std::env::var("PAYMENT_AMOUNT_USDC")
        .unwrap_or("0.09".to_string())
        .parse::<f64>()
        .unwrap_or(0.09);

    db.from(PAYMENTS_TABLE)
        .insert(json!([{
            "id": uuid::Uuid::new_v4().to_string(),
            "run_id": run_id,
            "txn_hash": txn_hash,
            "chain": chain,
            "amount_usdc": amount, "from_address": from_address,
            "confirmed": true, "confirmed_at": chrono::Utc::now().to_rfc3339()
        }]).to_string())
        .execute()
        .await
        .context("Failed to record payment")?;

    Ok(())
}

pub async fn payment_already_used(txn_hash: &str) -> Result<bool> {
    let db = client()?;

    let resp = db.from(PAYMENTS_TABLE)
        .eq("txn_hash", txn_hash)
        .select("id")
        .execute()
        .await
        .context("Failed to check payment")?;

    let body = resp.text().await?;
    let records: serde_json::Value = serde_json::from_str(&body)?;
    Ok(records.as_array().map(|a| !a.is_empty()).unwrap_or(false))
}