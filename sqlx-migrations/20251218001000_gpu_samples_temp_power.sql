-- Extend GPU samples with temperature and power, and add downsampled aggregates.

ALTER TABLE gpu_samples
    ADD COLUMN IF NOT EXISTS temp_c double precision,
    ADD COLUMN IF NOT EXISTS power_w double precision,
    ADD COLUMN IF NOT EXISTS power_limit_w double precision;

-- 1 minute continuous aggregate
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_matviews WHERE matviewname = 'gpu_samples_1m') THEN
    EXECUTE $sql$
      CREATE MATERIALIZED VIEW gpu_samples_1m
      WITH (timescaledb.continuous) AS
      SELECT
        time_bucket(INTERVAL '1 minute', time) AS bucket,
        instance_id,
        gpu_index,
        AVG(gpu_utilization) AS gpu_utilization,
        AVG(vram_used_mb) AS vram_used_mb,
        MAX(vram_total_mb) AS vram_total_mb,
        AVG(temp_c) AS temp_c,
        AVG(power_w) AS power_w,
        MAX(power_limit_w) AS power_limit_w
      FROM gpu_samples
      GROUP BY bucket, instance_id, gpu_index
      WITH NO DATA
    $sql$;
  END IF;
END $$;

-- 1 hour continuous aggregate
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_matviews WHERE matviewname = 'gpu_samples_1h') THEN
    EXECUTE $sql$
      CREATE MATERIALIZED VIEW gpu_samples_1h
      WITH (timescaledb.continuous) AS
      SELECT
        time_bucket(INTERVAL '1 hour', time) AS bucket,
        instance_id,
        gpu_index,
        AVG(gpu_utilization) AS gpu_utilization,
        AVG(vram_used_mb) AS vram_used_mb,
        MAX(vram_total_mb) AS vram_total_mb,
        AVG(temp_c) AS temp_c,
        AVG(power_w) AS power_w,
        MAX(power_limit_w) AS power_limit_w
      FROM gpu_samples
      GROUP BY bucket, instance_id, gpu_index
      WITH NO DATA
    $sql$;
  END IF;
END $$;

-- 1 day continuous aggregate
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_matviews WHERE matviewname = 'gpu_samples_1d') THEN
    EXECUTE $sql$
      CREATE MATERIALIZED VIEW gpu_samples_1d
      WITH (timescaledb.continuous) AS
      SELECT
        time_bucket(INTERVAL '1 day', time) AS bucket,
        instance_id,
        gpu_index,
        AVG(gpu_utilization) AS gpu_utilization,
        AVG(vram_used_mb) AS vram_used_mb,
        MAX(vram_total_mb) AS vram_total_mb,
        AVG(temp_c) AS temp_c,
        AVG(power_w) AS power_w,
        MAX(power_limit_w) AS power_limit_w
      FROM gpu_samples
      GROUP BY bucket, instance_id, gpu_index
      WITH NO DATA
    $sql$;
  END IF;
END $$;

-- Policies (best-effort; ignore if already set)
DO $$
BEGIN
  PERFORM public.add_continuous_aggregate_policy('gpu_samples_1m',
    start_offset => INTERVAL '30 days',
    end_offset => INTERVAL '1 minute',
    schedule_interval => INTERVAL '1 minute');
EXCEPTION WHEN OTHERS THEN
  -- ignore
END $$;

DO $$
BEGIN
  PERFORM public.add_continuous_aggregate_policy('gpu_samples_1h',
    start_offset => INTERVAL '180 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
EXCEPTION WHEN OTHERS THEN
END $$;

DO $$
BEGIN
  PERFORM public.add_continuous_aggregate_policy('gpu_samples_1d',
    start_offset => INTERVAL '3650 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day');
EXCEPTION WHEN OTHERS THEN
END $$;

-- Retention: keep raw for 7 days (aggregates are long-lived)
DO $$
BEGIN
  PERFORM public.add_retention_policy('gpu_samples', INTERVAL '7 days');
EXCEPTION WHEN OTHERS THEN
END $$;


