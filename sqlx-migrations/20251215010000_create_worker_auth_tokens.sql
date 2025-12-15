-- Per-worker auth tokens (Phase 0.2.1+)
-- Store hashes only; worker receives plaintext token once at bootstrap.

CREATE TABLE IF NOT EXISTS worker_auth_tokens (
  instance_id UUID PRIMARY KEY REFERENCES instances(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  last_seen_at TIMESTAMPTZ,
  rotated_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  worker_id UUID,
  metadata JSONB
);

CREATE INDEX IF NOT EXISTS idx_worker_auth_tokens_last_seen
  ON worker_auth_tokens(last_seen_at DESC)
  WHERE last_seen_at IS NOT NULL;

