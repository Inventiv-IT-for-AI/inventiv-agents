-- Persist provider-managed volumes attached to instances.
-- Allows lifecycle control (delete on terminate / keep).

CREATE TABLE IF NOT EXISTS instance_volumes (
  id UUID PRIMARY KEY,
  instance_id UUID NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
  provider_id UUID NOT NULL REFERENCES providers(id),
  zone_code TEXT NOT NULL,
  provider_volume_id TEXT NOT NULL,
  volume_type TEXT NOT NULL, -- e.g. sbs_volume
  size_bytes BIGINT NOT NULL,
  perf_iops INTEGER,
  delete_on_terminate BOOLEAN NOT NULL DEFAULT TRUE,
  status TEXT NOT NULL DEFAULT 'attached', -- creating | attaching | attached | deleting | deleted | failed
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  attached_at TIMESTAMPTZ,
  deleted_at TIMESTAMPTZ,
  error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_instance_volumes_instance_id ON instance_volumes(instance_id);
CREATE INDEX IF NOT EXISTS idx_instance_volumes_provider_volume_id ON instance_volumes(provider_volume_id);

