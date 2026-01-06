-- Migration: Remove current_organization_id from users table
-- This column is now managed in user_sessions table to support multi-session
-- This migration is idempotent and safe to run multiple times

-- Only drop the column if it exists (legacy installations)
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'users' AND column_name = 'current_organization_id'
    ) THEN
        ALTER TABLE public.users DROP COLUMN current_organization_id;
    END IF;
END $$;

-- Comment
COMMENT ON TABLE users IS 'User accounts. Organization context is managed in user_sessions table to support multiple concurrent sessions.';

