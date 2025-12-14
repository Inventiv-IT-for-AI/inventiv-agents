-- Add reconciliation & deletion tracking columns to instances table
-- Migration: 20251214120000_add_reconciliation_tracking.sql

ALTER TABLE instances
  ADD COLUMN IF NOT EXISTS last_reconciliation TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS deletion_reason TEXT,
  ADD COLUMN IF NOT EXISTS deleted_by_provider BOOLEAN DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_instances_last_reconciliation
  ON instances(last_reconciliation)
  WHERE last_reconciliation IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_instances_deleted_by_provider
  ON instances(deleted_by_provider)
  WHERE deleted_by_provider IS TRUE;

