-- Extend provider_settings to support typed values.

ALTER TABLE public.provider_settings
  ADD COLUMN IF NOT EXISTS value_bool boolean,
  ADD COLUMN IF NOT EXISTS value_text text,
  ADD COLUMN IF NOT EXISTS value_json jsonb;

-- Update validation function to support int/bool/text/json types.
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
  ELSIF def.value_type = 'bool' THEN
    IF NEW.value_bool IS NULL THEN
      RAISE EXCEPTION 'Setting % requires value_bool', NEW.key;
    END IF;
  ELSIF def.value_type = 'text' THEN
    IF NEW.value_text IS NULL OR btrim(NEW.value_text) = '' THEN
      RAISE EXCEPTION 'Setting % requires value_text', NEW.key;
    END IF;
  ELSIF def.value_type = 'json' THEN
    IF NEW.value_json IS NULL THEN
      RAISE EXCEPTION 'Setting % requires value_json', NEW.key;
    END IF;
  ELSE
    RAISE EXCEPTION 'Unsupported value_type for setting %: %', NEW.key, def.value_type;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;


