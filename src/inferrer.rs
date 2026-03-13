use anyhow::{Result, Context};
use reqwest::Client;
use serde_json::{json, Value};
use crate::models::{RepoContext, AgentsManifest};
use once_cell::sync::Lazy;

static CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .use_rustls_tls()
        .build()
        .expect("Failed to create reqwest client")
});

const GEMINI_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";
const CLAUDE_URL: &str =
    "https://api.anthropic.com/v1/messages";
const OPENAI_URL: &str =
    "https://api.openai.com/v1/chat/completions";

pub async fn infer_capabilities(
    ctx: &RepoContext,
    provider: &str,
    api_key: Option<&str>,
) -> Result<AgentsManifest> {
    let prompt = build_prompt(ctx);

    println!("   🤖 Provider: {}", provider);

    let result = match provider {
        "gemini" => {
            let key = resolve_key(api_key, "GEMINI_API_KEY", "gemini")?;
            call_gemini(&prompt, &key).await?
        }
        "claude" => {
            let key = resolve_key(api_key, "CLAUDE_API_KEY", "claude")?;
            call_claude(&prompt, &key).await?
        }
        "openai" => {
            let key = resolve_key(api_key, "OPENAI_API_KEY", "openai")?;
            call_openai(&prompt, &key).await?
        }
        other => anyhow::bail!(
            "Unknown provider '{}'. Valid options: gemini, claude, openai",
            other
        ),
    };

    println!("   ✓ Inferred {} capabilities", result.capabilities.len());
    println!("   ✓ Inferred {} endpoints", result.endpoints.len());

    Ok(result)
}

async fn call_gemini(prompt: &str, api_key: &str) -> Result<AgentsManifest> {
    let response = CLIENT
        .post(format!("{}?key={}", GEMINI_URL, api_key))
        .json(&json!({
            "contents": [{ "parts": [{ "text": prompt }] }],
            "generationConfig": {
                "temperature": 0.2,
                "responseMimeType": "application/json"
            }
        }))
        .send()
        .await
        .context("Failed to reach Gemini API")?;

    check_status(&response, "Gemini")?;

    let raw: Value = response.json().await?;
    let text = raw["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .context("Unexpected Gemini response shape")?;

    parse_manifest(text)
}

async fn call_claude(prompt: &str, api_key: &str) -> Result<AgentsManifest> {
    let response = CLIENT
        .post(CLAUDE_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&json!({
            "model": "claude-sonnet-4-5",
            "max_tokens": 4096,
            "messages": [{
                "role": "user",
                "content": prompt
            }]
        }))
        .send()
        .await
        .context("Failed to reach Claude API")?;

    check_status(&response, "Claude")?;

    let raw: Value = response.json().await?;
    let text = raw["content"][0]["text"]
        .as_str()
        .context("Unexpected Claude response shape")?;

    parse_manifest(text)
}

fn resolve_key(cli_key: Option<&str>, env_var: &str, provider: &str) -> Result<String> {
    if let Some(key) = cli_key {
        return Ok(key.to_string());
    }
    std::env::var(env_var).map_err(|_| anyhow::anyhow!(
        "No API key for {}. Pass --api-key or set {} in your environment.",
        provider, env_var
    ))
}

fn check_status(response: &reqwest::Response, provider: &str) -> Result<()> {
    if !response.status().is_success() {
        anyhow::bail!("{} API returned status {}", provider, response.status());
    }
    Ok(())
}

fn parse_manifest(text: &str) -> Result<AgentsManifest> {
    let clean = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str(clean)
        .context("Failed to parse LLM output as AgentsManifest")
}

fn build_prompt(ctx: &RepoContext) -> String {
    let mut parts: Vec<String> = vec![
        "You are an expert at analyzing software repositories and extracting agent-usable capabilities.".into(),
        "Analyze the following repository context and return a JSON object describing its capabilities for AI agents.".into(),
        "GUIDANCE: Look beyond just utility scripts. Identify server-side capabilities, REST API endpoints (e.g., NestJS/Express/FastAPI decorators like @Get, @Post, @app.get), and background services (notifications, chat systems, indexers).".into(),
        "CRITICAL: Return ONLY valid JSON. No markdown, no explanation, no preamble.".into(),
        "".into(),
        "The JSON must match this exact schema:".into(),
        r#"{
  "name": "string",
  "description": "string — what this project does, written for an AI agent",
  "version": "string or null",
  "capabilities": [
    {
      "name": "string (snake_case)",
      "description": "string — what an agent can do with this",
      "input_schema": null,
      "output_schema": null,
      "examples": ["string"]
    }
  ],
  "endpoints": [
    {
      "path": "string",
      "method": "GET|POST|PUT|DELETE",
      "description": "string",
      "parameters": [
        { "name": "string", "type": "string", "required": true, "description": "string" }
      ]
    }
  ],
  "authentication": { "type": "bearer|api_key|none", "description": "string or null" },
  "rate_limits": null,
  "contact": null
}"#.into(),
        "".into(),
        "--- REPOSITORY CONTEXT ---".into(),
        format!("Project name: {}", ctx.name),
    ];

    if let Some(readme) = &ctx.readme {
        parts.push(format!("\n## README\n{}", truncate(readme, 3000)));
    }
    if let Some(manifest) = &ctx.package_manifest {
        parts.push(format!("\n## Package Manifest\n{}", truncate(manifest, 1000)));
    }
    if let Some(openapi) = &ctx.openapi_spec {
        parts.push(format!("\n## OpenAPI Spec\n{}", truncate(openapi, 3000)));
    }
    if !ctx.source_files.is_empty() {
        parts.push("\n## Source Files".into());
        for file in ctx.source_files.iter().take(10) {
            parts.push(format!("\n### {}\n{}", file.path, truncate(&file.content, 1500)));
        }
    }

    parts.push("\n--- END CONTEXT ---".into());
    parts.push("Return the JSON object:".into());
    parts.join("\n")
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}
