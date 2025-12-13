-- Create action_logs table for audit logging
-- Migration: 20251212170000_create_action_logs.sql

CREATE TABLE IF NOT EXISTS action_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Action Information
    action_type VARCHAR(50) NOT NULL,
    component VARCHAR(20) NOT NULL CHECK (component IN ('backend', 'orchestrator')),
    
    -- Result & Error Tracking
    status VARCHAR(20) NOT NULL CHECK (status IN ('success', 'failed', 'in_progress')),
    error_code VARCHAR(50),
    error_message TEXT,
    
    -- Context
    instance_id UUID REFERENCES instances(id) ON DELETE SET NULL,
    user_id VARCHAR(100),
    request_payload JSONB,
    response_payload JSONB,
    
    -- Metadata
    duration_ms INTEGER,
    source_ip VARCHAR(45),
    
    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_action_logs_created_at ON action_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_action_logs_instance_id ON action_logs(instance_id) WHERE instance_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_action_logs_component_status ON action_logs(component, status);
CREATE INDEX IF NOT EXISTS idx_action_logs_action_type ON action_logs(action_type);
CREATE INDEX IF NOT EXISTS idx_action_logs_status ON action_logs(status) WHERE status != 'success';

-- Comment for documentation
COMMENT ON TABLE action_logs IS 'Audit log tracking all backend and orchestrator actions with results and errors';
COMMENT ON COLUMN action_logs.action_type IS 'Type of action: CREATE_INSTANCE, TERMINATE_INSTANCE, HEALTH_CHECK, etc.';
COMMENT ON COLUMN action_logs.component IS 'Which component performed the action: backend or orchestrator';
COMMENT ON COLUMN action_logs.duration_ms IS 'How long the action took to complete in milliseconds';
