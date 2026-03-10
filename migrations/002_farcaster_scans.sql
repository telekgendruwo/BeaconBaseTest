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
