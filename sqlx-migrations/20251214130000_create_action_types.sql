-- Create action_types catalog (code -> label/icon/color) to avoid frontend desync
-- Migration: 20251214130000_create_action_types.sql

CREATE TABLE IF NOT EXISTS action_types (
    code VARCHAR(80) PRIMARY KEY,
    label VARCHAR(120) NOT NULL,
    icon VARCHAR(60) NOT NULL,         -- Lucide icon name (e.g. "Zap", "Cloud")
    color_class VARCHAR(160) NOT NULL, -- Tailwind classes for badge (kept in DB to avoid desync)
    category VARCHAR(50),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed / upsert known action types
INSERT INTO action_types (code, label, icon, color_class, category, is_active) VALUES
  -- Create workflow
  ('REQUEST_CREATE', 'Request Create', 'Zap', 'bg-blue-500 hover:bg-blue-600 text-white', 'create', TRUE),
  ('EXECUTE_CREATE', 'Execute Create', 'Server', 'bg-purple-500 hover:bg-purple-600 text-white', 'create', TRUE),
  ('PROVIDER_CREATE', 'Provider Create', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'create', TRUE),
  ('PERSIST_PROVIDER_ID', 'Persist Provider ID', 'Database', 'bg-indigo-500 hover:bg-indigo-600 text-white', 'create', TRUE),
  ('PROVIDER_START', 'Provider Start', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'create', TRUE),
  ('PROVIDER_GET_IP', 'Provider Get IP', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'create', TRUE),
  ('INSTANCE_CREATED', 'Instance Created', 'Database', 'bg-green-500 hover:bg-green-600 text-white', 'create', TRUE),
  ('HEALTH_CHECK', 'Health Check', 'Clock', 'bg-teal-600 hover:bg-teal-700 text-white', 'health', TRUE),
  ('INSTANCE_READY', 'Instance Ready', 'CheckCircle', 'bg-green-600 hover:bg-green-700 text-white', 'health', TRUE),
  ('INSTANCE_STARTUP_FAILED', 'Instance Startup Failed', 'AlertTriangle', 'bg-gray-600 hover:bg-gray-700 text-white', 'health', TRUE),

  -- Termination workflow
  ('REQUEST_TERMINATE', 'Request Terminate', 'Zap', 'bg-blue-600 hover:bg-blue-700 text-white', 'terminate', TRUE),
  ('EXECUTE_TERMINATE', 'Execute Terminate', 'Server', 'bg-purple-600 hover:bg-purple-700 text-white', 'terminate', TRUE),
  ('PROVIDER_TERMINATE', 'Provider Terminate', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'terminate', TRUE),
  ('TERMINATION_PENDING', 'Termination Pending', 'Clock', 'bg-yellow-500 hover:bg-yellow-600 text-white', 'terminate', TRUE),
  ('TERMINATOR_RETRY', 'Terminator Retry', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'terminate', TRUE),
  ('TERMINATION_CONFIRMED', 'Termination Confirmed', 'Database', 'bg-red-500 hover:bg-red-600 text-white', 'terminate', TRUE),
  ('INSTANCE_TERMINATED', 'Instance Terminated', 'Database', 'bg-red-500 hover:bg-red-600 text-white', 'terminate', TRUE),

  -- Archive / reconciliation
  ('ARCHIVE_INSTANCE', 'Archive Instance', 'Archive', 'bg-gray-600 hover:bg-gray-700 text-white', 'archive', TRUE),
  ('PROVIDER_DELETED_DETECTED', 'Provider Deleted', 'AlertTriangle', 'bg-yellow-600 hover:bg-yellow-700 text-white', 'reconcile', TRUE),

  -- Legacy (keep active for display)
  ('TERMINATE_INSTANCE', 'Terminate Instance', 'Server', 'bg-purple-600 hover:bg-purple-700 text-white', 'legacy', TRUE),
  ('SCALEWAY_CREATE', 'Provider Create', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'legacy', TRUE),
  ('SCALEWAY_DELETE', 'Provider Delete', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'legacy', TRUE)
ON CONFLICT (code) DO UPDATE SET
  label = EXCLUDED.label,
  icon = EXCLUDED.icon,
  color_class = EXCLUDED.color_class,
  category = EXCLUDED.category,
  is_active = EXCLUDED.is_active,
  updated_at = NOW();

