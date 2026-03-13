use anyhow::{Result, Context};
use ethers_core::types::{Address, H256, U256};
use ethers_providers::{Provider, Http, Middleware};
use std::str::FromStr;
use serde_json::json;

const BASE_USDC: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
const SOLANA_USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

pub async fn verify_payment(
    chain: &str,
    txn_hash: &str,
    expected_amount: f64,
    expected_address: &str,
) -> Result<bool> {
    match chain {
        "base" => verify_base(txn_hash, expected_amount, expected_address).await,
        "solana" => verify_solana(txn_hash, expected_amount, expected_address).await,
        _ => anyhow::bail!("Unsupported chain: {}", chain),
    }
}

async fn verify_base(
    txn_hash: &str,
    expected_amount: f64,
    expected_address: &str,
) -> Result<bool> {
    let rpc_url = std::env::var("BASE_RPC_URL").unwrap_or_else(|_| "https://mainnet.base.org".to_string());
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let hash = H256::from_str(txn_hash).context("Invalid Base txn hash")?;
    let receipt = provider.get_transaction_receipt(hash).await?
        .context("Base transaction receipt not found")?;
    if receipt.status != Some(1.into()) {
        return Ok(false);
    }
    let usdc_addr = Address::from_str(BASE_USDC)?;
    let receiver_addr = Address::from_str(expected_address)?;
    let transfer_topic = H256::from_str("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")?;
    for log in receipt.logs {
        if log.address == usdc_addr && log.topics.len() == 3 && log.topics[0] == transfer_topic {
            let to = Address::from_slice(&log.topics[2][12..]);
            if to == receiver_addr {
                let value = U256::from_big_endian(&log.data);
                let amount_f64 = value.as_u128() as f64 / 1_000_000.0;
                if (amount_f64 - expected_amount).abs() < 0.001 {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

async fn verify_solana(
    txn_hash: &str,
    expected_amount: f64,
    expected_address: &str,
) -> Result<bool> {
    let rpc_url = std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let client = reqwest::Client::new();
    let resp = client.post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": [
                txn_hash,
                { "encoding": "json", "maxSupportedTransactionVersion": 0 }
            ]
        }))
        .send().await?.json::<serde_json::Value>().await?;
    let meta = &resp["result"]["meta"];
    if meta.is_null() { return Ok(false); }
    let pre = meta["preTokenBalances"].as_array().context("Pre-balances missing")?;
    let post = meta["postTokenBalances"].as_array().context("Post-balances missing")?;
    let mut pre_val = 0.0;
    for b in pre {
        if b["mint"] == SOLANA_USDC && b["owner"] == expected_address {
            pre_val = b["uiTokenAmount"]["uiAmount"].as_f64().unwrap_or(0.0);
            break;
        }
    }
    let mut post_val = 0.0;
    for b in post {
        if b["mint"] == SOLANA_USDC && b["owner"] == expected_address {
            post_val = b["uiTokenAmount"]["uiAmount"].as_f64().unwrap_or(0.0);
            break;
        }
    }
    if (post_val - pre_val - expected_amount).abs() < 0.001 {
        return Ok(true);
    }
    Ok(false)
}
