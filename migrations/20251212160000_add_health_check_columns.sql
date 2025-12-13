-- Add health check and state tracking columns to instances table
-- Migration: 20251212160000_add_health_check_columns.sql

ALTER TABLE instances
  ADD COLUMN IF NOT EXISTS ready_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS last_health_check TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS health_check_failures INTEGER DEFAULT 0;

-- Create instance_state_history table for state transition tracking
CREATE TABLE IF NOT EXISTS instance_state_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    from_status VARCHAR(50),
    to_status VARCHAR(50) NOT NULL,
    reason TEXT,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_state_history_instance ON instance_state_history(instance_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_instances_status ON instances(status) WHERE status IN ('booting', 'provisioning', 'draining');
CREATE INDEX IF NOT EXISTS idx_instances_health_check ON instances(last_health_check) WHERE status = 'booting';

-- Add startup_failed status if not exists
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumlabel = 'startup_failed' AND enumtypid = 'instance_status'::regtype) THEN
        ALTER TYPE instance_status ADD VALUE 'startup_failed';
    END IF;
    
    IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumlabel = 'draining' AND enumtypid = 'instance_status'::regtype) THEN
        ALTER TYPE instance_status ADD VALUE 'draining';
    END IF;
    
    IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumlabel = 'terminating' AND enumtypid = 'instance_status'::regtype) THEN
        ALTER TYPE instance_status ADD VALUE 'terminating';
    END IF;
END $$;
