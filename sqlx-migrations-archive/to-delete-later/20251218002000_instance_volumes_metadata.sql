-- Persist provider volume metadata so UI can display storage details.

ALTER TABLE instance_volumes
    ADD COLUMN IF NOT EXISTS provider_volume_name text,
    ADD COLUMN IF NOT EXISTS is_boot boolean DEFAULT false NOT NULL;


