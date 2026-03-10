#![allow(dead_code)]

mod scanner;
mod inferrer;
mod generator;
mod validator;
mod models;
mod db;
mod farcaster;

#[cfg(test)]
mod tests;

use clap::{Parser, Subcommand};
use rand::seq::SliceRandom;
use std::sync::Arc;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn random_emoji() -> &'static str {
    ["⬛", "⬜"].choose(&mut rand::thread_rng()).unwrap_or(&"⬛")
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
    /// Scan a local repo and generate AGENTS.md
    Generate {
        target: String,
        #[arg(short, long, default_value = "AGENTS.md")]
        output: String,
        #[arg(long, default_value = "gemini")]
        provider: String,
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Validate an existing AGENTS.md file
    Validate {
        file: String,
    },
    /// Scan a remote GitHub repo and generate AGENTS.md
    GenerateRemote {
        /// GitHub URL (e.g., github.com/user/repo)
        github_url: String,
        #[arg(short, long, default_value = "AGENTS.md")]
        output: String,
        #[arg(long, default_value = "gemini")]
        provider: String,
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Start the API server
    Serve {
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    /// Start the Farcaster bot
    FarcasterBot {
        #[arg(long, default_value = "30")]
        poll_interval: u64,
        #[arg(long, default_value = "beacon-agents")]
        channel: String,
        #[arg(long, default_value = "gemini")]
        provider: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            target,
            output,
            provider,
            api_key,
        } => {
            println!("{} Beacon — scanning {}...", random_emoji(), target);
            let ctx = scanner::scan_local(&target)?;
            println!("📦 Repo: {} ({} source files)", ctx.name, ctx.source_files.len());
            let manifest = inferrer::infer_capabilities(&ctx, &provider, api_key.as_deref()).await?;
            generator::generate_agents_md(&manifest, &output)?;
            println!("\n✅ Done! AGENTS.md written to: {}", output);
            println!("   Provider:     {}", provider);
            println!("   Capabilities: {}", manifest.capabilities.len());
            println!("   Endpoints:    {}", manifest.endpoints.len());
        }
        Commands::Validate {
            file,
        } => {
            println!("{} Beacon — validating {}...", random_emoji(), file);
            let result = validator::validate_file(&file)?;

            println!("\n📋 Validation Report");
            println!("   Valid:    {}", if result.valid { "✅ Yes" } else { "❌ No" });
            println!("   Errors:   {}", result.errors.len());
            println!("   Warnings: {}", result.warnings.len());

            if !result.errors.is_empty() {
                println!("\n❌ Errors:");
                for e in &result.errors {
                    println!("   • {}", e);
                }
            }
            if !result.warnings.is_empty() {
                println!("\n⚠️  Warnings:");
                for w in &result.warnings {
                    println!("   • {}", w);
                }
            }
        }
        Commands::GenerateRemote {
            github_url,
            output,
            provider,
            api_key,
        } => {
            println!("{} Beacon — scanning remote {}...", random_emoji(), github_url);
            let github_token = std::env::var("GITHUB_TOKEN").ok();
            let ctx = farcaster::github_scanner::scan_remote(&github_url, github_token.as_deref()).await?;
            println!("📦 Repo: {} ({} source files)", ctx.name, ctx.source_files.len());
            let manifest = inferrer::infer_capabilities(&ctx, &provider, api_key.as_deref()).await?;
            generator::generate_agents_md(&manifest, &output)?;
            println!("\n✅ Done! AGENTS.md written to: {}", output);
            println!("   Provider:     {}", provider);
            println!("   Capabilities: {}", manifest.capabilities.len());
            println!("   Endpoints:    {}", manifest.endpoints.len());
        }
        Commands::Serve { port } => {
            println!("{} Beacon — starting server on port {}...", random_emoji(), port);
            let pool = db::init_pool().await?;
            db::run_migrations(&pool).await?;

            let state = farcaster::api::AppState { pool };
            let app = farcaster::api::router(state);

            // Add CORS layer
            use tower_http::cors::{CorsLayer, Any};
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);

            let app = app.layer(cors);

            // Serve miniapp static files if the dist directory exists
            let app = if std::path::Path::new("miniapp/dist").exists() {
                use tower_http::services::ServeDir;
                app.fallback_service(ServeDir::new("miniapp/dist"))
            } else {
                app
            };

            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
            println!("⬛ Beacon server running at http://localhost:{}", port);
            axum::serve(listener, app).await?;
        }
        Commands::FarcasterBot {
            poll_interval,
            channel,
            provider,
        } => {
            println!("{} Beacon — starting Farcaster bot...", random_emoji());
            let pool = db::init_pool().await?;
            db::run_migrations(&pool).await?;

            let neynar = Arc::new(farcaster::neynar::NeynarClient::from_env()?);
            let config = farcaster::bot::BotConfig::new(channel.clone(), poll_interval, provider);

            let agency_address = config.agency_address.clone();

            // Spawn event listener if agency address is configured
            if let Some(addr) = agency_address {
                let neynar_clone = neynar.clone();
                let channel_clone = channel.clone();
                tokio::spawn(async move {
                    if let Err(e) = farcaster::bot::run_event_listener(
                        neynar_clone,
                        channel_clone,
                        addr,
                    )
                    .await
                    {
                        tracing::error!("Event listener error: {}", e);
                    }
                });
            }

            // Run the bot (blocking)
            farcaster::bot::run_bot(neynar, config, pool).await?;
        }
    }
    Ok(())
}
