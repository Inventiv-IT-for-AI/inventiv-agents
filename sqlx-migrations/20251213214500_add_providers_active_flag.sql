-- Add is_active flag to providers table
ALTER TABLE providers ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE;


