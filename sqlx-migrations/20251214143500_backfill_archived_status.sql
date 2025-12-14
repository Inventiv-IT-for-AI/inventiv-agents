-- Backfill archived instances to use the new 'archived' status.
-- Migration: 20251214143500_backfill_archived_status.sql

-- Instances: if archived flag is set, status should be 'archived'
UPDATE instances
SET status = 'archived'
WHERE is_archived = true
  AND status = 'terminated';

-- Action logs: align previous ARCHIVE_INSTANCE logs to show terminated -> archived
UPDATE action_logs al
SET instance_status_after = 'archived'
WHERE al.action_type = 'ARCHIVE_INSTANCE'
  AND al.status = 'success'
  AND al.instance_status_after = 'terminated'
  AND al.instance_id IS NOT NULL
  AND EXISTS (
    SELECT 1 FROM instances i
    WHERE i.id = al.instance_id
      AND i.is_archived = true
      AND i.status = 'archived'
  );

