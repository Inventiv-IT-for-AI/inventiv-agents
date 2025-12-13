-- Add error tracking columns to instances table
-- Migration: 20251212150000_add_error_tracking.sql

ALTER TABLE instances 
  ADD COLUMN IF NOT EXISTS error_message TEXT,
  ADD COLUMN IF NOT EXISTS error_code VARCHAR(50),
  ADD COLUMN IF NOT EXISTS failed_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS retry_count INTEGER DEFAULT 0;

-- Add new status values to instance_status enum
-- Note: In PostgreSQL, we need to add new enum values one by one
DO $$ 
BEGIN
    -- Add provisioning_failed status
    IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumlabel = 'provisioning_failed' AND enumtypid = 'instance_status'::regtype) THEN
        ALTER TYPE instance_status ADD VALUE 'provisioning_failed';
    END IF;
    
    -- Add startup_failed status
    IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumlabel = 'startup_failed' AND enumtypid = 'instance_status'::regtype) THEN
        ALTER TYPE instance_status ADD VALUE 'startup_failed';
    END IF;
END $$;

-- Create index on error_code for faster queries
CREATE INDEX IF NOT EXISTS idx_instances_error_code ON instances(error_code) WHERE error_code IS NOT NULL;

-- Create index on failed_at for analytics
CREATE INDEX IF NOT EXISTS idx_instances_failed_at ON instances(failed_at) WHERE failed_at IS NOT NULL;
