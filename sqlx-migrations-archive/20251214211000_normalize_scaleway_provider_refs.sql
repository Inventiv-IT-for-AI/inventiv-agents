-- Normalize legacy duplicate "Scaleway" providers so catalog references are consistent.
--
-- Problem observed in UI:
-- - instance_types rows referencing different provider UUIDs for the same logical provider ("scaleway")
--
-- Strategy:
-- - Pick a canonical provider id for code/name "scaleway" (prefer providers.code='scaleway', else providers.name ILIKE 'scaleway')
-- - Re-point instance_types/instances to the canonical provider id
-- - If instance_types would conflict on (provider_id, code), merge references onto the canonical row and delete the duplicate
-- - Deactivate legacy provider rows (so they don't appear in Settings)
--
-- Notes:
-- - This is idempotent.
-- - We intentionally do not touch regions/zones in this migration (lower risk).

DO $$
DECLARE
  canonical_provider_id uuid;
BEGIN
  -- Ensure required columns exist (older DBs / dev resets)
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'providers' AND column_name = 'code'
  ) THEN
    ALTER TABLE providers ADD COLUMN code VARCHAR(50);
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'providers' AND column_name = 'is_active'
  ) THEN
    ALTER TABLE providers ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
  END IF;

  -- Backfill code if missing
  UPDATE providers
  SET code = lower(regexp_replace(name, '[^a-zA-Z0-9]+', '_', 'g'))
  WHERE code IS NULL;

  -- Pick canonical provider id
  SELECT id INTO canonical_provider_id
  FROM providers
  WHERE code = 'scaleway'
  ORDER BY id
  LIMIT 1;

  IF canonical_provider_id IS NULL THEN
    SELECT id INTO canonical_provider_id
    FROM providers
    WHERE lower(name) LIKE 'scaleway%'
    ORDER BY id
    LIMIT 1;
  END IF;

  -- If still missing, do nothing: catalog is seeded exclusively via `seeds/catalog_seeds.sql`.
  IF canonical_provider_id IS NULL THEN
    RETURN;
  END IF;

  -- Ensure canonical row uses code='scaleway' and is active
  UPDATE providers
  SET code = 'scaleway',
      is_active = TRUE
  WHERE id = canonical_provider_id;

  ---------------------------------------------------------------------------
  -- Merge instance_types duplicates created under legacy provider ids
  ---------------------------------------------------------------------------

  -- Ensure instance_types.code exists (older DBs)
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'instance_types' AND column_name = 'code'
  ) THEN
    ALTER TABLE instance_types ADD COLUMN code VARCHAR(50);
    UPDATE instance_types SET code = name WHERE code IS NULL;
    ALTER TABLE instance_types ALTER COLUMN code SET NOT NULL;
  END IF;

  -- 1) For legacy provider rows that look like scaleway, merge types by code
  WITH legacy_providers AS (
    SELECT id
    FROM providers
    WHERE id <> canonical_provider_id
      AND (code = 'scaleway' OR lower(code) LIKE 'scaleway%' OR lower(name) LIKE 'scaleway%')
  ),
  canonical_types AS (
    SELECT id, code
    FROM instance_types
    WHERE provider_id = canonical_provider_id
  ),
  legacy_types AS (
    SELECT id, code
    FROM instance_types
    WHERE provider_id IN (SELECT id FROM legacy_providers)
  ),
  conflicts AS (
    SELECT lt.id AS legacy_type_id, ct.id AS canonical_type_id
    FROM legacy_types lt
    JOIN canonical_types ct ON ct.code = lt.code
  )
  -- instances: point to canonical type
  UPDATE instances i
  SET instance_type_id = c.canonical_type_id
  FROM conflicts c
  WHERE i.instance_type_id = c.legacy_type_id;

  -- instance_type_zones: merge availability then remove legacy
  WITH legacy_providers AS (
    SELECT id
    FROM providers
    WHERE id <> canonical_provider_id
      AND (code = 'scaleway' OR lower(code) LIKE 'scaleway%' OR lower(name) LIKE 'scaleway%')
  ),
  canonical_types AS (
    SELECT id, code
    FROM instance_types
    WHERE provider_id = canonical_provider_id
  ),
  legacy_types AS (
    SELECT id, code
    FROM instance_types
    WHERE provider_id IN (SELECT id FROM legacy_providers)
  ),
  conflicts AS (
    SELECT lt.id AS legacy_type_id, ct.id AS canonical_type_id
    FROM legacy_types lt
    JOIN canonical_types ct ON ct.code = lt.code
  )
  INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available)
  SELECT c.canonical_type_id, itz.zone_id, bool_or(itz.is_available) AS is_available
  FROM conflicts c
  JOIN instance_type_zones itz ON itz.instance_type_id = c.legacy_type_id
  GROUP BY c.canonical_type_id, itz.zone_id
  ON CONFLICT (instance_type_id, zone_id)
  DO UPDATE SET is_available = EXCLUDED.is_available;

  WITH legacy_providers AS (
    SELECT id
    FROM providers
    WHERE id <> canonical_provider_id
      AND (code = 'scaleway' OR lower(code) LIKE 'scaleway%' OR lower(name) LIKE 'scaleway%')
  ),
  canonical_types AS (
    SELECT id, code
    FROM instance_types
    WHERE provider_id = canonical_provider_id
  ),
  legacy_types AS (
    SELECT id, code
    FROM instance_types
    WHERE provider_id IN (SELECT id FROM legacy_providers)
  ),
  conflicts AS (
    SELECT lt.id AS legacy_type_id, ct.id AS canonical_type_id
    FROM legacy_types lt
    JOIN canonical_types ct ON ct.code = lt.code
  )
  DELETE FROM instance_type_zones itz
  WHERE itz.instance_type_id IN (SELECT legacy_type_id FROM conflicts);

  WITH legacy_providers AS (
    SELECT id
    FROM providers
    WHERE id <> canonical_provider_id
      AND (code = 'scaleway' OR lower(code) LIKE 'scaleway%' OR lower(name) LIKE 'scaleway%')
  ),
  canonical_types AS (
    SELECT id, code
    FROM instance_types
    WHERE provider_id = canonical_provider_id
  ),
  legacy_types AS (
    SELECT id, code
    FROM instance_types
    WHERE provider_id IN (SELECT id FROM legacy_providers)
  ),
  conflicts AS (
    SELECT lt.id AS legacy_type_id, ct.id AS canonical_type_id
    FROM legacy_types lt
    JOIN canonical_types ct ON ct.code = lt.code
  )
  DELETE FROM instance_types it
  WHERE it.id IN (SELECT legacy_type_id FROM conflicts);

  -- 2) Re-point any remaining legacy instance_types provider_id -> canonical provider_id
  UPDATE instance_types
  SET provider_id = canonical_provider_id
  WHERE provider_id IN (
    SELECT id
    FROM providers
    WHERE id <> canonical_provider_id
      AND (code = 'scaleway' OR lower(code) LIKE 'scaleway%' OR lower(name) LIKE 'scaleway%')
  );

  -- 3) Re-point instances.provider_id as well (so joins show one provider)
  UPDATE instances
  SET provider_id = canonical_provider_id
  WHERE provider_id IN (
    SELECT id
    FROM providers
    WHERE id <> canonical_provider_id
      AND (code = 'scaleway' OR lower(code) LIKE 'scaleway%' OR lower(name) LIKE 'scaleway%')
  );

  -- 4) Deactivate legacy providers so Settings doesn't show duplicates
  UPDATE providers
  SET is_active = FALSE
  WHERE id <> canonical_provider_id
    AND (code = 'scaleway' OR lower(code) LIKE 'scaleway%' OR lower(name) LIKE 'scaleway%');

END $$;

