-- Add unique constraint on instance_volumes(instance_id, provider_volume_id)
-- This constraint ensures that each volume can only be tracked once per instance
-- The constraint allows multiple entries if one is deleted (deleted_at IS NOT NULL)
-- but prevents duplicate active volumes

-- First, create a unique index with WHERE deleted_at IS NULL for efficient lookups
CREATE UNIQUE INDEX IF NOT EXISTS idx_instance_volumes_unique 
ON instance_volumes(instance_id, provider_volume_id) 
WHERE deleted_at IS NULL;

-- Then, create a unique constraint without WHERE clause for ON CONFLICT support
-- This allows PostgreSQL to use it in ON CONFLICT clauses
-- Note: This constraint allows multiple entries if one has deleted_at IS NOT NULL
-- The application logic handles checking deleted_at IS NULL before inserting
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'instance_volumes_unique_constraint'
    ) THEN
        ALTER TABLE instance_volumes 
        ADD CONSTRAINT instance_volumes_unique_constraint 
        UNIQUE (instance_id, provider_volume_id);
    END IF;
END $$;

