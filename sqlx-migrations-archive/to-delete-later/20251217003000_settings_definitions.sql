-- Settings definitions catalog (min/max/default/description).
-- Seeded in seeds/catalog_seeds.sql so the UI can display metadata and the DB can validate values.

CREATE TABLE IF NOT EXISTS public.settings_definitions (
    key text PRIMARY KEY,
    scope text NOT NULL DEFAULT 'provider', -- provider | global (future)
    value_type text NOT NULL DEFAULT 'int', -- int | bool | text | json (future)
    min_int bigint,
    max_int bigint,
    default_int bigint,
    description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE OR REPLACE FUNCTION public.set_updated_at_settings_definitions()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_settings_definitions_updated_at ON public.settings_definitions;
CREATE TRIGGER trg_settings_definitions_updated_at
BEFORE UPDATE ON public.settings_definitions
FOR EACH ROW
EXECUTE FUNCTION public.set_updated_at_settings_definitions();

-- Relax provider_settings check constraint (keys should come from settings_definitions instead).
ALTER TABLE public.provider_settings
  DROP CONSTRAINT IF EXISTS provider_settings_key_allowed;

-- Enforce allowed keys via FK to definitions.
ALTER TABLE public.provider_settings
  ADD CONSTRAINT provider_settings_key_fk
  FOREIGN KEY (key) REFERENCES public.settings_definitions(key)
  ON DELETE RESTRICT;

-- Validate provider_settings values against definitions (min/max + type).
CREATE OR REPLACE FUNCTION public.validate_provider_settings()
RETURNS TRIGGER AS $$
DECLARE
  def record;
BEGIN
  SELECT * INTO def FROM public.settings_definitions WHERE key = NEW.key;
  IF def IS NULL THEN
    RAISE EXCEPTION 'Unknown setting key: %', NEW.key;
  END IF;

  IF def.value_type = 'int' THEN
    IF NEW.value_int IS NULL THEN
      RAISE EXCEPTION 'Setting % requires value_int', NEW.key;
    END IF;
    IF def.min_int IS NOT NULL AND NEW.value_int < def.min_int THEN
      RAISE EXCEPTION 'Setting % out of range (min=%)', NEW.key, def.min_int;
    END IF;
    IF def.max_int IS NOT NULL AND NEW.value_int > def.max_int THEN
      RAISE EXCEPTION 'Setting % out of range (max=%)', NEW.key, def.max_int;
    END IF;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_provider_settings_validate ON public.provider_settings;
CREATE TRIGGER trg_provider_settings_validate
BEFORE INSERT OR UPDATE ON public.provider_settings
FOR EACH ROW
EXECUTE FUNCTION public.validate_provider_settings();


