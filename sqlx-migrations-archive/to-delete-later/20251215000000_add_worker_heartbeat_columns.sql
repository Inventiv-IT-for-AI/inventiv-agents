-- Add worker heartbeat/capacity columns to instances (Phase 0.2.1 "Worker ready")
-- Safe to run multiple times.

ALTER TABLE instances
  ADD COLUMN IF NOT EXISTS worker_last_heartbeat TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS worker_status TEXT,
  ADD COLUMN IF NOT EXISTS worker_model_id TEXT,
  ADD COLUMN IF NOT EXISTS worker_health_port INTEGER,
  ADD COLUMN IF NOT EXISTS worker_vllm_port INTEGER,
  ADD COLUMN IF NOT EXISTS worker_queue_depth INTEGER,
  ADD COLUMN IF NOT EXISTS worker_gpu_utilization DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS worker_metadata JSONB;

CREATE INDEX IF NOT EXISTS idx_instances_worker_last_heartbeat
  ON instances(worker_last_heartbeat)
  WHERE worker_last_heartbeat IS NOT NULL;

