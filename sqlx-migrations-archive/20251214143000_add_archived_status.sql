-- Add archived status to instance_status enum
-- Migration: 20251214143000_add_archived_status.sql

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_enum e
    JOIN pg_type t ON t.oid = e.enumtypid
    WHERE t.typname = 'instance_status'
      AND e.enumlabel = 'archived'
  ) THEN
    ALTER TYPE instance_status ADD VALUE 'archived';
  END IF;
END $$;

