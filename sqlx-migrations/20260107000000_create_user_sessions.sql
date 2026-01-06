-- Migration: Create user_sessions table for multi-session support
-- This allows users to have multiple active sessions with different organizations

CREATE TABLE IF NOT EXISTS public.user_sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Session context (organization and role)
    current_organization_id uuid REFERENCES organizations(id) ON DELETE SET NULL,
    organization_role text CHECK (organization_role IN ('owner', 'admin', 'manager', 'user')),
    
    -- Security & tracking
    session_token_hash text NOT NULL,  -- SHA256 hash of the JWT token (for revocation)
    ip_address inet,
    user_agent text,
    
    -- Lifecycle
    created_at timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,  -- Soft delete for audit trail
    
    -- Constraints
    CONSTRAINT user_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT user_sessions_org_id_fkey FOREIGN KEY (current_organization_id) REFERENCES organizations(id) ON DELETE SET NULL,
    CONSTRAINT user_sessions_org_role_check CHECK (organization_role IN ('owner', 'admin', 'manager', 'user'))
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_user_sessions_user_id ON user_sessions(user_id) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_user_sessions_token_hash ON user_sessions(session_token_hash) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_user_sessions_expires_at ON user_sessions(expires_at) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_user_sessions_org_id ON user_sessions(current_organization_id) WHERE revoked_at IS NULL;

-- Comment
COMMENT ON TABLE user_sessions IS 'Active user sessions with organization context. Allows multiple sessions per user with different organizations.';
COMMENT ON COLUMN user_sessions.session_token_hash IS 'SHA256 hash of the JWT token for secure revocation';
COMMENT ON COLUMN user_sessions.organization_role IS 'Role of the user in the current organization (resolved from organization_memberships)';

