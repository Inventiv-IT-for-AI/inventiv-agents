-- Normalize catalog data strictly by `code` (no hardcoded UUIDs).
--
-- Goal:
-- - eliminate duplicate providers/regions/zones/instance_types created historically
-- - repoint all foreign keys to a single canonical row per (code scope)
--
-- Canonical choice rule:
-- - keep the smallest UUID (ORDER BY id) for determinism
--
-- This migration does NOT insert any catalog rows.

DO $$
BEGIN
  ---------------------------------------------------------------------------
  -- PROVIDERS: canonical per providers.code
  ---------------------------------------------------------------------------
  WITH prov_dups AS (
    SELECT code, MIN(id) AS keep_id
    FROM providers
    GROUP BY code
    HAVING COUNT(*) > 1
  ),
  prov_map AS (
    SELECT p.id AS drop_id, d.keep_id
    FROM providers p
    JOIN prov_dups d ON d.code = p.code
    WHERE p.id <> d.keep_id
  )
  UPDATE regions r
  SET provider_id = m.keep_id
  FROM prov_map m
  WHERE r.provider_id = m.drop_id;

  WITH prov_dups AS (
    SELECT code, MIN(id) AS keep_id
    FROM providers
    GROUP BY code
    HAVING COUNT(*) > 1
  ),
  prov_map AS (
    SELECT p.id AS drop_id, d.keep_id
    FROM providers p
    JOIN prov_dups d ON d.code = p.code
    WHERE p.id <> d.keep_id
  )
  UPDATE instance_types it
  SET provider_id = m.keep_id
  FROM prov_map m
  WHERE it.provider_id = m.drop_id;

  WITH prov_dups AS (
    SELECT code, MIN(id) AS keep_id
    FROM providers
    GROUP BY code
    HAVING COUNT(*) > 1
  ),
  prov_map AS (
    SELECT p.id AS drop_id, d.keep_id
    FROM providers p
    JOIN prov_dups d ON d.code = p.code
    WHERE p.id <> d.keep_id
  )
  UPDATE instances i
  SET provider_id = m.keep_id
  FROM prov_map m
  WHERE i.provider_id = m.drop_id;

  -- Deactivate duplicates (don't delete: avoid breaking historical references)
  WITH prov_dups AS (
    SELECT code, MIN(id) AS keep_id
    FROM providers
    GROUP BY code
    HAVING COUNT(*) > 1
  )
  UPDATE providers p
  SET is_active = FALSE
  FROM prov_dups d
  WHERE p.code = d.code
    AND p.id <> d.keep_id;

  ---------------------------------------------------------------------------
  -- REGIONS: canonical per (provider_id, regions.code)
  ---------------------------------------------------------------------------
  WITH region_dups AS (
    SELECT provider_id, code, MIN(id) AS keep_id
    FROM regions
    GROUP BY provider_id, code
    HAVING COUNT(*) > 1
  ),
  region_map AS (
    SELECT r.id AS drop_id, d.keep_id
    FROM regions r
    JOIN region_dups d ON d.provider_id = r.provider_id AND d.code = r.code
    WHERE r.id <> d.keep_id
  )
  UPDATE zones z
  SET region_id = m.keep_id
  FROM region_map m
  WHERE z.region_id = m.drop_id;

  -- Deactivate duplicates
  WITH region_dups AS (
    SELECT provider_id, code, MIN(id) AS keep_id
    FROM regions
    GROUP BY provider_id, code
    HAVING COUNT(*) > 1
  )
  UPDATE regions r
  SET is_active = FALSE
  FROM region_dups d
  WHERE r.provider_id = d.provider_id
    AND r.code = d.code
    AND r.id <> d.keep_id;

  ---------------------------------------------------------------------------
  -- ZONES: canonical per (region_id, zones.code)
  ---------------------------------------------------------------------------
  WITH zone_dups AS (
    SELECT region_id, code, MIN(id) AS keep_id
    FROM zones
    GROUP BY region_id, code
    HAVING COUNT(*) > 1
  ),
  zone_map AS (
    SELECT z.id AS drop_id, d.keep_id
    FROM zones z
    JOIN zone_dups d ON d.region_id = z.region_id AND d.code = z.code
    WHERE z.id <> d.keep_id
  )
  UPDATE instances i
  SET zone_id = m.keep_id
  FROM zone_map m
  WHERE i.zone_id = m.drop_id;

  WITH zone_dups AS (
    SELECT region_id, code, MIN(id) AS keep_id
    FROM zones
    GROUP BY region_id, code
    HAVING COUNT(*) > 1
  ),
  zone_map AS (
    SELECT z.id AS drop_id, d.keep_id
    FROM zones z
    JOIN zone_dups d ON d.region_id = z.region_id AND d.code = z.code
    WHERE z.id <> d.keep_id
  )
  UPDATE instance_type_zones itz
  SET zone_id = m.keep_id
  FROM zone_map m
  WHERE itz.zone_id = m.drop_id;

  -- Deactivate duplicates
  WITH zone_dups AS (
    SELECT region_id, code, MIN(id) AS keep_id
    FROM zones
    GROUP BY region_id, code
    HAVING COUNT(*) > 1
  )
  UPDATE zones z
  SET is_active = FALSE
  FROM zone_dups d
  WHERE z.region_id = d.region_id
    AND z.code = d.code
    AND z.id <> d.keep_id;

  ---------------------------------------------------------------------------
  -- INSTANCE TYPES: canonical per (provider_id, instance_types.code)
  -- merge instances + instance_type_zones then delete duplicates (safe under FK)
  ---------------------------------------------------------------------------

  -- Repoint instances.instance_type_id
  WITH type_dups AS (
    SELECT provider_id, code, MIN(id) AS keep_id
    FROM instance_types
    GROUP BY provider_id, code
    HAVING COUNT(*) > 1
  ),
  type_map AS (
    SELECT it.id AS drop_id, d.keep_id
    FROM instance_types it
    JOIN type_dups d ON d.provider_id = it.provider_id AND d.code = it.code
    WHERE it.id <> d.keep_id
  )
  UPDATE instances i
  SET instance_type_id = m.keep_id
  FROM type_map m
  WHERE i.instance_type_id = m.drop_id;

  -- Merge availability rows (OR semantics on is_available)
  WITH type_dups AS (
    SELECT provider_id, code, MIN(id) AS keep_id
    FROM instance_types
    GROUP BY provider_id, code
    HAVING COUNT(*) > 1
  ),
  type_map AS (
    SELECT it.id AS drop_id, d.keep_id
    FROM instance_types it
    JOIN type_dups d ON d.provider_id = it.provider_id AND d.code = it.code
    WHERE it.id <> d.keep_id
  )
  INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available)
  SELECT m.keep_id, itz.zone_id, bool_or(itz.is_available) AS is_available
  FROM instance_type_zones itz
  JOIN type_map m ON m.drop_id = itz.instance_type_id
  GROUP BY m.keep_id, itz.zone_id
  ON CONFLICT (instance_type_id, zone_id)
  DO UPDATE SET is_available = EXCLUDED.is_available;

  -- Remove duplicate availability rows pointing to drop_id
  WITH type_dups AS (
    SELECT provider_id, code, MIN(id) AS keep_id
    FROM instance_types
    GROUP BY provider_id, code
    HAVING COUNT(*) > 1
  ),
  type_map AS (
    SELECT it.id AS drop_id, d.keep_id
    FROM instance_types it
    JOIN type_dups d ON d.provider_id = it.provider_id AND d.code = it.code
    WHERE it.id <> d.keep_id
  )
  DELETE FROM instance_type_zones itz
  WHERE itz.instance_type_id IN (SELECT drop_id FROM type_map);

  -- Delete duplicate instance_types (FK already repointed)
  WITH type_dups AS (
    SELECT provider_id, code, MIN(id) AS keep_id
    FROM instance_types
    GROUP BY provider_id, code
    HAVING COUNT(*) > 1
  ),
  type_map AS (
    SELECT it.id AS drop_id, d.keep_id
    FROM instance_types it
    JOIN type_dups d ON d.provider_id = it.provider_id AND d.code = it.code
    WHERE it.id <> d.keep_id
  )
  DELETE FROM instance_types it
  WHERE it.id IN (SELECT drop_id FROM type_map);

END $$;

