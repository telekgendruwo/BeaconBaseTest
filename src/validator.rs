use anyhow::{Result, Context};
use reqwest::Client;
use crate::models::{ValidationResult, EndpointCheckResult};
use crate::errors::BeaconError;
use serde_json::{json, Value};

const BEACON_CLOUD_URL: &str = "https://beacon-latest.onrender.com";

pub async fn validate_cloud(content: &str) -> Result<ValidationResult> {
    let client = Client::new();
    let beacon_url = std::env::var("BEACON_CLOUD_URL")
        .unwrap_or_else(|_| BEACON_CLOUD_URL.to_string());
    let validate_url = format!("{}/validate", beacon_url);

    println!("   ⚡️ Contacting Beacon Cloud for validation...");

    let payload = json!({
        "content": content
    });

    let initial_res = client
        .post(&validate_url)
        .json(&payload)
        .send()
        .await
        .context("Failed to connect to Beacon Cloud API")?;

    if initial_res.status() == reqwest::StatusCode::PAYMENT_REQUIRED {
        let headers = initial_res.headers();
        let amount = headers.get("x-payment-amount").and_then(|v| v.to_str().ok()).unwrap_or("N/A").to_string();
        let run_id = headers.get("x-payment-run-id").and_then(|v| v.to_str().ok()).context("Missing run ID from server")?.to_string();
        let base_addr = headers.get("x-payment-address-base").and_then(|v| v.to_str().ok()).unwrap_or("N/A").to_string();
        let sol_addr = headers.get("x-payment-address-solana").and_then(|v| v.to_str().ok()).unwrap_or("N/A").to_string();

        println!("   💰 Payment required to proceed.");
        println!("\n--------------------------------------------------");
        println!("Please send {} USDC to one of these addresses:", amount);
        println!("  - Base:   {}", base_addr);
        println!("  - Solana: {}", sol_addr);
        println!("--------------------------------------------------\n");

        let mut chain = String::new();
        println!("Which chain did you pay on? (base/solana)");
        std::io::stdin().read_line(&mut chain).context("Failed to read chain")?;
        let chain = chain.trim().to_lowercase();
        
        let mut txn_hash = String::new();
        println!("Please paste the transaction hash:");
        std::io::stdin().read_line(&mut txn_hash).context("Failed to read transaction hash")?;
        let txn_hash = txn_hash.trim();

        println!("   🔍 Verifying payment...");

        let final_res = client
            .post(&validate_url)
            .header("x-payment-run-id", run_id)
            .header("x-payment-chain", &chain)
            .header("x-payment-txn-hash", txn_hash)
            .json(&payload)
            .send()
            .await
            .context("Failed to send final request to Beacon Cloud")?;

        if !final_res.status().is_success() {
            let status = final_res.status().as_u16();
            let raw: Value = final_res.json().await.unwrap_or(json!({"error": "Unknown error"}));
            let message = raw["error"].as_str().or(raw["error"]["message"].as_str()).unwrap_or("Unknown error").to_string();
            return Err(BeaconError::CloudError { status, message }.into());
        }

        let raw: Value = final_res.json().await?;
        return Ok(ValidationResult {
            valid: raw["valid"].as_bool().unwrap_or(false),
            errors: serde_json::from_value(raw["errors"].clone()).unwrap_or_default(),
            warnings: serde_json::from_value(raw["warnings"].clone()).unwrap_or_default(),
            endpoint_results: serde_json::from_value(raw["endpoint_results"].clone()).unwrap_or_default(),
        });
    }

    if !initial_res.status().is_success() {
        let status = initial_res.status().as_u16();
        let raw: Value = initial_res.json().await.unwrap_or(json!({"error": "Unknown error"}));
        let message = raw["error"].as_str().or(raw["error"]["message"].as_str()).unwrap_or("Unknown error").to_string();
        return Err(BeaconError::CloudError { status, message }.into());
    }

    let raw: Value = initial_res.json().await?;
    Ok(ValidationResult {
        valid: raw["valid"].as_bool().unwrap_or(false),
        errors: serde_json::from_value(raw["errors"].clone()).unwrap_or_default(),
        warnings: serde_json::from_value(raw["warnings"].clone()).unwrap_or_default(),
        endpoint_results: serde_json::from_value(raw["endpoint_results"].clone()).unwrap_or_default(),
    })
}

pub fn validate_file(path: &str) -> Result<ValidationResult> {
    let content = std::fs::read_to_string(path)
        .map_err(|_| anyhow::anyhow!("Could not read file: {}", path))?;

    validate_content(&content)
}

pub fn validate_content(content: &str) -> Result<ValidationResult> {
    let mut errors: Vec<String> = vec![];
    let mut warnings: Vec<String> = vec![];

    if !content.contains("# AGENTS.md") && !content.lines().any(|l| l.starts_with("# ")) {
        errors.push("Missing top-level # heading".to_string());
    }

    if !content.lines().any(|l| l.starts_with("> ")) {
        warnings.push("No description blockquote found (recommended: > Your description)".to_string());
    }

    if !content.contains("## Capabilities") {
        errors.push("Missing ## Capabilities section".to_string());
    }

    if content.contains("## Capabilities") {
        let caps_count = content.matches("### `").count();
        if caps_count == 0 {
            warnings.push("Capabilities section is empty — no capabilities declared".to_string());
        }
    }

    if content.contains("## Endpoints") {
        let valid_methods = ["GET", "POST", "PUT", "DELETE", "PATCH"];
        
        let in_endpoints_section = content
            .split("## Endpoints")
            .nth(1)
            .unwrap_or("");

        let endpoint_lines: Vec<&str> = in_endpoints_section
            .lines()
            .filter(|l| l.starts_with("### `"))
            .collect();

        for line in &endpoint_lines {
            let has_method = valid_methods.iter().any(|m| line.contains(m));
            if !has_method {
                warnings.push(format!(
                    "Endpoint missing HTTP method: {}",
                    line.trim_start_matches("### `").trim_end_matches('`')
                ));
            }
        }
    }

    if !content.contains("Generated by") {
        warnings.push("Missing generator footer".to_string());
    }

    let valid = errors.is_empty();

    if valid {
        println!("   ✅ Schema valid");
    } else {
        println!("   ❌ {} error(s) found", errors.len());
    }

    if !warnings.is_empty() {
        println!("   ⚠️  {} warning(s)", warnings.len());
    }

    Ok(ValidationResult {
        valid,
        errors,
        warnings,
        endpoint_results: vec![],
    })
}

pub async fn check_endpoints(content: &str) -> Result<Vec<EndpointCheckResult>> {
    let client = Client::new();
    let mut results = vec![];
    let base_url = extract_base_url(content);

    for line in content.lines() {
        if !line.starts_with("### `") {
            continue;
        }
        let inner = line.trim_start_matches("### `").trim_end_matches('`');
        let parts: Vec<&str> = inner.splitn(2, ' ').collect();
        if parts.len() != 2 {
            continue;
        }
        let method = parts[0];
        let path = parts[1];

        let url = if path.starts_with("http") {
            path.to_string()
        } else if let Some(base) = &base_url {
            format!("{}{}", base.trim_end_matches('/'), path)
        } else {
            results.push(EndpointCheckResult {
                endpoint: format!("{} {}", method, path),
                reachable: false,
                status_code: None,
                error: Some("No base URL found — cannot check relative paths".to_string()),
            });
            continue;
        };

        println!("   🔍 Checking: {} {}", method, url);

        let req = match method {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            _ => client.get(&url),
        };

        match req.timeout(std::time::Duration::from_secs(5)).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let reachable = status < 500;
                println!("   {} {} → {}", 
                    if reachable { "✅" } else { "❌" }, 
                    url, status);
                results.push(EndpointCheckResult {
                    endpoint: format!("{} {}", method, path),
                    reachable,
                    status_code: Some(status),
                    error: None,
                });
            }
            Err(e) => {
                println!("   ❌ {} → {}", url, e);
                results.push(EndpointCheckResult {
                    endpoint: format!("{} {}", method, path),
                    reachable: false,
                    status_code: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok(results)
}

fn extract_base_url(content: &str) -> Option<String> {
    for line in content.lines() {
        if line.to_lowercase().contains("base url") {
            if let Some(start) = line.find('`') {
                if let Some(end) = line[start + 1..].find('`') {
                    let url = &line[start + 1..start + 1 + end];
                    if url.starts_with("http") {
                        return Some(url.to_string());
                    }
                }
            }
        }
    }
    None
}
