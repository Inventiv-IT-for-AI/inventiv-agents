-- FinOps raw events store (append-only)
-- Migration: 20251214162000_create_finops_events_table.sql

CREATE SCHEMA IF NOT EXISTS finops;

CREATE TABLE IF NOT EXISTS finops.events (
  event_id UUID PRIMARY KEY,
  occurred_at TIMESTAMPTZ NOT NULL,
  event_type TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'unknown',
  payload JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_finops_events_occurred_at
  ON finops.events(occurred_at);

CREATE INDEX IF NOT EXISTS idx_finops_events_type_time
  ON finops.events(event_type, occurred_at);
