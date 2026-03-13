# Beacon — Deployment Guide

## 1. Database Migrations

Run the following SQL against your Supabase database (SQL Editor or via `psql`).

### Table: `agent_manifests`

```sql
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
);

CREATE INDEX IF NOT EXISTS idx_agent_manifests_name
ON agent_manifests USING gin (to_tsvector('english', name || ' ' || description));

CREATE INDEX IF NOT EXISTS idx_agent_manifests_created
ON agent_manifests (created_at DESC);
```

### Table: `farcaster_scans`

```sql
CREATE TABLE IF NOT EXISTS farcaster_scans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cast_hash TEXT NOT NULL UNIQUE,
    github_url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    agents_md TEXT,
    reply_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_farcaster_scans_cast_hash
ON farcaster_scans (cast_hash);

CREATE INDEX IF NOT EXISTS idx_farcaster_scans_status
ON farcaster_scans (status);
```

### Running via CLI

```bash
psql "$DATABASE_URL" -f migrations/001_agent_manifests.sql
psql "$DATABASE_URL" -f migrations/002_farcaster_scans.sql
```

---

## 2. Vercel Environment Variables

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `DATABASE_URL` | Supabase Postgres connection string | `postgresql://postgres.[ref]:[pass]@aws-0-[region].pooler.supabase.com:6543/postgres` |
| `NEYNAR_API_KEY` | Neynar API key | `6F68AFEB-B7B3-4D1D-9CEC-63C923BD06CA` |
| `NEYNAR_SIGNER_UUID` | Approved Neynar managed signer | `802e5b05-9e31-4385-ac3a-86217d8562a6` |
| `FARCASTER_BOT_FID` | Bot's Farcaster ID | `2872494` |
| `GEMINI_API_KEY` | Google Gemini API key (default inference provider) | — |

### Optional

| Variable | Description | Default |
|----------|-------------|---------|
| `GITHUB_TOKEN` | GitHub Personal Access Token (increases API rate limit from 60 to 5,000 req/hr) | None |
| `CLAUDE_API_KEY` | Anthropic Claude API key (only needed if users select Claude as provider) | None |
| `BASE_WS_URL` | Base chain WebSocket RPC URL (for on-chain event listener) | `wss://base-mainnet.g.alchemy.com/v2/demo` |
| `BEACON_AGENCY_ADDRESS` | BeaconAgency contract address on Base (for Wrap event broadcasting) | `0xd8b934580fcE35a11B58C6D73aDeE468a2833fa8` |

### Where to find `DATABASE_URL`

Supabase Dashboard → **Settings** → **Database** → **Connection string (URI)**

---

## 3. CLI Commands

```bash
# Start the API server (serves Mini App + API endpoints)
beacon serve --port 3000

# Start the Farcaster bot (mention polling + on-chain event listener)
beacon farcaster-bot --poll-interval 30 --channel beacon-agents

# Scan a remote GitHub repo
beacon generate-remote github.com/user/repo --provider gemini

# Scan a local repo (existing)
beacon generate ./path/to/repo

# Validate an AGENTS.md file (existing)
beacon validate AGENTS.md
```

---

## 4. API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/api/agents?q=&limit=&offset=` | Search/list agent manifests |
| `GET` | `/api/agents/:id` | Get a single agent manifest |
| `POST` | `/api/generate` | Generate AGENTS.md from a GitHub URL |
| `POST` | `/api/validate` | Validate AGENTS.md content |
| `POST` | `/api/farcaster/webhook` | Farcaster Mini App lifecycle events |
