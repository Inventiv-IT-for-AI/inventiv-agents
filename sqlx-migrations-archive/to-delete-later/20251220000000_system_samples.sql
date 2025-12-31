-- System-level time series samples (CPU/Mem/Disk/Network) from worker heartbeats.
-- Uses TimescaleDB hypertable for efficient inserts/queries.
-- Retention is short by default (raw signals).

CREATE TABLE IF NOT EXISTS system_samples (
    time timestamptz NOT NULL DEFAULT NOW(),
    instance_id uuid NOT NULL,
    cpu_usage_pct double precision,
    load1 double precision,
    mem_used_bytes bigint,
    mem_total_bytes bigint,
    disk_used_bytes bigint,
    disk_total_bytes bigint,
    net_rx_bps double precision,
    net_tx_bps double precision,
    PRIMARY KEY (time, instance_id),
    FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_system_samples_instance_time
    ON system_samples (instance_id, time DESC);

-- Convert to hypertable (idempotent)
SELECT public.create_hypertable('system_samples', 'time', if_not_exists => TRUE);

-- Retention: keep raw for 7 days (best-effort)
DO $$
BEGIN
  PERFORM public.add_retention_policy('system_samples', INTERVAL '7 days');
EXCEPTION WHEN OTHERS THEN
  -- ignore
END $$;


