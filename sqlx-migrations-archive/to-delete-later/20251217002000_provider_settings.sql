-- Provider-scoped settings (simple key/value store).
-- Used for tuning provisioning/boot parameters without redeploying.

CREATE TABLE IF NOT EXISTS public.provider_settings (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_id uuid NOT NULL REFERENCES public.providers(id) ON DELETE CASCADE,
    key text NOT NULL,
    value_int bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT provider_settings_pkey PRIMARY KEY (id),
    CONSTRAINT provider_settings_provider_key_uniq UNIQUE (provider_id, key),
    CONSTRAINT provider_settings_key_allowed CHECK (
        key IN (
            'WORKER_INSTANCE_STARTUP_TIMEOUT_S',
            'INSTANCE_STARTUP_TIMEOUT_S'
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_provider_settings_provider_id ON public.provider_settings(provider_id);

-- Keep updated_at in sync
CREATE OR REPLACE FUNCTION public.set_updated_at_provider_settings()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_provider_settings_updated_at ON public.provider_settings;
CREATE TRIGGER trg_provider_settings_updated_at
BEFORE UPDATE ON public.provider_settings
FOR EACH ROW
EXECUTE FUNCTION public.set_updated_at_provider_settings();


