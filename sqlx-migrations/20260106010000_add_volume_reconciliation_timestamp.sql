-- Add reconciliation timestamps to instance_volumes for volume reconciliation tracking
-- All data is preserved for audit, traceability, FinOps calculations, and debugging
-- This allows precise cost calculations and recalculation based on detailed usage per second

-- reconciled_at: Timestamp when volume reconciliation was completed successfully
-- (Volume confirmed deleted at provider after verification)
ALTER TABLE instance_volumes 
ADD COLUMN IF NOT EXISTS reconciled_at timestamp with time zone;

COMMENT ON COLUMN instance_volumes.reconciled_at IS 
'Timestamp when volume reconciliation was completed successfully. '
'Volume confirmed deleted at provider. Data preserved for audit, traceability, and FinOps.';

-- last_reconciliation: Timestamp of last reconciliation attempt (for backoff/retry logic)
ALTER TABLE instance_volumes 
ADD COLUMN IF NOT EXISTS last_reconciliation timestamp with time zone;

COMMENT ON COLUMN instance_volumes.last_reconciliation IS 
'Timestamp of last reconciliation attempt. Used for backoff/retry logic. '
'Updated on each reconciliation attempt, regardless of success/failure.';

-- Add index for efficient queries on volumes needing reconciliation
CREATE INDEX IF NOT EXISTS idx_instance_volumes_reconciliation 
ON instance_volumes(deleted_at, reconciled_at, last_reconciliation) 
WHERE deleted_at IS NOT NULL AND reconciled_at IS NULL;

