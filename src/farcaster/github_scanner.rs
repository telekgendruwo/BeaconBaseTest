use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use once_cell::sync::Lazy;
use url::Url;
use crate::models::{Language, RepoContext, SourceFile};

static CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .use_rustls_tls()
        .user_agent("beacon-scanner")
        .build()
        .expect("Failed to create reqwest client")
});

const MAX_SOURCE_FILES: usize = 50;
const MAX_FILE_BYTES: usize = 50_000;

#[derive(Deserialize)]
struct GitHubTree {
    tree: Vec<TreeEntry>,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    r#type: String,
    size: Option<u64>,
}

#[derive(Deserialize)]
struct GitHubContent {
    content: Option<String>,
    encoding: Option<String>,
}

#[derive(Deserialize)]
struct GitHubRepo {
    default_branch: String,
}

fn parse_github_url(github_url: &str) -> Result<(String, String)> {
    let url_str = if github_url.starts_with("http") {
        github_url.to_string()
    } else {
        format!("https://{}", github_url)
    };

    let url = Url::parse(&url_str).context("Invalid GitHub URL")?;
    let segments: Vec<&str> = url
        .path_segments()
        .context("No path in URL")?
        .filter(|s| !s.is_empty())
        .collect();

    if segments.len() < 2 {
        anyhow::bail!("Expected github.com/owner/repo, got: {}", github_url);
    }

    Ok((segments[0].to_string(), segments[1].trim_end_matches(".git").to_string()))
}

async fn fetch_file(owner: &str, repo: &str, path: &str, token: Option<&str>) -> Result<String> {
    let url = format!("https://api.github.com/repos/{}/{}/contents/{}", owner, repo, path);
    let mut req = CLIENT.get(&url).header("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }

    let resp = req.send().await.context("Failed to fetch file from GitHub")?;
    if !resp.status().is_success() {
        anyhow::bail!("GitHub API returned {} for {}", resp.status(), path);
    }

    let content: GitHubContent = resp.json().await?;
    decode_content(&content)
}

fn decode_content(content: &GitHubContent) -> Result<String> {
    match (&content.content, &content.encoding) {
        (Some(encoded), Some(enc)) if enc == "base64" => {
            let cleaned: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &cleaned)
                .context("Failed to decode base64 content")?;
            Ok(String::from_utf8_lossy(&bytes).to_string())
        }
        (Some(raw), _) => Ok(raw.clone()),
        _ => anyhow::bail!("No content in GitHub response"),
    }
}

pub async fn scan_remote(github_url: &str, token: Option<&str>) -> Result<RepoContext> {
    let (owner, repo) = parse_github_url(github_url)?;
    println!("   📡 Scanning remote: {}/{}", owner, repo);

    let repo_url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    let mut req = CLIENT.get(&repo_url).header("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let resp = req.send().await.context("Failed to fetch repo info")?;
    if !resp.status().is_success() {
        anyhow::bail!("GitHub API returned {} for repo info", resp.status());
    }
    let repo_info: GitHubRepo = resp.json().await?;
    let branch = &repo_info.default_branch;

    let tree_url = format!(
        "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
        owner, repo, branch
    );
    let mut req = CLIENT.get(&tree_url).header("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let resp = req.send().await.context("Failed to fetch repo tree")?;
    if !resp.status().is_success() {
        anyhow::bail!("GitHub API returned {} for tree", resp.status());
    }
    let tree: GitHubTree = resp.json().await?;

    let mut ctx = RepoContext {
        name: repo.clone(),
        ..Default::default()
    };

    let mut source_candidates: Vec<&TreeEntry> = Vec::new();

    for entry in &tree.tree {
        if entry.r#type != "blob" {
            continue;
        }
        if let Some(size) = entry.size {
            if size > MAX_FILE_BYTES as u64 {
                continue;
            }
        }
        if should_skip_path(&entry.path) {
            continue;
        }

        let filename = entry.path.rsplit('/').next().unwrap_or(&entry.path).to_lowercase();

        if filename.starts_with("readme") {
            if ctx.readme.is_none() {
                ctx.readme = fetch_file(&owner, &repo, &entry.path, token).await.ok();
                if ctx.readme.is_some() {
                    println!("   ✓ README found");
                }
            }
            continue;
        }

        if filename == "agents.md" {
            ctx.existing_agents_md = fetch_file(&owner, &repo, &entry.path, token).await.ok();
            if ctx.existing_agents_md.is_some() {
                println!("   ✓ Existing AGENTS.md found");
            }
            continue;
        }

        if matches!(filename.as_str(), "cargo.toml" | "package.json" | "pyproject.toml" | "go.mod") {
            if ctx.package_manifest.is_none() {
                ctx.package_manifest = fetch_file(&owner, &repo, &entry.path, token).await.ok();
                if ctx.package_manifest.is_some() {
                    println!("   ✓ Package manifest found: {}", filename);
                }
            }
            continue;
        }

        if filename.contains("openapi") || filename.contains("swagger") {
            if ctx.openapi_spec.is_none() {
                ctx.openapi_spec = fetch_file(&owner, &repo, &entry.path, token).await.ok();
                if ctx.openapi_spec.is_some() {
                    println!("   ✓ OpenAPI spec found: {}", entry.path);
                }
            }
            continue;
        }

        if is_source_ext(&entry.path) {
            source_candidates.push(entry);
        }
    }

    let to_fetch = source_candidates.iter().take(MAX_SOURCE_FILES);
    for entry in to_fetch {
        if let Ok(content) = fetch_file(&owner, &repo, &entry.path, token).await {
            let ext = entry.path.rsplit('.').next().unwrap_or("");
            let lang = Language::from_extension(ext);
            ctx.source_files.push(SourceFile {
                path: entry.path.clone(),
                language: lang,
                content,
            });
        }
    }

    println!(
        "   ✓ Remote scan complete — {} source files collected",
        ctx.source_files.len()
    );

    Ok(ctx)
}

fn should_skip_path(path: &str) -> bool {
    let skip_dirs = [
        "target/", "node_modules/", ".git/", ".github/", "dist/",
        "build/", "__pycache__/", ".venv/", "venv/",
    ];
    let skip_files = [".DS_Store", "Thumbs.db"];

    for dir in &skip_dirs {
        if path.contains(dir) {
            return true;
        }
    }
    let filename = path.rsplit('/').next().unwrap_or(path);
    if skip_files.contains(&filename) {
        return true;
    }
    if filename.ends_with(".lock") || filename.ends_with(".sum") {
        return true;
    }
    false
}

fn is_source_ext(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("");
    matches!(ext, "rs" | "py" | "ts" | "js" | "tsx" | "jsx" | "go" | "java" | "cpp" | "c" | "h")
}
