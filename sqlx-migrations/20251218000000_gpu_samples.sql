-- Per-GPU time series samples (nvtop-like dashboard).
-- Uses TimescaleDB hypertable for efficient inserts/queries.

CREATE TABLE IF NOT EXISTS gpu_samples (
    time timestamptz NOT NULL DEFAULT NOW(),
    instance_id uuid NOT NULL,
    gpu_index integer NOT NULL,
    gpu_utilization double precision,
    vram_used_mb double precision,
    vram_total_mb double precision,
    PRIMARY KEY (time, instance_id, gpu_index),
    FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_gpu_samples_instance_gpu_time
    ON gpu_samples (instance_id, gpu_index, time DESC);

-- Convert to hypertable (idempotent)
SELECT public.create_hypertable('gpu_samples', 'time', if_not_exists => TRUE);


