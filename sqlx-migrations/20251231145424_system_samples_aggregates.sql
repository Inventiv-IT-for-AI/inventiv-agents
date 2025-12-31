-- Add continuous aggregates for system_samples (similar to gpu_samples).
-- Improves query performance for longer time windows (hour/day).

-- 1 minute continuous aggregate
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_matviews WHERE matviewname = 'system_samples_1m') THEN
    EXECUTE $sql$
      CREATE MATERIALIZED VIEW system_samples_1m
      WITH (timescaledb.continuous) AS
      SELECT
        time_bucket(INTERVAL '1 minute', time) AS bucket,
        instance_id,
        AVG(cpu_usage_pct) AS cpu_usage_pct,
        AVG(load1) AS load1,
        AVG(mem_used_bytes)::bigint AS mem_used_bytes,
        MAX(mem_total_bytes)::bigint AS mem_total_bytes,
        AVG(disk_used_bytes)::bigint AS disk_used_bytes,
        MAX(disk_total_bytes)::bigint AS disk_total_bytes,
        AVG(net_rx_bps) AS net_rx_bps,
        AVG(net_tx_bps) AS net_tx_bps
      FROM system_samples
      GROUP BY bucket, instance_id
      WITH NO DATA
    $sql$;
  END IF;
END $$;

-- 1 hour continuous aggregate
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_matviews WHERE matviewname = 'system_samples_1h') THEN
    EXECUTE $sql$
      CREATE MATERIALIZED VIEW system_samples_1h
      WITH (timescaledb.continuous) AS
      SELECT
        time_bucket(INTERVAL '1 hour', time) AS bucket,
        instance_id,
        AVG(cpu_usage_pct) AS cpu_usage_pct,
        AVG(load1) AS load1,
        AVG(mem_used_bytes)::bigint AS mem_used_bytes,
        MAX(mem_total_bytes)::bigint AS mem_total_bytes,
        AVG(disk_used_bytes)::bigint AS disk_used_bytes,
        MAX(disk_total_bytes)::bigint AS disk_total_bytes,
        AVG(net_rx_bps) AS net_rx_bps,
        AVG(net_tx_bps) AS net_tx_bps
      FROM system_samples
      GROUP BY bucket, instance_id
      WITH NO DATA
    $sql$;
  END IF;
END $$;

-- 1 day continuous aggregate
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_matviews WHERE matviewname = 'system_samples_1d') THEN
    EXECUTE $sql$
      CREATE MATERIALIZED VIEW system_samples_1d
      WITH (timescaledb.continuous) AS
      SELECT
        time_bucket(INTERVAL '1 day', time) AS bucket,
        instance_id,
        AVG(cpu_usage_pct) AS cpu_usage_pct,
        AVG(load1) AS load1,
        AVG(mem_used_bytes)::bigint AS mem_used_bytes,
        MAX(mem_total_bytes)::bigint AS mem_total_bytes,
        AVG(disk_used_bytes)::bigint AS disk_used_bytes,
        MAX(disk_total_bytes)::bigint AS disk_total_bytes,
        AVG(net_rx_bps) AS net_rx_bps,
        AVG(net_tx_bps) AS net_tx_bps
      FROM system_samples
      GROUP BY bucket, instance_id
      WITH NO DATA
    $sql$;
  END IF;
END $$;

-- Policies (best-effort; ignore if already set)
DO $$
BEGIN
  PERFORM public.add_continuous_aggregate_policy('system_samples_1m',
    start_offset => INTERVAL '30 days',
    end_offset => INTERVAL '1 minute',
    schedule_interval => INTERVAL '1 minute');
EXCEPTION WHEN OTHERS THEN
  -- ignore
END $$;

DO $$
BEGIN
  PERFORM public.add_continuous_aggregate_policy('system_samples_1h',
    start_offset => INTERVAL '180 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
EXCEPTION WHEN OTHERS THEN
  -- ignore
END $$;

DO $$
BEGIN
  PERFORM public.add_continuous_aggregate_policy('system_samples_1d',
    start_offset => INTERVAL '3650 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day');
EXCEPTION WHEN OTHERS THEN
  -- ignore
END $$;

