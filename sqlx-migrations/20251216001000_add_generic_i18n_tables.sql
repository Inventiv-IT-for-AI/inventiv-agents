-- Generic DB i18n model:
-- - i18n_keys: opaque identifiers referenced by *_i18n_id columns
-- - i18n_texts: translated texts per (key_id, locale_code)
-- Fallback order: en-US -> fr-FR -> ar (implemented as helper function)

CREATE TABLE IF NOT EXISTS public.i18n_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS public.i18n_texts (
  key_id UUID NOT NULL,
  locale_code TEXT NOT NULL,
  text_value TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (key_id, locale_code),
  CONSTRAINT i18n_texts_key_id_fkey FOREIGN KEY (key_id) REFERENCES public.i18n_keys(id) ON DELETE CASCADE,
  CONSTRAINT i18n_texts_locale_code_fkey FOREIGN KEY (locale_code) REFERENCES public.locales(code)
);

CREATE INDEX IF NOT EXISTS idx_i18n_texts_locale_code ON public.i18n_texts(locale_code);

-- Helper function to resolve text with fallback.
CREATE OR REPLACE FUNCTION public.i18n_get_text(
  p_key_id UUID,
  p_preferred_locale TEXT,
  p_fallback_locales TEXT[] DEFAULT ARRAY['en-US','fr-FR','ar']::TEXT[]
)
RETURNS TEXT
LANGUAGE sql
STABLE
AS $$
  WITH wanted AS (
    SELECT p_preferred_locale AS locale_code, 0 AS ord
    UNION ALL
    SELECT unnest(p_fallback_locales) AS locale_code, 1 + generate_subscripts(p_fallback_locales, 1) AS ord
  )
  SELECT t.text_value
  FROM wanted w
  JOIN public.i18n_texts t
    ON t.key_id = p_key_id
   AND t.locale_code = w.locale_code
  ORDER BY w.ord ASC
  LIMIT 1
$$;


