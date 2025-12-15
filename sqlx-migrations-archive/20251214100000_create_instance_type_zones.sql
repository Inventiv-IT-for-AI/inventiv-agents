-- Create instance_type_zones table for zone-based instance type availability
-- This table drives the UI filtering (zones -> instance types) and settings associations.

CREATE TABLE IF NOT EXISTS instance_type_zones (
    instance_type_id UUID NOT NULL REFERENCES instance_types(id) ON DELETE CASCADE,
    zone_id UUID NOT NULL REFERENCES zones(id) ON DELETE CASCADE,
    is_available BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_type_id, zone_id)
);

-- Useful indexes
CREATE INDEX IF NOT EXISTS idx_instance_type_zones_zone_id ON instance_type_zones(zone_id);
CREATE INDEX IF NOT EXISTS idx_instance_type_zones_instance_type_id ON instance_type_zones(instance_type_id);


