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
