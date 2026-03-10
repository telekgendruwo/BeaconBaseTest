use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::time::{Duration, interval};

use crate::db::DbPool;
use crate::farcaster::github_scanner;
use crate::farcaster::neynar::{NeynarClient, Cast};
use crate::generator;
use crate::inferrer;

pub struct BotConfig {
    pub channel_id: String,
    pub poll_interval_secs: u64,
    pub provider: String,
    pub agency_address: Option<String>,
}

#[derive(Debug)]
pub enum BotCommand {
    Scan { github_url: String },
    Validate { github_url: String },
    Help,
    Unknown,
}

impl BotConfig {
    pub fn new(channel_id: String, poll_interval_secs: u64, provider: String) -> Self {
        Self {
            channel_id,
            poll_interval_secs,
            provider,
            agency_address: std::env::var("BEACON_AGENCY_ADDRESS").ok(),
        }
    }
}

/// Parse the command from a cast's text after the @beacon mention.
pub fn parse_command(text: &str) -> BotCommand {
    // Strip the mention (e.g., "@beacon scan ...")
    let lower = text.to_lowercase();
    let after_mention = if let Some(idx) = lower.find("@beacon") {
        &text[idx + 7..].trim()
    } else {
        text.trim()
    };

    let parts: Vec<&str> = after_mention.split_whitespace().collect();
    match parts.first().map(|s| s.to_lowercase()).as_deref() {
        Some("scan") => {
            if let Some(url) = parts.get(1) {
                BotCommand::Scan {
                    github_url: url.to_string(),
                }
            } else {
                BotCommand::Unknown
            }
        }
        Some("validate") => {
            if let Some(url) = parts.get(1) {
                BotCommand::Validate {
                    github_url: url.to_string(),
                }
            } else {
                BotCommand::Unknown
            }
        }
        Some("help") => BotCommand::Help,
        _ => BotCommand::Unknown,
    }
}

/// Main bot loop: poll mentions, parse commands, execute, reply.
pub async fn run_bot(
    neynar: Arc<NeynarClient>,
    config: BotConfig,
    pool: DbPool,
) -> Result<()> {
    println!("⬛ Beacon Bot starting — polling every {}s", config.poll_interval_secs);
    let mut ticker = interval(Duration::from_secs(config.poll_interval_secs));
    let github_token = std::env::var("GITHUB_TOKEN").ok();

    loop {
        ticker.tick().await;
        tracing::debug!("Polling for mentions...");

        let (casts, _cursor) = match neynar.fetch_mentions(None).await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Failed to fetch mentions: {}", e);
                continue;
            }
        };

        for cast in &casts {
            // Check if we already processed this cast
            if crate::db::scan_exists(&pool, &cast.hash).await.unwrap_or(true) {
                continue;
            }

            let command = parse_command(&cast.text);
            tracing::info!("Processing cast {} — command: {:?}", cast.hash, command);

            match command {
                BotCommand::Scan { github_url } => {
                    handle_scan(
                        &neynar,
                        &pool,
                        cast,
                        &github_url,
                        &config.provider,
                        github_token.as_deref(),
                        config.channel_id.as_str(),
                    )
                    .await;
                }
                BotCommand::Validate { github_url } => {
                    handle_validate(
                        &neynar,
                        &pool,
                        cast,
                        &github_url,
                        github_token.as_deref(),
                        config.channel_id.as_str(),
                    )
                    .await;
                }
                BotCommand::Help => {
                    handle_help(&neynar, cast, config.channel_id.as_str()).await;
                }
                BotCommand::Unknown => {
                    handle_help(&neynar, cast, config.channel_id.as_str()).await;
                }
            }
        }
    }
}

async fn handle_scan(
    neynar: &NeynarClient,
    pool: &DbPool,
    cast: &Cast,
    github_url: &str,
    provider: &str,
    github_token: Option<&str>,
    channel_id: &str,
) {
    // Record that we're processing this cast
    let scan_id = match crate::db::insert_farcaster_scan(pool, &cast.hash, github_url).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to insert scan record: {}", e);
            return;
        }
    };

    // Scan the repo
    let ctx = match github_scanner::scan_remote(github_url, github_token).await {
        Ok(ctx) => ctx,
        Err(e) => {
            let _ = neynar
                .post_cast(
                    &format!("❌ Failed to scan {}: {}", github_url, e),
                    Some(&cast.hash),
                    Some(channel_id),
                )
                .await;
            let _ = crate::db::update_farcaster_scan(pool, scan_id, "failed", None, None).await;
            return;
        }
    };

    // Infer capabilities
    let manifest = match inferrer::infer_capabilities(&ctx, provider, None).await {
        Ok(m) => m,
        Err(e) => {
            let _ = neynar
                .post_cast(
                    &format!("❌ Failed to infer capabilities: {}", e),
                    Some(&cast.hash),
                    Some(channel_id),
                )
                .await;
            let _ = crate::db::update_farcaster_scan(pool, scan_id, "failed", None, None).await;
            return;
        }
    };

    // Generate markdown summary
    let agents_md = generator::render_markdown(&manifest);

    // Build threaded reply
    let summary = format!(
        "⬛ Beacon scan complete for {}\n\n🏷️ {}\n📝 {}\n\n🔧 Capabilities: {}\n🌐 Endpoints: {}",
        github_url,
        manifest.name,
        manifest.description,
        manifest.capabilities.len(),
        manifest.endpoints.len(),
    );

    // First reply: summary
    let reply_hash = match neynar.post_cast(&summary, Some(&cast.hash), Some(channel_id)).await {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Failed to post reply: {}", e);
            return;
        }
    };

    // Thread capability details (2-3 per cast)
    if !manifest.capabilities.is_empty() {
        let mut cap_chunks = Vec::new();
        let mut current = String::from("🔧 Capabilities:\n");

        for cap in manifest.capabilities.iter() {
            let entry = format!("\n• `{}` — {}", cap.name, cap.description);
            if current.len() + entry.len() > 950 {
                cap_chunks.push(current.clone());
                current = String::from("🔧 Capabilities (cont.):\n");
            }
            current.push_str(&entry);
        }
        if !current.is_empty() && current != "🔧 Capabilities:\n" {
            cap_chunks.push(current);
        }

        let _ = neynar.post_threaded(&cap_chunks, &reply_hash, Some(channel_id)).await;
    }

    // Store result
    let _ = crate::db::insert_agent_manifest(
        pool,
        &manifest,
        None,
        cast.author.fid as i64,
    )
    .await;

    let _ = crate::db::update_farcaster_scan(
        pool,
        scan_id,
        "complete",
        Some(&agents_md),
        Some(&reply_hash),
    )
    .await;
}

async fn handle_validate(
    neynar: &NeynarClient,
    pool: &DbPool,
    cast: &Cast,
    github_url: &str,
    github_token: Option<&str>,
    channel_id: &str,
) {
    // Record scan
    let scan_id = crate::db::insert_farcaster_scan(pool, &cast.hash, github_url)
        .await
        .ok();

    // Try to fetch AGENTS.md from the repo
    let agents_content = match github_scanner::scan_remote(github_url, github_token).await {
        Ok(ctx) => ctx.existing_agents_md,
        Err(e) => {
            let _ = neynar
                .post_cast(
                    &format!("❌ Failed to fetch repo: {}", e),
                    Some(&cast.hash),
                    Some(channel_id),
                )
                .await;
            return;
        }
    };

    let content = match agents_content {
        Some(c) => c,
        None => {
            let _ = neynar
                .post_cast(
                    "❌ No AGENTS.md found in this repository.",
                    Some(&cast.hash),
                    Some(channel_id),
                )
                .await;
            return;
        }
    };

    let result = match crate::validator::validate_content(&content) {
        Ok(r) => r,
        Err(e) => {
            let _ = neynar
                .post_cast(
                    &format!("❌ Validation error: {}", e),
                    Some(&cast.hash),
                    Some(channel_id),
                )
                .await;
            return;
        }
    };

    let reply = format!(
        "📋 Validation Report for {}\n\n{} Valid: {}\n❌ Errors: {}\n⚠️ Warnings: {}{}{}",
        github_url,
        if result.valid { "✅" } else { "❌" },
        if result.valid { "Yes" } else { "No" },
        result.errors.len(),
        result.warnings.len(),
        if result.errors.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nErrors:\n{}",
                result.errors.iter().map(|e| format!("• {}", e)).collect::<Vec<_>>().join("\n")
            )
        },
        if result.warnings.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nWarnings:\n{}",
                result.warnings.iter().map(|w| format!("• {}", w)).collect::<Vec<_>>().join("\n")
            )
        },
    );

    let _ = neynar.post_cast(&reply, Some(&cast.hash), Some(channel_id)).await;

    if let Some(sid) = scan_id {
        let _ = crate::db::update_farcaster_scan(pool, sid, "complete", None, None).await;
    }
}

async fn handle_help(neynar: &NeynarClient, cast: &Cast, channel_id: &str) {
    let help_text = "⬛ Beacon — Make any repo agent-ready\n\n\
        Commands:\n\
        • scan <github_url> — Scan a repo & generate AGENTS.md\n\
        • validate <github_url> — Validate an existing AGENTS.md\n\
        • help — Show this message\n\n\
        Example: @beacon scan github.com/user/repo";

    let _ = neynar.post_cast(help_text, Some(&cast.hash), Some(channel_id)).await;
}

/// Watch for on-chain Wrap events and broadcast to Farcaster channel.
pub async fn run_event_listener(
    neynar: Arc<NeynarClient>,
    channel_id: String,
    agency_address: String,
) -> Result<()> {
    use ethers::prelude::*;
    use ethers::types::{Address, Filter, H256};
    use std::str::FromStr;

    let rpc_url = std::env::var("BASE_WS_URL")
        .unwrap_or_else(|_| "wss://base-mainnet.g.alchemy.com/v2/demo".to_string());

    println!("⬛ Event listener starting — watching {}", agency_address);

    let provider = Provider::<Ws>::connect(&rpc_url)
        .await
        .context("Failed to connect to Base WebSocket")?;

    let address: Address = agency_address
        .parse()
        .context("Invalid agency contract address")?;

    // Wrap(address,uint256,uint256) event signature
    let wrap_topic = H256::from_str(
        "0x5d624aa9c148153ab3446c5fcb3a68ea5f21877063a7e43eefb28366aa4e6266",
    )
    .unwrap();

    let filter = Filter::new().address(address).topic0(wrap_topic);

    let mut stream = provider
        .subscribe_logs(&filter)
        .await
        .context("Failed to subscribe to logs")?;

    while let Some(log) = stream.next().await {
        let tx_hash = log.transaction_hash.map(|h| format!("{:?}", h)).unwrap_or_default();
        let to = if log.topics.len() > 1 {
            format!("0x{}", ethers::utils::hex::encode(&log.topics[1].as_bytes()[12..]))
        } else {
            "unknown".to_string()
        };

        let token_id = if log.topics.len() > 2 {
            ethers::types::U256::from_big_endian(log.topics[2].as_bytes()).to_string()
        } else {
            "unknown".to_string()
        };

        let cast_text = format!(
            "🆕 New agent identity registered on Base!\n\n\
            🎫 Token ID: {}\n\
            👤 Owner: {}\n\
            🔗 View: https://basescan.org/tx/{}",
            token_id, to, tx_hash
        );

        if let Err(e) = neynar.post_cast(&cast_text, None, Some(&channel_id)).await {
            tracing::error!("Failed to broadcast Wrap event: {}", e);
        }
    }

    Ok(())
}
