-- Workbench persistence: runs + messages
-- Supports internal UI usage and external API-key driven clients.

CREATE TABLE IF NOT EXISTS public.workbench_runs (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  created_at timestamptz NOT NULL DEFAULT now(),
  started_at timestamptz NOT NULL DEFAULT now(),
  completed_at timestamptz,

  -- Actor (either a logged-in user, or an API key principal)
  created_by_user_id uuid REFERENCES public.users(id) ON DELETE SET NULL,
  created_via_api_key_id uuid REFERENCES public.api_keys(id) ON DELETE SET NULL,

  -- OpenAI / vLLM model id (HF repo id or other identifier accepted by /v1/*)
  model_id text NOT NULL,

  -- chat | validation | batch (future-proof; UI starts with chat)
  mode text NOT NULL DEFAULT 'chat',

  -- in_progress | success | failed | cancelled
  status text NOT NULL DEFAULT 'in_progress',

  -- Metrics (best-effort from UI/proxy)
  ttft_ms integer,
  duration_ms integer,

  error_message text,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb
);

-- Basic invariants
ALTER TABLE public.workbench_runs
  DROP CONSTRAINT IF EXISTS workbench_runs_actor_check;
ALTER TABLE public.workbench_runs
  ADD CONSTRAINT workbench_runs_actor_check
  CHECK (
    (created_by_user_id IS NOT NULL) OR (created_via_api_key_id IS NOT NULL)
  );

ALTER TABLE public.workbench_runs
  DROP CONSTRAINT IF EXISTS workbench_runs_status_check;
ALTER TABLE public.workbench_runs
  ADD CONSTRAINT workbench_runs_status_check
  CHECK (status IN ('in_progress','success','failed','cancelled'));

ALTER TABLE public.workbench_runs
  DROP CONSTRAINT IF EXISTS workbench_runs_mode_check;
ALTER TABLE public.workbench_runs
  ADD CONSTRAINT workbench_runs_mode_check
  CHECK (mode IN ('chat','validation','batch'));

CREATE INDEX IF NOT EXISTS idx_workbench_runs_created_at
  ON public.workbench_runs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_workbench_runs_user
  ON public.workbench_runs(created_by_user_id, created_at DESC)
  WHERE created_by_user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_workbench_runs_api_key
  ON public.workbench_runs(created_via_api_key_id, created_at DESC)
  WHERE created_via_api_key_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_workbench_runs_model
  ON public.workbench_runs(model_id, created_at DESC);

CREATE TABLE IF NOT EXISTS public.workbench_messages (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  run_id uuid NOT NULL REFERENCES public.workbench_runs(id) ON DELETE CASCADE,
  message_index integer NOT NULL,
  role text NOT NULL,
  content text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE public.workbench_messages
  DROP CONSTRAINT IF EXISTS workbench_messages_role_check;
ALTER TABLE public.workbench_messages
  ADD CONSTRAINT workbench_messages_role_check
  CHECK (role IN ('system','user','assistant'));

ALTER TABLE public.workbench_messages
  DROP CONSTRAINT IF EXISTS workbench_messages_run_index_uniq;
ALTER TABLE public.workbench_messages
  ADD CONSTRAINT workbench_messages_run_index_uniq UNIQUE (run_id, message_index);

CREATE INDEX IF NOT EXISTS idx_workbench_messages_run_created
  ON public.workbench_messages(run_id, created_at ASC);


