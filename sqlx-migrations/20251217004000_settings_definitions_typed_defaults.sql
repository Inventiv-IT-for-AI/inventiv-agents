-- Extend settings_definitions to support typed defaults.

ALTER TABLE public.settings_definitions
  ADD COLUMN IF NOT EXISTS default_bool boolean,
  ADD COLUMN IF NOT EXISTS default_text text,
  ADD COLUMN IF NOT EXISTS default_json jsonb;


