-- Cleanup legacy Scaleway regions/zones that can lead to ambiguous zone resolution.
--
-- Context:
-- - In this project, zone codes like "fr-par-2" are used by the UI/API.
-- - DB uniqueness is (region_id, zone_code), so stale/legacy regions may still contain zones
--   with the same zone_code, creating duplicates for a provider.
-- - We do NOT delete rows (safe for audit/history + future multi-tenant evolution).
--   We only deactivate legacy rows and mark availability mappings unavailable.

-- Keep only the official region codes used by our catalog seed.
WITH scw AS (
  SELECT id FROM providers WHERE code = 'scaleway' LIMIT 1
),
legacy_regions AS (
  SELECT r.id
  FROM regions r
  JOIN scw ON scw.id = r.provider_id
  WHERE r.code NOT IN ('fr-par', 'nl-ams', 'pl-waw')
)
UPDATE regions
SET is_active = false
WHERE id IN (SELECT id FROM legacy_regions);

-- Deactivate zones attached to legacy regions
UPDATE zones
SET is_active = false
WHERE region_id IN (
  SELECT r.id
  FROM regions r
  JOIN providers p ON p.id = r.provider_id
  WHERE p.code = 'scaleway'
    AND r.code NOT IN ('fr-par', 'nl-ams', 'pl-waw')
);

-- Also mark availability mappings unavailable for inactive zones/regions to avoid UI/validation surprises.
UPDATE instance_type_zones itz
SET is_available = false
WHERE itz.zone_id IN (
  SELECT z.id
  FROM zones z
  JOIN regions r ON r.id = z.region_id
  JOIN providers p ON p.id = r.provider_id
  WHERE p.code = 'scaleway'
    AND (z.is_active = false OR r.is_active = false)
);


