-- Add instance status before/after to action_logs for better auditing
-- Migration: 20251214133000_add_action_logs_instance_statuses.sql

ALTER TABLE action_logs
  ADD COLUMN IF NOT EXISTS instance_status_before VARCHAR(50),
  ADD COLUMN IF NOT EXISTS instance_status_after  VARCHAR(50);

CREATE INDEX IF NOT EXISTS idx_action_logs_instance_status_after
  ON action_logs(instance_status_after)
  WHERE instance_status_after IS NOT NULL;

