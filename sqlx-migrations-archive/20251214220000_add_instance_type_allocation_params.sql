-- Add provider-specific allocation parameters to instance types.
--
-- This is used by the orchestrator to handle provider constraints that depend on the instance type,
-- e.g. Scaleway L4 instances requiring a specific boot image / storage profile.

ALTER TABLE instance_types
  ADD COLUMN IF NOT EXISTS allocation_params JSONB NOT NULL DEFAULT '{}'::jsonb;

