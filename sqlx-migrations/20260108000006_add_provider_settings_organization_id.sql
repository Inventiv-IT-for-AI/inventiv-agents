-- Migration: Add organization_id to provider_settings for multi-tenant support
-- Each organization has its own provider credentials and settings
-- No backward compatibility needed (DB is regularly reset)

-- Add organization_id column (NOT NULL - all provider_settings must belong to an organization)
ALTER TABLE provider_settings 
  ADD COLUMN organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE;

-- Unique constraint: (provider_id, key, organization_id)
-- Allows each organization to have its own settings for each provider
ALTER TABLE provider_settings 
  ADD CONSTRAINT provider_settings_provider_key_org_uniq 
  UNIQUE (provider_id, key, organization_id);

-- Indexes for performance (filtering by organization)
CREATE INDEX idx_provider_settings_org ON provider_settings(organization_id);
CREATE INDEX idx_provider_settings_provider_org ON provider_settings(provider_id, organization_id);

