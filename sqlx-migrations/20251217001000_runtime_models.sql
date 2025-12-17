-- Runtime models registry + counters.
-- Tracks models that have been seen on any worker (instances.worker_model_id),
-- and exposes request counters collected by the OpenAI proxy.

CREATE TABLE IF NOT EXISTS runtime_models (
  model_id TEXT PRIMARY KEY,
  first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  total_requests BIGINT NOT NULL DEFAULT 0,
  failed_requests BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_runtime_models_last_seen
  ON runtime_models(last_seen_at DESC);

-- Upsert helper driven by instances table changes.
CREATE OR REPLACE FUNCTION touch_runtime_model_from_instance() RETURNS trigger AS $$
DECLARE
  m TEXT;
  ts TIMESTAMPTZ;
BEGIN
  m := NEW.worker_model_id;
  IF m IS NULL OR btrim(m) = '' THEN
    RETURN NEW;
  END IF;

  ts := COALESCE(NEW.worker_last_heartbeat, NOW());

  INSERT INTO runtime_models(model_id, first_seen_at, last_seen_at)
  VALUES (m, ts, ts)
  ON CONFLICT (model_id) DO UPDATE
    SET last_seen_at = GREATEST(runtime_models.last_seen_at, EXCLUDED.last_seen_at);

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_instances_touch_runtime_model ON instances;
CREATE TRIGGER trg_instances_touch_runtime_model
AFTER INSERT OR UPDATE OF worker_model_id, worker_last_heartbeat
ON instances
FOR EACH ROW
EXECUTE FUNCTION touch_runtime_model_from_instance();


