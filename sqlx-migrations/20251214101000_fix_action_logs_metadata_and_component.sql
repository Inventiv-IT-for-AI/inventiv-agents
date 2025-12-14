-- Fix action_logs schema to match current API/Orchestrator loggers
-- - add metadata column (JSONB)
-- - allow component = 'api' (code uses 'api', docs sometimes say 'backend')

ALTER TABLE action_logs
    ADD COLUMN IF NOT EXISTS metadata JSONB;

-- Fix/relax component check constraint (drop if present, recreate)
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'action_logs_component_check'
    ) THEN
        ALTER TABLE action_logs DROP CONSTRAINT action_logs_component_check;
    END IF;
END $$;

ALTER TABLE action_logs
    ADD CONSTRAINT action_logs_component_check
    CHECK (component IN ('api', 'backend', 'orchestrator'));


