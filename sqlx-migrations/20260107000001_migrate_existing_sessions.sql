-- Migration: Migrate existing sessions from users.current_organization_id to user_sessions
-- This migration is idempotent and safe to run multiple times
-- It only migrates if users.current_organization_id column still exists (legacy data)

-- Check if users.current_organization_id column exists before migrating
DO $$
BEGIN
    -- Only migrate if the column exists (legacy installations)
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'users' AND column_name = 'current_organization_id'
    ) THEN
        -- Insert legacy sessions for users who have a current_organization_id
        INSERT INTO user_sessions (
            user_id,
            current_organization_id,
            organization_role,
            session_token_hash,
            ip_address,
            user_agent,
            created_at,
            last_used_at,
            expires_at
        )
        SELECT
            u.id,
            u.current_organization_id,
            om.role,
            -- Generate a placeholder hash (will be updated on next login)
            encode(digest(gen_random_uuid()::text || u.id::text, 'sha256'), 'hex'),
            NULL::inet,  -- IP address unknown for legacy sessions
            'Legacy Migration',  -- Placeholder user agent
            NOW(),
            NOW(),
            NOW() + INTERVAL '12 hours'  -- Default TTL
        FROM users u
        LEFT JOIN organization_memberships om 
            ON om.organization_id = u.current_organization_id 
            AND om.user_id = u.id
        WHERE u.current_organization_id IS NOT NULL
          AND NOT EXISTS (
            SELECT 1 FROM user_sessions us 
            WHERE us.user_id = u.id 
              AND us.current_organization_id = u.current_organization_id
              AND us.revoked_at IS NULL
          );

        -- Also create sessions for users without an organization (Personal mode)
        INSERT INTO user_sessions (
            user_id,
            current_organization_id,
            organization_role,
            session_token_hash,
            ip_address,
            user_agent,
            created_at,
            last_used_at,
            expires_at
        )
        SELECT
            u.id,
            NULL,
            NULL,
            encode(digest(gen_random_uuid()::text || u.id::text, 'sha256'), 'hex'),
            NULL::inet,
            'Legacy Migration',
            NOW(),
            NOW(),
            NOW() + INTERVAL '12 hours'
        FROM users u
        WHERE u.current_organization_id IS NULL
            AND NOT EXISTS (
                SELECT 1 FROM user_sessions us 
                WHERE us.user_id = u.id AND us.revoked_at IS NULL
            );
    END IF;
END $$;

-- Comment
COMMENT ON TABLE user_sessions IS 'Active user sessions with organization context. Legacy sessions migrated from users.current_organization_id will be replaced on next login.';

