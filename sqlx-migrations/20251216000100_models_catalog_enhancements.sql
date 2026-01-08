-- Enhance models catalog so it can be managed dynamically from UI/API.
-- Baseline already has table `models` (id, name, model_id, required_vram_gb, context_length, created_at, updated_at).

ALTER TABLE public.models
  ADD COLUMN IF NOT EXISTS is_active boolean DEFAULT true NOT NULL;

-- Recommended data volume size (GB) to allocate for workers running this model.
-- Used by orchestrator as a safer, model-driven sizing (fallback remains possible).
ALTER TABLE public.models
  ADD COLUMN IF NOT EXISTS data_volume_gb bigint;

-- Free-form metadata for future needs (router params, tags, etc.)
ALTER TABLE public.models
  ADD COLUMN IF NOT EXISTS metadata jsonb DEFAULT '{}'::jsonb NOT NULL;

CREATE INDEX IF NOT EXISTS idx_models_is_active ON public.models(is_active);






