-- Global settings (key/value), backed by settings_definitions where scope='global'.

CREATE TABLE IF NOT EXISTS public.global_settings (
    key text PRIMARY KEY REFERENCES public.settings_definitions(key) ON DELETE RESTRICT,
    value_int bigint,
    value_bool boolean,
    value_text text,
    value_json jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE OR REPLACE FUNCTION public.set_updated_at_global_settings()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_global_settings_updated_at ON public.global_settings;
CREATE TRIGGER trg_global_settings_updated_at
BEFORE UPDATE ON public.global_settings
FOR EACH ROW
EXECUTE FUNCTION public.set_updated_at_global_settings();

-- Validate global_settings values against definitions (min/max + type)
CREATE OR REPLACE FUNCTION public.validate_global_settings()
RETURNS TRIGGER AS $$
DECLARE
  def record;
BEGIN
  SELECT * INTO def FROM public.settings_definitions WHERE key = NEW.key;
  IF def IS NULL THEN
    RAISE EXCEPTION 'Unknown setting key: %', NEW.key;
  END IF;
  IF def.scope <> 'global' THEN
    RAISE EXCEPTION 'Setting % is not global (scope=%)', NEW.key, def.scope;
  END IF;

  IF def.value_type = 'int' THEN
    IF NEW.value_int IS NULL THEN RAISE EXCEPTION 'Setting % requires value_int', NEW.key; END IF;
    IF def.min_int IS NOT NULL AND NEW.value_int < def.min_int THEN RAISE EXCEPTION 'Setting % out of range (min=%)', NEW.key, def.min_int; END IF;
    IF def.max_int IS NOT NULL AND NEW.value_int > def.max_int THEN RAISE EXCEPTION 'Setting % out of range (max=%)', NEW.key, def.max_int; END IF;
  ELSIF def.value_type = 'bool' THEN
    IF NEW.value_bool IS NULL THEN RAISE EXCEPTION 'Setting % requires value_bool', NEW.key; END IF;
  ELSIF def.value_type = 'text' THEN
    IF NEW.value_text IS NULL OR btrim(NEW.value_text) = '' THEN RAISE EXCEPTION 'Setting % requires value_text', NEW.key; END IF;
  ELSIF def.value_type = 'json' THEN
    IF NEW.value_json IS NULL THEN RAISE EXCEPTION 'Setting % requires value_json', NEW.key; END IF;
  ELSE
    RAISE EXCEPTION 'Unsupported value_type for setting %: %', NEW.key, def.value_type;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_global_settings_validate ON public.global_settings;
CREATE TRIGGER trg_global_settings_validate
BEFORE INSERT OR UPDATE ON public.global_settings
FOR EACH ROW
EXECUTE FUNCTION public.validate_global_settings();


