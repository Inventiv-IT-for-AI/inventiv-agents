-- DEV ONLY: reset catalog & dependent data
-- Source of truth for catalog: seeds/catalog_seeds.sql (do not duplicate catalog here)

BEGIN;

-- Truncate providers; CASCADE will also clear dependent catalog + anything FK-linked
-- (regions, zones, instance_types, instance_type_zones, instances, etc.)
TRUNCATE TABLE providers CASCADE;

COMMIT;

