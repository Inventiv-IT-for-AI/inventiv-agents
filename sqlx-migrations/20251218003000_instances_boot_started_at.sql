-- Track when an instance entered BOOTING (initial provisioning or reinstall).
-- Used to compute startup timeout correctly (do not use created_at for reinstalls).

ALTER TABLE instances
    ADD COLUMN IF NOT EXISTS boot_started_at timestamp with time zone;

-- Backfill: for currently booting instances, default boot_started_at to created_at if missing.
UPDATE instances
SET boot_started_at = COALESCE(boot_started_at, created_at)
WHERE status = 'booting'
  AND boot_started_at IS NULL;


