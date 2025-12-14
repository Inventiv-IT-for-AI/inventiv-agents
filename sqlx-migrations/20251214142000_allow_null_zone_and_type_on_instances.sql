-- Allow persisting failed creation requests with an instance_id,
-- even when catalog validation fails (zone/type missing or inactive).
-- Migration: 20251214142000_allow_null_zone_and_type_on_instances.sql

ALTER TABLE instances
  ALTER COLUMN zone_id DROP NOT NULL,
  ALTER COLUMN instance_type_id DROP NOT NULL;

