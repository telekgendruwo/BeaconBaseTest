use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;
use crate::models::AgentsManifest;

pub type DbPool = PgPool;

/// Initialize the database connection pool from DATABASE_URL.
pub async fn init_pool() -> Result<DbPool> {
    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL not set")?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    println!("   ✓ Database connected");
    Ok(pool)
}

/// Run migrations (creates tables if they don't exist).
pub async fn run_migrations(pool: &DbPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agent_manifests (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            run_id TEXT,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            manifest_json JSONB NOT NULL,
            capabilities_count INTEGER NOT NULL DEFAULT 0,
            endpoints_count INTEGER NOT NULL DEFAULT 0,
            on_chain_id TEXT,
            fid BIGINT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await
    .context("Failed to create agent_manifests table")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS farcaster_scans (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            cast_hash TEXT NOT NULL UNIQUE,
            github_url TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            agents_md TEXT,
            reply_hash TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await
    .context("Failed to create farcaster_scans table")?;

    // Create index for agent search
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_manifests_name
        ON agent_manifests USING gin (to_tsvector('english', name || ' ' || description))
        "#,
    )
    .execute(pool)
    .await
    .ok(); // Don't fail if index already exists

    println!("   ✓ Database migrations complete");
    Ok(())
}

/// Insert a new agent manifest.
pub async fn insert_agent_manifest(
    pool: &DbPool,
    manifest: &AgentsManifest,
    run_id: Option<&str>,
    fid: i64,
) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let manifest_json = serde_json::to_value(manifest)
        .context("Failed to serialize manifest")?;

    sqlx::query(
        r#"
        INSERT INTO agent_manifests (id, run_id, name, description, manifest_json, capabilities_count, endpoints_count, fid)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(id)
    .bind(run_id)
    .bind(&manifest.name)
    .bind(&manifest.description)
    .bind(&manifest_json)
    .bind(manifest.capabilities.len() as i32)
    .bind(manifest.endpoints.len() as i32)
    .bind(fid)
    .execute(pool)
    .await
    .context("Failed to insert agent manifest")?;

    Ok(id)
}

/// Search agent manifests by query string.
pub async fn search_agents(
    pool: &DbPool,
    query: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<AgentManifestRow>> {
    let rows = if let Some(q) = query {
        if q.is_empty() {
            sqlx::query_as::<_, AgentManifestRow>(
                r#"
                SELECT id, run_id, name, description, manifest_json, capabilities_count, endpoints_count, on_chain_id, fid, created_at
                FROM agent_manifests
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, AgentManifestRow>(
                r#"
                SELECT id, run_id, name, description, manifest_json, capabilities_count, endpoints_count, on_chain_id, fid, created_at
                FROM agent_manifests
                WHERE to_tsvector('english', name || ' ' || description) @@ plainto_tsquery('english', $1)
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(q)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
    } else {
        sqlx::query_as::<_, AgentManifestRow>(
            r#"
            SELECT id, run_id, name, description, manifest_json, capabilities_count, endpoints_count, on_chain_id, fid, created_at
            FROM agent_manifests
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    };

    Ok(rows)
}

/// Get a single agent manifest by ID.
pub async fn get_agent(pool: &DbPool, id: Uuid) -> Result<Option<AgentManifestRow>> {
    let row = sqlx::query_as::<_, AgentManifestRow>(
        r#"
        SELECT id, run_id, name, description, manifest_json, capabilities_count, endpoints_count, on_chain_id, fid, created_at
        FROM agent_manifests
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Check if a Farcaster scan already exists for a given cast hash.
pub async fn scan_exists(pool: &DbPool, cast_hash: &str) -> Result<bool> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM farcaster_scans WHERE cast_hash = $1",
    )
    .bind(cast_hash)
    .fetch_one(pool)
    .await?;

    Ok(row > 0)
}

/// Insert a new Farcaster scan record.
pub async fn insert_farcaster_scan(
    pool: &DbPool,
    cast_hash: &str,
    github_url: &str,
) -> Result<Uuid> {
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO farcaster_scans (id, cast_hash, github_url, status)
        VALUES ($1, $2, $3, 'scanning')
        "#,
    )
    .bind(id)
    .bind(cast_hash)
    .bind(github_url)
    .execute(pool)
    .await
    .context("Failed to insert farcaster scan")?;

    Ok(id)
}

/// Update a Farcaster scan record.
pub async fn update_farcaster_scan(
    pool: &DbPool,
    id: Uuid,
    status: &str,
    agents_md: Option<&str>,
    reply_hash: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE farcaster_scans
        SET status = $2, agents_md = $3, reply_hash = $4
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(status)
    .bind(agents_md)
    .bind(reply_hash)
    .execute(pool)
    .await
    .context("Failed to update farcaster scan")?;

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct AgentManifestRow {
    pub id: Uuid,
    pub run_id: Option<String>,
    pub name: String,
    pub description: String,
    pub manifest_json: serde_json::Value,
    pub capabilities_count: i32,
    pub endpoints_count: i32,
    pub on_chain_id: Option<String>,
    pub fid: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
