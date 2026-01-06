--
-- Consolidated Database Schema
-- Generated from current database state (2025-12-31)
-- All previous migrations have been consolidated into this baseline
--
-- IMPORTANT: keep search_path including public so sqlx can access its _sqlx_migrations table.
SELECT pg_catalog.set_config('search_path', 'public', false);
CREATE EXTENSION IF NOT EXISTS timescaledb WITH SCHEMA public;

--
-- Name: EXTENSION timescaledb; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION timescaledb IS 'Enables scalable inserts and complex queries for time-series data (Community Edition)';

CREATE SCHEMA IF NOT EXISTS finops;

--
-- Name: pgcrypto; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;

--
-- Name: EXTENSION pgcrypto; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';

-- Create instance_status enum type if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'instance_status') THEN
        CREATE TYPE public.instance_status AS ENUM (
            'provisioning',
            'booting',
            'ready',
            'draining',
            'terminated',
            'failed',
            'startup_failed',
            'terminating',
            'provisioning_failed',
            'archived'
        );
    END IF;
END $$;

CREATE OR REPLACE FUNCTION public.check_model_instance_compatibility(p_model_id uuid, p_instance_type_id uuid) RETURNS boolean
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_model_vram_gb integer;

    v_instance_vram_total_gb integer;
    v_provider_code text;
    v_model_id text;
BEGIN
    SELECT required_vram_gb, model_id INTO v_model_vram_gb, v_model_id
    FROM models
    WHERE id = p_model_id;
    SELECT (it.gpu_count * it.vram_per_gpu_gb), p.code INTO v_instance_vram_total_gb, v_provider_code
    FROM instance_types it
    JOIN providers p ON p.id = it.provider_id
    WHERE it.id = p_instance_type_id;
    IF v_provider_code = 'mock' THEN
        RETURN (v_model_id = 'mock-echo-model');
    END IF;
    RETURN (v_instance_vram_total_gb >= v_model_vram_gb);
END;
$$;
CREATE OR REPLACE FUNCTION public.set_updated_at_global_settings() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
  NEW.updated_at = now();

  RETURN NEW;
END;
$$;
CREATE OR REPLACE FUNCTION public.set_updated_at_provider_settings() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
  NEW.updated_at = now();

  RETURN NEW;
END;
$$;
CREATE OR REPLACE FUNCTION public.set_updated_at_settings_definitions() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
  NEW.updated_at = now();

  RETURN NEW;
END;
$$;
CREATE OR REPLACE FUNCTION public.touch_runtime_model_from_instance() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE
  m TEXT;

  ts TIMESTAMPTZ;
BEGIN
  m := NEW.worker_model_id;
  IF m IS NULL OR btrim(m) = '' THEN
    RETURN NEW;
  END IF;
  ts := COALESCE(NEW.worker_last_heartbeat, NOW());
  INSERT INTO runtime_models(model_id, first_seen_at, last_seen_at)
  VALUES (m, ts, ts)
  ON CONFLICT (model_id) DO UPDATE
    SET last_seen_at = ts;
  RETURN NEW;
END;
$$;
CREATE OR REPLACE FUNCTION public.validate_global_settings() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
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
$$;
CREATE OR REPLACE FUNCTION public.validate_provider_settings() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
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
$$;
CREATE TABLE public.gpu_samples (
    "time" timestamp with time zone DEFAULT now() NOT NULL,
    instance_id uuid NOT NULL,
    gpu_index integer NOT NULL,
    gpu_utilization double precision,
    vram_used_mb double precision,
    vram_total_mb double precision,
    temp_c double precision,
    power_w double precision,
    power_limit_w double precision
);

CREATE TABLE public.system_samples (
    "time" timestamp with time zone DEFAULT now() NOT NULL,
    instance_id uuid NOT NULL,
    cpu_usage_pct double precision,
    load1 double precision,
    mem_used_bytes bigint,
    mem_total_bytes bigint,
    disk_used_bytes bigint,
    disk_total_bytes bigint,
    net_rx_bps double precision,
    net_tx_bps double precision
);

CREATE TABLE finops.api_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    customer_id uuid NOT NULL,
    key_prefix text NOT NULL,
    key_hash text NOT NULL,
    label text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    revoked_at timestamp with time zone
);

CREATE TABLE finops.cost_actual_cumulative_minute (
    bucket_minute timestamp with time zone NOT NULL,
    provider_id uuid,
    instance_id uuid,
    cumulative_amount_eur numeric(18,6) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    provider_id_key uuid GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL,
    instance_id_key uuid GENERATED ALWAYS AS (COALESCE(instance_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL
);

CREATE TABLE finops.cost_actual_minute (
    bucket_minute timestamp with time zone NOT NULL,
    provider_id uuid,
    instance_id uuid,
    amount_eur numeric(14,6) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    provider_id_key uuid GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL,
    instance_id_key uuid GENERATED ALWAYS AS (COALESCE(instance_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL
);

CREATE TABLE finops.cost_forecast_minute (
    bucket_minute timestamp with time zone NOT NULL,
    provider_id uuid,
    burn_rate_eur_per_hour numeric(14,6) NOT NULL,
    forecast_eur_per_minute numeric(14,6) NOT NULL,
    forecast_eur_per_day numeric(14,6) NOT NULL,
    forecast_eur_per_month_30d numeric(14,6) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    provider_id_key uuid GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL,
    forecast_eur_per_hour numeric(14,6) DEFAULT 0 NOT NULL,
    forecast_eur_per_year_365d numeric(14,6) DEFAULT 0 NOT NULL
);

CREATE TABLE finops.customers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    external_ref text,
    name text,
    email text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE finops.events (
    event_id uuid NOT NULL,
    occurred_at timestamp with time zone NOT NULL,
    event_type text NOT NULL,
    source text DEFAULT 'unknown'::text NOT NULL,
    payload jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE finops.inference_usage (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    occurred_at timestamp with time zone NOT NULL,
    customer_id uuid,
    api_key_id uuid,
    model_id uuid,
    instance_id uuid,
    input_tokens integer,
    output_tokens integer,
    total_tokens integer,
    unit_price_eur_per_1k_tokens numeric(14,6),
    charged_amount_eur numeric(14,6),
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    provider_organization_id uuid,
    consumer_organization_id uuid,
    organization_model_id uuid
);

CREATE TABLE finops.provider_costs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    occurred_at timestamp with time zone NOT NULL,
    provider_id uuid,
    region_id uuid,
    zone_id uuid,
    instance_id uuid,
    resource_type text NOT NULL,
    resource_id text,
    amount_eur numeric(14,6) NOT NULL,
    currency text DEFAULT 'EUR'::text NOT NULL,
    external_id text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE finops.subscription_charges (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    customer_id uuid NOT NULL,
    charged_at timestamp with time zone NOT NULL,
    period_start timestamp with time zone,
    period_end timestamp with time zone,
    plan_code text,
    amount_eur numeric(14,6) NOT NULL,
    external_id text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE finops.token_purchases (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    customer_id uuid NOT NULL,
    api_key_id uuid,
    purchased_at timestamp with time zone NOT NULL,
    tokens bigint NOT NULL,
    amount_eur numeric(14,6) NOT NULL,
    external_id text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE IF NOT EXISTS public._sqlx_migrations (
  version bigint NOT NULL,
  description text NOT NULL,
  installed_on timestamp with time zone DEFAULT now() NOT NULL,
  success boolean NOT NULL,
  checksum bytea NOT NULL,
  execution_time bigint NOT NULL
);

CREATE TABLE public.action_logs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    action_type character varying(50) NOT NULL,
    component character varying(20) NOT NULL,
    status character varying(20) NOT NULL,
    error_code character varying(50),
    error_message text,
    instance_id uuid,
    user_id character varying(100),
    request_payload jsonb,
    response_payload jsonb,
    duration_ms integer,
    source_ip character varying(45),
    created_at timestamp with time zone DEFAULT now(),
    completed_at timestamp with time zone,
    parent_log_id uuid,
    metadata jsonb,
    instance_status_before character varying(50),
    instance_status_after character varying(50),
    CONSTRAINT action_logs_component_check CHECK (((component)::text = ANY (ARRAY[('api'::character varying)::text, ('backend'::character varying)::text, ('orchestrator'::character varying)::text]))),
    CONSTRAINT action_logs_status_check CHECK (((status)::text = ANY (ARRAY[('success'::character varying)::text, ('failed'::character varying)::text, ('in_progress'::character varying)::text])))
);

COMMENT ON TABLE public.action_logs IS 'Audit log tracking all backend and orchestrator actions with results and errors';
COMMENT ON COLUMN public.action_logs.action_type IS 'Type of action: CREATE_INSTANCE, TERMINATE_INSTANCE, HEALTH_CHECK, etc.';
COMMENT ON COLUMN public.action_logs.component IS 'Which component performed the action: backend or orchestrator';
COMMENT ON COLUMN public.action_logs.duration_ms IS 'How long the action took to complete in milliseconds';
CREATE TABLE public.action_types (
    code character varying(80) NOT NULL,
    label character varying(120) NOT NULL,
    icon character varying(60) NOT NULL,
    color_class character varying(160) NOT NULL,
    category character varying(50),
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);

CREATE TABLE public.api_keys (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    name text NOT NULL,
    key_hash text NOT NULL,
    key_prefix text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone,
    revoked_at timestamp with time zone,
    metadata jsonb,
    organization_id uuid
);

CREATE TABLE public.global_settings (
    key text NOT NULL,
    value_int bigint,
    value_bool boolean,
    value_text text,
    value_json jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.instance_state_history (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid NOT NULL,
    from_status character varying(50),
    to_status character varying(50) NOT NULL,
    reason text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now()
);

CREATE TABLE public.instance_type_zones (
    instance_type_id uuid NOT NULL,
    zone_id uuid NOT NULL,
    is_available boolean DEFAULT true,
    created_at timestamp without time zone DEFAULT now()
);

CREATE TABLE public.instance_types (
    id uuid NOT NULL,
    code character varying(50) NOT NULL,
    name character varying(50) NOT NULL,
    provider_id uuid NOT NULL,
    gpu_count integer NOT NULL,
    vram_per_gpu_gb integer NOT NULL,
    cost_per_hour numeric(10,4) DEFAULT 0.0,
    cpu_count integer DEFAULT 0,
    ram_gb integer DEFAULT 0,
    bandwidth_bps bigint DEFAULT 0,
    is_active boolean DEFAULT true NOT NULL,
    allocation_params jsonb DEFAULT '{}'::jsonb NOT NULL
);

CREATE TABLE public.instance_volumes (
    id uuid NOT NULL,
    instance_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    zone_code text NOT NULL,
    provider_volume_id text NOT NULL,
    volume_type text NOT NULL,
    size_bytes bigint NOT NULL,
    perf_iops integer,
    delete_on_terminate boolean DEFAULT true NOT NULL,
    status text DEFAULT 'attached'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    attached_at timestamp with time zone,
    deleted_at timestamp with time zone,
    error_message text,
    provider_volume_name text,
    is_boot boolean DEFAULT false NOT NULL
);

CREATE TABLE public.instances (
    id uuid NOT NULL,
    provider_id uuid NOT NULL,
    zone_id uuid,
    instance_type_id uuid,
    model_id uuid,
    provider_instance_id character varying(255),
    ip_address inet,
    api_key character varying(255),
    status public.instance_status DEFAULT 'provisioning'::public.instance_status NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    terminated_at timestamp with time zone,
    gpu_profile jsonb NOT NULL,
    is_archived boolean DEFAULT false NOT NULL,
    ready_at timestamp with time zone,
    last_health_check timestamp with time zone,
    health_check_failures integer DEFAULT 0,
    deletion_reason character varying(50),
    deleted_by_provider boolean DEFAULT false,
    last_reconciliation timestamp without time zone,
    error_message text,
    error_code character varying(50),
    failed_at timestamp with time zone,
    retry_count integer DEFAULT 0,
    worker_last_heartbeat timestamp with time zone,
    worker_status text,
    worker_model_id text,
    worker_health_port integer,
    worker_vllm_port integer,
    worker_queue_depth integer,
    worker_gpu_utilization double precision,
    worker_metadata jsonb,
    boot_started_at timestamp with time zone
);

CREATE TABLE public.mock_provider_instances (
    provider_instance_id text NOT NULL,
    provider_id uuid NOT NULL,
    zone_code text NOT NULL,
    instance_type_code text NOT NULL,
    status text NOT NULL,
    ip_address inet,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    started_at timestamp with time zone,
    termination_requested_at timestamp with time zone,
    delete_after timestamp with time zone,
    terminated_at timestamp with time zone,
    metadata jsonb
);

CREATE SEQUENCE public.mock_provider_ip_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

CREATE TABLE public.models (
    id uuid NOT NULL,
    name character varying(255) NOT NULL,
    model_id character varying(255) NOT NULL,
    required_vram_gb integer NOT NULL,
    context_length integer NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    is_active boolean DEFAULT true NOT NULL,
    data_volume_gb bigint,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL
);

CREATE TABLE public.organization_memberships (
    organization_id uuid NOT NULL,
    user_id uuid NOT NULL,
    role text DEFAULT 'user'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT organization_memberships_role_check CHECK ((role = ANY (ARRAY['owner'::text, 'admin'::text, 'manager'::text, 'user'::text])))
);

CREATE TABLE public.organization_model_shares (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_organization_id uuid NOT NULL,
    consumer_organization_id uuid NOT NULL,
    organization_model_id uuid NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    pricing jsonb DEFAULT '{}'::jsonb NOT NULL,
    starts_at timestamp with time zone DEFAULT now() NOT NULL,
    ends_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT organization_model_shares_distinct_orgs CHECK ((provider_organization_id <> consumer_organization_id))
);

CREATE TABLE public.organization_models (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    organization_id uuid NOT NULL,
    model_id uuid NOT NULL,
    name text NOT NULL,
    code text NOT NULL,
    description text,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.organizations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    slug text NOT NULL,
    created_by_user_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.provider_settings (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_id uuid NOT NULL,
    key text NOT NULL,
    value_int bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    value_bool boolean,
    value_text text,
    value_json jsonb
);

CREATE TABLE public.providers (
    id uuid NOT NULL,
    name character varying(50) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now(),
    is_active boolean DEFAULT true NOT NULL,
    code character varying(50) NOT NULL
);

CREATE TABLE public.regions (
    id uuid NOT NULL,
    provider_id uuid NOT NULL,
    name character varying(50) NOT NULL,
    code character varying(50) NOT NULL,
    is_active boolean DEFAULT true NOT NULL
);

CREATE TABLE public.runtime_models (
    model_id text NOT NULL,
    first_seen_at timestamp with time zone DEFAULT now() NOT NULL,
    last_seen_at timestamp with time zone DEFAULT now() NOT NULL,
    total_requests bigint DEFAULT 0 NOT NULL,
    failed_requests bigint DEFAULT 0 NOT NULL
);

ALTER TABLE ONLY public.runtime_models ADD CONSTRAINT runtime_models_model_id_key UNIQUE (model_id);

CREATE TABLE public.settings_definitions (
    key text NOT NULL,
    scope text DEFAULT 'provider'::text NOT NULL,
    value_type text DEFAULT 'int'::text NOT NULL,
    min_int bigint,
    max_int bigint,
    default_int bigint,
    description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    default_bool boolean,
    default_text text,
    default_json jsonb,
    CONSTRAINT settings_definitions_pkey PRIMARY KEY (key)
);

CREATE TABLE public.ssh_keys (
    id uuid NOT NULL,
    name character varying(50) NOT NULL,
    public_key text NOT NULL,
    provider_id uuid NOT NULL,
    provider_key_id character varying(255),
    created_at timestamp with time zone DEFAULT now()
);

CREATE TABLE public.users (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    email character varying(255) NOT NULL,
    password_hash character varying(255) NOT NULL,
    role character varying(50) DEFAULT 'admin'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    first_name text,
    last_name text,
    username text NOT NULL
);

CREATE TABLE public.user_sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL,
    current_organization_id uuid,
    organization_role text CHECK (organization_role IN ('owner', 'admin', 'manager', 'user')),
    session_token_hash text NOT NULL,
    ip_address inet,
    user_agent text,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    CONSTRAINT user_sessions_org_role_check CHECK (organization_role IN ('owner', 'admin', 'manager', 'user') OR organization_role IS NULL)
);

CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id) WHERE revoked_at IS NULL;
CREATE INDEX idx_user_sessions_token_hash ON user_sessions(session_token_hash) WHERE revoked_at IS NULL;
CREATE INDEX idx_user_sessions_expires_at ON user_sessions(expires_at) WHERE revoked_at IS NULL;
CREATE INDEX idx_user_sessions_org_id ON user_sessions(current_organization_id) WHERE revoked_at IS NULL;

CREATE TABLE public.workbench_messages (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    run_id uuid NOT NULL,
    message_index integer NOT NULL,
    role text NOT NULL,
    content text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT workbench_messages_role_check CHECK ((role = ANY (ARRAY['system'::text, 'user'::text, 'assistant'::text])))
);

CREATE TABLE public.workbench_projects (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    deleted_at timestamp with time zone,
    owner_user_id uuid,
    organization_id uuid,
    name text NOT NULL,
    shared_with_org boolean DEFAULT false NOT NULL
);

CREATE TABLE public.workbench_runs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    completed_at timestamp with time zone,
    created_by_user_id uuid,
    created_via_api_key_id uuid,
    model_id text NOT NULL,
    mode text DEFAULT 'chat'::text NOT NULL,
    status text DEFAULT 'in_progress'::text NOT NULL,
    ttft_ms integer,
    duration_ms integer,
    error_message text,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    title text,
    project_id uuid,
    organization_id uuid,
    shared_with_org boolean DEFAULT false NOT NULL,
    deleted_at timestamp with time zone,
    CONSTRAINT workbench_runs_actor_check CHECK (((created_by_user_id IS NOT NULL) OR (created_via_api_key_id IS NOT NULL))),
    CONSTRAINT workbench_runs_mode_check CHECK ((mode = ANY (ARRAY['chat'::text, 'validation'::text, 'batch'::text]))),
    CONSTRAINT workbench_runs_status_check CHECK ((status = ANY (ARRAY['in_progress'::text, 'success'::text, 'failed'::text, 'cancelled'::text])))
);

CREATE TABLE public.worker_auth_tokens (
    instance_id uuid NOT NULL,
    token_hash text NOT NULL,
    token_prefix text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_seen_at timestamp with time zone,
    rotated_at timestamp with time zone,
    revoked_at timestamp with time zone,
    worker_id uuid,
    metadata jsonb
);

CREATE TABLE public.zones (
    id uuid NOT NULL,
    region_id uuid NOT NULL,
    name character varying(50) NOT NULL,
    code character varying(50) NOT NULL,
    is_active boolean DEFAULT true NOT NULL
);

--
-- Primary Key Constraints
--

ALTER TABLE ONLY finops.api_keys
    ADD CONSTRAINT api_keys_pkey PRIMARY KEY (id);

ALTER TABLE ONLY finops.cost_actual_cumulative_minute
    ADD CONSTRAINT cost_actual_cumulative_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key, instance_id_key);

ALTER TABLE ONLY finops.cost_actual_minute
    ADD CONSTRAINT cost_actual_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key, instance_id_key);

ALTER TABLE ONLY finops.cost_forecast_minute
    ADD CONSTRAINT cost_forecast_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key);

ALTER TABLE ONLY finops.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY finops.events
    ADD CONSTRAINT events_pkey PRIMARY KEY (event_id);

ALTER TABLE ONLY finops.inference_usage
    ADD CONSTRAINT inference_usage_pkey PRIMARY KEY (id);

ALTER TABLE ONLY finops.provider_costs
    ADD CONSTRAINT provider_costs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY finops.subscription_charges
    ADD CONSTRAINT subscription_charges_pkey PRIMARY KEY (id);

ALTER TABLE ONLY finops.token_purchases
    ADD CONSTRAINT token_purchases_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.action_logs
    ADD CONSTRAINT action_logs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.action_types
    ADD CONSTRAINT action_types_pkey PRIMARY KEY (code);

ALTER TABLE ONLY public.instance_state_history
    ADD CONSTRAINT instance_state_history_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.instance_type_zones
    ADD CONSTRAINT instance_type_zones_pkey PRIMARY KEY (instance_type_id, zone_id);

ALTER TABLE ONLY public.instance_types
    ADD CONSTRAINT instance_types_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.instance_volumes
    ADD CONSTRAINT instance_volumes_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.instances
    ADD CONSTRAINT instances_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.mock_provider_instances
    ADD CONSTRAINT mock_provider_instances_pkey PRIMARY KEY (provider_instance_id);

ALTER TABLE ONLY public.models
    ADD CONSTRAINT models_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.providers
    ADD CONSTRAINT providers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.regions
    ADD CONSTRAINT regions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.ssh_keys
    ADD CONSTRAINT ssh_keys_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.worker_auth_tokens
    ADD CONSTRAINT worker_auth_tokens_pkey PRIMARY KEY (instance_id);

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_pkey PRIMARY KEY (id);

-- Add PRIMARY KEY to organizations if not already present
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'organizations_pkey'
    ) THEN
        ALTER TABLE ONLY public.organizations
            ADD CONSTRAINT organizations_pkey PRIMARY KEY (id);
    END IF;
END $$;

-- Add FOREIGN KEY constraints for user_sessions (after all PRIMARY KEYs are defined)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'user_sessions_user_id_fkey'
    ) THEN
        ALTER TABLE ONLY public.user_sessions
            ADD CONSTRAINT user_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;
    END IF;
    
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'user_sessions_org_id_fkey'
    ) THEN
        ALTER TABLE ONLY public.user_sessions
            ADD CONSTRAINT user_sessions_org_id_fkey FOREIGN KEY (current_organization_id) REFERENCES public.organizations(id) ON DELETE SET NULL;
    END IF;
END $$;

--
-- Unique Constraints
--

ALTER TABLE ONLY finops.customers
    ADD CONSTRAINT customers_email_key UNIQUE (email);

ALTER TABLE ONLY finops.customers
    ADD CONSTRAINT customers_external_ref_key UNIQUE (external_ref);

ALTER TABLE ONLY finops.provider_costs
    ADD CONSTRAINT provider_costs_provider_id_external_id_key UNIQUE (provider_id, external_id);

ALTER TABLE ONLY finops.subscription_charges
    ADD CONSTRAINT subscription_charges_external_id_key UNIQUE (external_id);

ALTER TABLE ONLY finops.token_purchases
    ADD CONSTRAINT token_purchases_external_id_key UNIQUE (external_id);

ALTER TABLE ONLY public.instance_types
    ADD CONSTRAINT instance_types_provider_code_key UNIQUE (provider_id, code);

ALTER TABLE ONLY public.instance_types
    ADD CONSTRAINT instance_types_provider_id_name_key UNIQUE (provider_id, name);

ALTER TABLE ONLY public.models
    ADD CONSTRAINT models_model_id_key UNIQUE (model_id);

ALTER TABLE ONLY public.providers
    ADD CONSTRAINT providers_code_key UNIQUE (code);

ALTER TABLE ONLY public.providers
    ADD CONSTRAINT providers_name_key UNIQUE (name);

ALTER TABLE ONLY public.regions
    ADD CONSTRAINT regions_provider_code_key UNIQUE (provider_id, code);

ALTER TABLE ONLY public.regions
    ADD CONSTRAINT regions_provider_id_name_key UNIQUE (provider_id, name);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_email_key UNIQUE (email);

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_region_code_key UNIQUE (region_id, code);

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_region_id_name_key UNIQUE (region_id, name);

CREATE INDEX idx_finops_api_keys_customer ON finops.api_keys USING btree (customer_id);

--
-- Name: idx_finops_cost_actual_cumulative_minute_bucket; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_cost_actual_cumulative_minute_bucket ON finops.cost_actual_cumulative_minute USING btree (bucket_minute);

--
-- Name: idx_finops_cost_actual_cumulative_provider_key; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_cost_actual_cumulative_provider_key ON finops.cost_actual_cumulative_minute USING btree (provider_id_key);

--
-- Name: idx_finops_cost_actual_minute_bucket; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_cost_actual_minute_bucket ON finops.cost_actual_minute USING btree (bucket_minute);

--
-- Name: idx_finops_cost_actual_minute_instance_bucket; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_cost_actual_minute_instance_bucket ON finops.cost_actual_minute USING btree (instance_id, bucket_minute) WHERE (instance_id IS NOT NULL);

--
-- Name: idx_finops_cost_actual_minute_provider_key; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_cost_actual_minute_provider_key ON finops.cost_actual_minute USING btree (provider_id_key);

--
-- Name: idx_finops_cost_forecast_minute_bucket; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_cost_forecast_minute_bucket ON finops.cost_forecast_minute USING btree (bucket_minute);

--
-- Name: idx_finops_cost_forecast_minute_provider_key; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_cost_forecast_minute_provider_key ON finops.cost_forecast_minute USING btree (provider_id_key);

--
-- Name: idx_finops_events_occurred_at; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_events_occurred_at ON finops.events USING btree (occurred_at);

--
-- Name: idx_finops_events_type_time; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_events_type_time ON finops.events USING btree (event_type, occurred_at);

--
-- Name: idx_finops_inference_usage_api_key_time; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_inference_usage_api_key_time ON finops.inference_usage USING btree (api_key_id, occurred_at) WHERE (api_key_id IS NOT NULL);

--
-- Name: idx_finops_inference_usage_consumer_time; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_inference_usage_consumer_time ON finops.inference_usage USING btree (consumer_organization_id, occurred_at) WHERE (consumer_organization_id IS NOT NULL);

--
-- Name: idx_finops_inference_usage_provider_time; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_inference_usage_provider_time ON finops.inference_usage USING btree (provider_organization_id, occurred_at) WHERE (provider_organization_id IS NOT NULL);

--
-- Name: idx_finops_inference_usage_time; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_inference_usage_time ON finops.inference_usage USING btree (occurred_at);

--
-- Name: idx_finops_provider_costs_instance_time; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_provider_costs_instance_time ON finops.provider_costs USING btree (instance_id, occurred_at) WHERE (instance_id IS NOT NULL);

--
-- Name: idx_finops_provider_costs_time; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_provider_costs_time ON finops.provider_costs USING btree (occurred_at);

--
-- Name: idx_finops_subscription_charges_customer; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_subscription_charges_customer ON finops.subscription_charges USING btree (customer_id, charged_at);

--
-- Name: idx_finops_subscription_charges_time; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_subscription_charges_time ON finops.subscription_charges USING btree (charged_at);

--
-- Name: idx_finops_token_purchases_customer; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_token_purchases_customer ON finops.token_purchases USING btree (customer_id, purchased_at);

--
-- Name: idx_finops_token_purchases_time; Type: INDEX; Schema: finops; Owner: -
--

CREATE INDEX idx_finops_token_purchases_time ON finops.token_purchases USING btree (purchased_at);

--
-- Name: gpu_samples_time_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX gpu_samples_time_idx ON public.gpu_samples USING btree ("time" DESC);

--
-- Name: idx_action_logs_action_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_action_logs_action_type ON public.action_logs USING btree (action_type);

--
-- Name: idx_action_logs_component_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_action_logs_component_status ON public.action_logs USING btree (component, status);

--
-- Name: idx_action_logs_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_action_logs_created_at ON public.action_logs USING btree (created_at DESC);

--
-- Name: idx_action_logs_instance_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_action_logs_instance_id ON public.action_logs USING btree (instance_id) WHERE (instance_id IS NOT NULL);

--
-- Name: idx_action_logs_instance_status_after; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_action_logs_instance_status_after ON public.action_logs USING btree (instance_status_after) WHERE (instance_status_after IS NOT NULL);

--
-- Name: idx_action_logs_parent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_action_logs_parent ON public.action_logs USING btree (parent_log_id) WHERE (parent_log_id IS NOT NULL);

--
-- Name: idx_action_logs_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_action_logs_status ON public.action_logs USING btree (status) WHERE ((status)::text <> 'success'::text);

--
-- Name: idx_api_keys_org_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_api_keys_org_created ON public.api_keys USING btree (organization_id, created_at DESC) WHERE (organization_id IS NOT NULL);

--
-- Name: idx_api_keys_prefix; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_api_keys_prefix ON public.api_keys USING btree (key_prefix);

--
-- Name: idx_api_keys_user_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_api_keys_user_created ON public.api_keys USING btree (user_id, created_at DESC);

--
-- Name: idx_gpu_samples_instance_gpu_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_gpu_samples_instance_gpu_time ON public.gpu_samples USING btree (instance_id, gpu_index, "time" DESC);

--
-- Name: idx_instance_type_zones_instance_type_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instance_type_zones_instance_type_id ON public.instance_type_zones USING btree (instance_type_id);

--
-- Name: idx_instance_type_zones_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instance_type_zones_type ON public.instance_type_zones USING btree (instance_type_id);

--
-- Name: idx_instance_type_zones_zone; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instance_type_zones_zone ON public.instance_type_zones USING btree (zone_id);

--
-- Name: idx_instance_type_zones_zone_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instance_type_zones_zone_id ON public.instance_type_zones USING btree (zone_id);

--
-- Name: idx_instance_volumes_instance_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instance_volumes_instance_id ON public.instance_volumes USING btree (instance_id);

--
-- Name: idx_instance_volumes_provider_volume_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instance_volumes_provider_volume_id ON public.instance_volumes USING btree (provider_volume_id);

--
-- Name: idx_instances_deleted_by_provider; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instances_deleted_by_provider ON public.instances USING btree (deleted_by_provider) WHERE (deleted_by_provider IS TRUE);

--
-- Name: idx_instances_error_code; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instances_error_code ON public.instances USING btree (error_code) WHERE (error_code IS NOT NULL);

--
-- Name: idx_instances_failed_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instances_failed_at ON public.instances USING btree (failed_at) WHERE (failed_at IS NOT NULL);

--
-- Name: idx_instances_health_check; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instances_health_check ON public.instances USING btree (last_health_check) WHERE (status = 'booting'::public.instance_status);

--
-- Name: idx_instances_last_reconciliation; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instances_last_reconciliation ON public.instances USING btree (last_reconciliation) WHERE (last_reconciliation IS NOT NULL);

--
-- Name: idx_instances_model_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instances_model_id ON public.instances USING btree (model_id);

--
-- Name: idx_instances_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instances_status ON public.instances USING btree (status) WHERE (status = ANY (ARRAY['booting'::public.instance_status, 'provisioning'::public.instance_status, 'draining'::public.instance_status]));

--
-- Name: idx_instances_unique_ip_health_port_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_instances_unique_ip_health_port_active ON public.instances USING btree (ip_address, worker_health_port) WHERE ((ip_address IS NOT NULL) AND (worker_health_port IS NOT NULL) AND (status = ANY (ARRAY['booting'::public.instance_status, 'ready'::public.instance_status, 'draining'::public.instance_status])));

--
-- Name: idx_instances_unique_ip_vllm_port_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX idx_instances_unique_ip_vllm_port_active ON public.instances USING btree (ip_address, worker_vllm_port) WHERE ((ip_address IS NOT NULL) AND (worker_vllm_port IS NOT NULL) AND (status = ANY (ARRAY['booting'::public.instance_status, 'ready'::public.instance_status, 'draining'::public.instance_status])));

--
-- Name: idx_instances_worker_last_heartbeat; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_instances_worker_last_heartbeat ON public.instances USING btree (worker_last_heartbeat) WHERE (worker_last_heartbeat IS NOT NULL);

--
-- Name: idx_mock_provider_instances_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mock_provider_instances_status ON public.mock_provider_instances USING btree (status);

--
-- Name: idx_mock_provider_instances_zone; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mock_provider_instances_zone ON public.mock_provider_instances USING btree (zone_code);

--
-- Name: idx_models_is_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_models_is_active ON public.models USING btree (is_active);

--
-- Name: idx_provider_settings_provider_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_provider_settings_provider_id ON public.provider_settings USING btree (provider_id);

--
-- Name: idx_runtime_models_last_seen; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_runtime_models_last_seen ON public.runtime_models USING btree (last_seen_at DESC);

--
-- Name: idx_state_history_instance; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_state_history_instance ON public.instance_state_history USING btree (instance_id, created_at DESC);

--
-- Name: idx_system_samples_instance_time; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_system_samples_instance_time ON public.system_samples USING btree (instance_id, "time" DESC);

--
-- Name: idx_workbench_messages_run_created; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_messages_run_created ON public.workbench_messages USING btree (run_id, created_at);

--
-- Name: idx_workbench_projects_org; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_projects_org ON public.workbench_projects USING btree (organization_id, created_at DESC) WHERE ((organization_id IS NOT NULL) AND (deleted_at IS NULL));

--
-- Name: idx_workbench_projects_owner; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_projects_owner ON public.workbench_projects USING btree (owner_user_id, created_at DESC) WHERE ((owner_user_id IS NOT NULL) AND (deleted_at IS NULL));

--
-- Name: idx_workbench_runs_api_key; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_runs_api_key ON public.workbench_runs USING btree (created_via_api_key_id, created_at DESC) WHERE (created_via_api_key_id IS NOT NULL);

--
-- Name: idx_workbench_runs_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_runs_created_at ON public.workbench_runs USING btree (created_at DESC);

--
-- Name: idx_workbench_runs_deleted; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_runs_deleted ON public.workbench_runs USING btree (deleted_at) WHERE (deleted_at IS NOT NULL);

--
-- Name: idx_workbench_runs_model; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_runs_model ON public.workbench_runs USING btree (model_id, created_at DESC);

--
-- Name: idx_workbench_runs_org_shared; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_runs_org_shared ON public.workbench_runs USING btree (organization_id, created_at DESC) WHERE ((organization_id IS NOT NULL) AND (shared_with_org = true) AND (deleted_at IS NULL));

--
-- Name: idx_workbench_runs_project; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_runs_project ON public.workbench_runs USING btree (project_id, created_at DESC) WHERE ((project_id IS NOT NULL) AND (deleted_at IS NULL));

--
-- Name: idx_workbench_runs_user; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_workbench_runs_user ON public.workbench_runs USING btree (created_by_user_id, created_at DESC) WHERE (created_by_user_id IS NOT NULL);

--
-- Name: idx_worker_auth_tokens_last_seen; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_worker_auth_tokens_last_seen ON public.worker_auth_tokens USING btree (last_seen_at DESC) WHERE (last_seen_at IS NOT NULL);

--
-- Name: organization_memberships_user_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX organization_memberships_user_idx ON public.organization_memberships USING btree (user_id, organization_id);

--
-- Name: organization_model_shares_consumer_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX organization_model_shares_consumer_idx ON public.organization_model_shares USING btree (consumer_organization_id, status, created_at DESC);

--
-- Name: organization_model_shares_provider_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX organization_model_shares_provider_idx ON public.organization_model_shares USING btree (provider_organization_id, status, created_at DESC);

--
-- Name: organization_model_shares_unique_active; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX organization_model_shares_unique_active ON public.organization_model_shares USING btree (provider_organization_id, consumer_organization_id, organization_model_id) WHERE (status = 'active'::text);

--
-- Name: organization_models_model_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX organization_models_model_idx ON public.organization_models USING btree (model_id);

--
-- Name: organization_models_org_active_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX organization_models_org_active_idx ON public.organization_models USING btree (organization_id, is_active, created_at DESC);

--
-- Name: organization_models_org_code_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX organization_models_org_code_key ON public.organization_models USING btree (organization_id, code);

--
-- Name: organizations_created_by_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX organizations_created_by_idx ON public.organizations USING btree (created_by_user_id, created_at DESC);

--
-- Name: organizations_slug_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX organizations_slug_key ON public.organizations USING btree (slug);

--
-- Name: system_samples_time_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX system_samples_time_idx ON public.system_samples USING btree ("time" DESC);

--
-- Name: users_username_key; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX users_username_key ON public.users USING btree (username);

--

CREATE TRIGGER trg_global_settings_updated_at BEFORE UPDATE ON public.global_settings FOR EACH ROW EXECUTE FUNCTION public.set_updated_at_global_settings();

--
-- Name: global_settings trg_global_settings_validate; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trg_global_settings_validate BEFORE INSERT OR UPDATE ON public.global_settings FOR EACH ROW EXECUTE FUNCTION public.validate_global_settings();

--
-- Name: instances trg_instances_touch_runtime_model; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trg_instances_touch_runtime_model AFTER INSERT OR UPDATE OF worker_model_id, worker_last_heartbeat ON public.instances FOR EACH ROW EXECUTE FUNCTION public.touch_runtime_model_from_instance();

--
-- Name: provider_settings trg_provider_settings_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trg_provider_settings_updated_at BEFORE UPDATE ON public.provider_settings FOR EACH ROW EXECUTE FUNCTION public.set_updated_at_provider_settings();

--
-- Name: provider_settings trg_provider_settings_validate; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trg_provider_settings_validate BEFORE INSERT OR UPDATE ON public.provider_settings FOR EACH ROW EXECUTE FUNCTION public.validate_provider_settings();

--
-- Name: settings_definitions trg_settings_definitions_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trg_settings_definitions_updated_at BEFORE UPDATE ON public.settings_definitions FOR EACH ROW EXECUTE FUNCTION public.set_updated_at_settings_definitions();

--

-- TimescaleDB: Convert tables to hypertables
DO $$
BEGIN
  PERFORM create_hypertable('gpu_samples', 'time', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
  -- ignore if already exists
END $$;

DO $$
BEGIN
  PERFORM create_hypertable('system_samples', 'time', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
  -- ignore if already exists
END $$;

-- TimescaleDB: Triggers are automatically created by TimescaleDB when converting to hypertables
-- No need to create them manually

-- TimescaleDB: Create continuous aggregates for GPU samples
CREATE MATERIALIZED VIEW IF NOT EXISTS gpu_samples_1m
WITH (timescaledb.continuous) AS
SELECT
    time_bucket(INTERVAL '1 minute', time) AS bucket,
    instance_id,
    gpu_index,
    AVG(gpu_utilization) AS gpu_utilization,
    AVG(vram_used_mb) AS vram_used_mb,
    MAX(vram_total_mb) AS vram_total_mb,
    AVG(temp_c) AS temp_c,
    AVG(power_w) AS power_w,
    MAX(power_limit_w) AS power_limit_w
FROM gpu_samples
GROUP BY bucket, instance_id, gpu_index
WITH NO DATA;

CREATE MATERIALIZED VIEW IF NOT EXISTS gpu_samples_1h
WITH (timescaledb.continuous) AS
SELECT
    time_bucket(INTERVAL '1 hour', time) AS bucket,
    instance_id,
    gpu_index,
    AVG(gpu_utilization) AS gpu_utilization,
    AVG(vram_used_mb) AS vram_used_mb,
    MAX(vram_total_mb) AS vram_total_mb,
    AVG(temp_c) AS temp_c,
    AVG(power_w) AS power_w,
    MAX(power_limit_w) AS power_limit_w
FROM gpu_samples
GROUP BY bucket, instance_id, gpu_index
WITH NO DATA;

CREATE MATERIALIZED VIEW IF NOT EXISTS gpu_samples_1d
WITH (timescaledb.continuous) AS
SELECT
    time_bucket(INTERVAL '1 day', time) AS bucket,
    instance_id,
    gpu_index,
    AVG(gpu_utilization) AS gpu_utilization,
    AVG(vram_used_mb) AS vram_used_mb,
    MAX(vram_total_mb) AS vram_total_mb,
    AVG(temp_c) AS temp_c,
    AVG(power_w) AS power_w,
    MAX(power_limit_w) AS power_limit_w
FROM gpu_samples
GROUP BY bucket, instance_id, gpu_index
WITH NO DATA;

-- TimescaleDB: Create continuous aggregates for system samples
CREATE MATERIALIZED VIEW IF NOT EXISTS system_samples_1m
WITH (timescaledb.continuous) AS
SELECT
    time_bucket(INTERVAL '1 minute', time) AS bucket,
    instance_id,
    AVG(cpu_usage_pct) AS cpu_usage_pct,
    AVG(load1) AS load1,
    AVG(mem_used_bytes)::bigint AS mem_used_bytes,
    MAX(mem_total_bytes)::bigint AS mem_total_bytes,
    AVG(disk_used_bytes)::bigint AS disk_used_bytes,
    MAX(disk_total_bytes)::bigint AS disk_total_bytes,
    AVG(net_rx_bps) AS net_rx_bps,
    AVG(net_tx_bps) AS net_tx_bps
FROM system_samples
GROUP BY bucket, instance_id
WITH NO DATA;

CREATE MATERIALIZED VIEW IF NOT EXISTS system_samples_1h
WITH (timescaledb.continuous) AS
SELECT
    time_bucket(INTERVAL '1 hour', time) AS bucket,
    instance_id,
    AVG(cpu_usage_pct) AS cpu_usage_pct,
    AVG(load1) AS load1,
    AVG(mem_used_bytes)::bigint AS mem_used_bytes,
    MAX(mem_total_bytes)::bigint AS mem_total_bytes,
    AVG(disk_used_bytes)::bigint AS disk_used_bytes,
    MAX(disk_total_bytes)::bigint AS disk_total_bytes,
    AVG(net_rx_bps) AS net_rx_bps,
    AVG(net_tx_bps) AS net_tx_bps
FROM system_samples
GROUP BY bucket, instance_id
WITH NO DATA;

CREATE MATERIALIZED VIEW IF NOT EXISTS system_samples_1d
WITH (timescaledb.continuous) AS
SELECT
    time_bucket(INTERVAL '1 day', time) AS bucket,
    instance_id,
    AVG(cpu_usage_pct) AS cpu_usage_pct,
    AVG(load1) AS load1,
    AVG(mem_used_bytes)::bigint AS mem_used_bytes,
    MAX(mem_total_bytes)::bigint AS mem_total_bytes,
    AVG(disk_used_bytes)::bigint AS disk_used_bytes,
    MAX(disk_total_bytes)::bigint AS disk_total_bytes,
    AVG(net_rx_bps) AS net_rx_bps,
    AVG(net_tx_bps) AS net_tx_bps
FROM system_samples
GROUP BY bucket, instance_id
WITH NO DATA;

-- TimescaleDB: Add refresh policies for continuous aggregates
DO $$
BEGIN
  PERFORM add_continuous_aggregate_policy('gpu_samples_1m',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 minute',
    schedule_interval => INTERVAL '1 minute',
    if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
  -- ignore if already exists
END $$;

DO $$
BEGIN
  PERFORM add_continuous_aggregate_policy('gpu_samples_1h',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
  -- ignore if already exists
END $$;

DO $$
BEGIN
  PERFORM add_continuous_aggregate_policy('gpu_samples_1d',
    start_offset => INTERVAL '30 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day',
    if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
  -- ignore if already exists
END $$;

DO $$
BEGIN
  PERFORM add_continuous_aggregate_policy('system_samples_1m',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 minute',
    schedule_interval => INTERVAL '1 minute',
    if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
  -- ignore if already exists
END $$;

DO $$
BEGIN
  PERFORM add_continuous_aggregate_policy('system_samples_1h',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
  -- ignore if already exists
END $$;

DO $$
BEGIN
  PERFORM add_continuous_aggregate_policy('system_samples_1d',
    start_offset => INTERVAL '30 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day',
    if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
  -- ignore if already exists
END $$;
