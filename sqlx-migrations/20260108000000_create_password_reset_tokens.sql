-- Table for password reset tokens
CREATE TABLE IF NOT EXISTS public.password_reset_tokens (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL,
    token_hash text NOT NULL,  -- SHA256 hash of the token
    expires_at timestamptz NOT NULL,
    used_at timestamptz,  -- NULL until token is used
    created_at timestamptz NOT NULL DEFAULT now()
);

-- Add foreign key constraint separately (idempotent)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'password_reset_tokens_user_id_fkey'
    ) THEN
        ALTER TABLE public.password_reset_tokens
        ADD CONSTRAINT password_reset_tokens_user_id_fkey 
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
    END IF;
END $$;

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_password_reset_tokens_user_id ON password_reset_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_password_reset_tokens_token_hash ON password_reset_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_password_reset_tokens_expires_at ON password_reset_tokens(expires_at) WHERE used_at IS NULL;

-- Comment
COMMENT ON TABLE password_reset_tokens IS 'Password reset tokens with expiration. Tokens are hashed for security.';
COMMENT ON COLUMN password_reset_tokens.token_hash IS 'SHA256 hash of the reset token';
COMMENT ON COLUMN password_reset_tokens.used_at IS 'Timestamp when token was used (NULL = unused)';

