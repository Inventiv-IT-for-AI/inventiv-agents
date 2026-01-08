--
-- PostgreSQL database dump
--

-- Dumped from database version 14.17
-- Dumped by pg_dump version 14.17

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
-- IMPORTANT: keep search_path including public so sqlx can access its _sqlx_migrations table.
SELECT pg_catalog.set_config('search_path', 'public', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: timescaledb; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS timescaledb WITH SCHEMA public;


--
-- Name: EXTENSION timescaledb; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION timescaledb IS 'Enables scalable inserts and complex queries for time-series data (Community Edition)';


--
-- Name: finops; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA finops;


--
-- Name: pgcrypto; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;


--
-- Name: EXTENSION pgcrypto; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';


--
-- Name: instance_status; Type: TYPE; Schema: public; Owner: -
--

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


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: api_keys; Type: TABLE; Schema: finops; Owner: -
--

CREATE TABLE finops.api_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    customer_id uuid NOT NULL,
    key_prefix text NOT NULL,
    key_hash text NOT NULL,
    label text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    revoked_at timestamp with time zone
);


--
-- Name: cost_actual_cumulative_minute; Type: TABLE; Schema: finops; Owner: -
--

CREATE TABLE finops.cost_actual_cumulative_minute (
    bucket_minute timestamp with time zone NOT NULL,
    provider_id uuid,
    instance_id uuid,
    cumulative_amount_usd numeric(18,6) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    provider_id_key uuid GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL,
    instance_id_key uuid GENERATED ALWAYS AS (COALESCE(instance_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL
);


--
-- Name: cost_actual_minute; Type: TABLE; Schema: finops; Owner: -
--

CREATE TABLE finops.cost_actual_minute (
    bucket_minute timestamp with time zone NOT NULL,
    provider_id uuid,
    instance_id uuid,
    amount_usd numeric(14,6) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    provider_id_key uuid GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL,
    instance_id_key uuid GENERATED ALWAYS AS (COALESCE(instance_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL
);


--
-- Name: cost_forecast_minute; Type: TABLE; Schema: finops; Owner: -
--

CREATE TABLE finops.cost_forecast_minute (
    bucket_minute timestamp with time zone NOT NULL,
    provider_id uuid,
    burn_rate_usd_per_hour numeric(14,6) NOT NULL,
    forecast_usd_per_minute numeric(14,6) NOT NULL,
    forecast_usd_per_day numeric(14,6) NOT NULL,
    forecast_usd_per_month_30d numeric(14,6) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    provider_id_key uuid GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED NOT NULL
);


--
-- Name: customers; Type: TABLE; Schema: finops; Owner: -
--

CREATE TABLE finops.customers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    external_ref text,
    name text,
    email text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: events; Type: TABLE; Schema: finops; Owner: -
--

CREATE TABLE finops.events (
    event_id uuid NOT NULL,
    occurred_at timestamp with time zone NOT NULL,
    event_type text NOT NULL,
    source text DEFAULT 'unknown'::text NOT NULL,
    payload jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: inference_usage; Type: TABLE; Schema: finops; Owner: -
--

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
    unit_price_usd_per_1k_tokens numeric(14,6),
    charged_amount_usd numeric(14,6),
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: provider_costs; Type: TABLE; Schema: finops; Owner: -
--

CREATE TABLE finops.provider_costs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    occurred_at timestamp with time zone NOT NULL,
    provider_id uuid,
    region_id uuid,
    zone_id uuid,
    instance_id uuid,
    resource_type text NOT NULL,
    resource_id text,
    amount_usd numeric(14,6) NOT NULL,
    currency text DEFAULT 'USD'::text NOT NULL,
    external_id text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: subscription_charges; Type: TABLE; Schema: finops; Owner: -
--

CREATE TABLE finops.subscription_charges (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    customer_id uuid NOT NULL,
    charged_at timestamp with time zone NOT NULL,
    period_start timestamp with time zone,
    period_end timestamp with time zone,
    plan_code text,
    amount_usd numeric(14,6) NOT NULL,
    external_id text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: token_purchases; Type: TABLE; Schema: finops; Owner: -
--

CREATE TABLE finops.token_purchases (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    customer_id uuid NOT NULL,
    api_key_id uuid,
    purchased_at timestamp with time zone NOT NULL,
    tokens bigint NOT NULL,
    amount_usd numeric(14,6) NOT NULL,
    external_id text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: action_logs; Type: TABLE; Schema: public; Owner: -
--

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
    CONSTRAINT action_logs_component_check CHECK (((component)::text = ANY ((ARRAY['api'::character varying, 'backend'::character varying, 'orchestrator'::character varying])::text[]))),
    CONSTRAINT action_logs_status_check CHECK (((status)::text = ANY ((ARRAY['success'::character varying, 'failed'::character varying, 'in_progress'::character varying])::text[])))
);


--
-- Name: TABLE action_logs; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.action_logs IS 'Audit log tracking all backend and orchestrator actions with results and errors';


--
-- Name: COLUMN action_logs.action_type; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.action_logs.action_type IS 'Type of action: CREATE_INSTANCE, TERMINATE_INSTANCE, HEALTH_CHECK, etc.';


--
-- Name: COLUMN action_logs.component; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.action_logs.component IS 'Which component performed the action: backend or orchestrator';


--
-- Name: COLUMN action_logs.duration_ms; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.action_logs.duration_ms IS 'How long the action took to complete in milliseconds';


--
-- Name: action_types; Type: TABLE; Schema: public; Owner: -
--

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


--
-- Name: instance_state_history; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.instance_state_history (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    instance_id uuid NOT NULL,
    from_status character varying(50),
    to_status character varying(50) NOT NULL,
    reason text,
    metadata jsonb,
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: instance_type_zones; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.instance_type_zones (
    instance_type_id uuid NOT NULL,
    zone_id uuid NOT NULL,
    is_available boolean DEFAULT true,
    created_at timestamp without time zone DEFAULT now()
);


--
-- Name: instance_types; Type: TABLE; Schema: public; Owner: -
--

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


--
-- Name: instance_volumes; Type: TABLE; Schema: public; Owner: -
--

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
    error_message text
);


--
-- Name: instances; Type: TABLE; Schema: public; Owner: -
--

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
    -- Worker (data plane) state (Phase 0.2.1 "Worker ready")
    worker_last_heartbeat timestamp with time zone,
    worker_status text,
    worker_model_id text,
    worker_health_port integer,
    worker_vllm_port integer,
    worker_queue_depth integer,
    worker_gpu_utilization double precision,
    worker_metadata jsonb
);


--
-- Name: mock_provider_instances; Type: TABLE; Schema: public; Owner: -
--

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


--
-- Name: mock_provider_ip_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.mock_provider_ip_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: models; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.models (
    id uuid NOT NULL,
    name character varying(255) NOT NULL,
    model_id character varying(255) NOT NULL,
    required_vram_gb integer NOT NULL,
    context_length integer NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: providers; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.providers (
    id uuid NOT NULL,
    name character varying(50) NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now(),
    is_active boolean DEFAULT true NOT NULL,
    code character varying(50) NOT NULL
);


--
-- Name: regions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.regions (
    id uuid NOT NULL,
    provider_id uuid NOT NULL,
    name character varying(50) NOT NULL,
    code character varying(50) NOT NULL,
    is_active boolean DEFAULT true NOT NULL
);


--
-- Name: ssh_keys; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.ssh_keys (
    id uuid NOT NULL,
    name character varying(50) NOT NULL,
    public_key text NOT NULL,
    provider_id uuid NOT NULL,
    provider_key_id character varying(255),
    created_at timestamp with time zone DEFAULT now()
);


--
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.users (
    id uuid NOT NULL,
    email character varying(255) NOT NULL,
    password_hash character varying(255) NOT NULL,
    role character varying(50) DEFAULT 'admin'::character varying NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


--
-- Name: zones; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.zones (
    id uuid NOT NULL,
    region_id uuid NOT NULL,
    name character varying(50) NOT NULL,
    code character varying(50) NOT NULL,
    is_active boolean DEFAULT true NOT NULL
);


--
-- Name: api_keys api_keys_key_hash_key; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.api_keys
    ADD CONSTRAINT api_keys_key_hash_key UNIQUE (key_hash);


--
-- Name: api_keys api_keys_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.api_keys
    ADD CONSTRAINT api_keys_pkey PRIMARY KEY (id);


--
-- Name: cost_actual_cumulative_minute cost_actual_cumulative_minute_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.cost_actual_cumulative_minute
    ADD CONSTRAINT cost_actual_cumulative_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key, instance_id_key);


--
-- Name: cost_actual_minute cost_actual_minute_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.cost_actual_minute
    ADD CONSTRAINT cost_actual_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key, instance_id_key);


--
-- Name: cost_forecast_minute cost_forecast_minute_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.cost_forecast_minute
    ADD CONSTRAINT cost_forecast_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key);


--
-- Name: customers customers_email_key; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.customers
    ADD CONSTRAINT customers_email_key UNIQUE (email);


--
-- Name: customers customers_external_ref_key; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.customers
    ADD CONSTRAINT customers_external_ref_key UNIQUE (external_ref);


--
-- Name: customers customers_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.customers
    ADD CONSTRAINT customers_pkey PRIMARY KEY (id);


--
-- Name: events events_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.events
    ADD CONSTRAINT events_pkey PRIMARY KEY (event_id);


--
-- Name: inference_usage inference_usage_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.inference_usage
    ADD CONSTRAINT inference_usage_pkey PRIMARY KEY (id);


--
-- Name: provider_costs provider_costs_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.provider_costs
    ADD CONSTRAINT provider_costs_pkey PRIMARY KEY (id);


--
-- Name: provider_costs provider_costs_provider_id_external_id_key; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.provider_costs
    ADD CONSTRAINT provider_costs_provider_id_external_id_key UNIQUE (provider_id, external_id);


--
-- Name: subscription_charges subscription_charges_external_id_key; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.subscription_charges
    ADD CONSTRAINT subscription_charges_external_id_key UNIQUE (external_id);


--
-- Name: subscription_charges subscription_charges_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.subscription_charges
    ADD CONSTRAINT subscription_charges_pkey PRIMARY KEY (id);


--
-- Name: token_purchases token_purchases_external_id_key; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.token_purchases
    ADD CONSTRAINT token_purchases_external_id_key UNIQUE (external_id);


--
-- Name: token_purchases token_purchases_pkey; Type: CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.token_purchases
    ADD CONSTRAINT token_purchases_pkey PRIMARY KEY (id);


--
-- Name: action_logs action_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.action_logs
    ADD CONSTRAINT action_logs_pkey PRIMARY KEY (id);


--
-- Name: action_types action_types_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.action_types
    ADD CONSTRAINT action_types_pkey PRIMARY KEY (code);


--
-- Name: instance_state_history instance_state_history_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_state_history
    ADD CONSTRAINT instance_state_history_pkey PRIMARY KEY (id);


--
-- Name: instance_type_zones instance_type_zones_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_type_zones
    ADD CONSTRAINT instance_type_zones_pkey PRIMARY KEY (instance_type_id, zone_id);


--
-- Name: instance_types instance_types_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_types
    ADD CONSTRAINT instance_types_pkey PRIMARY KEY (id);


--
-- Name: instance_types instance_types_provider_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_types
    ADD CONSTRAINT instance_types_provider_code_key UNIQUE (provider_id, code);


--
-- Name: instance_types instance_types_provider_id_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_types
    ADD CONSTRAINT instance_types_provider_id_name_key UNIQUE (provider_id, name);


--
-- Name: instance_volumes instance_volumes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_volumes
    ADD CONSTRAINT instance_volumes_pkey PRIMARY KEY (id);


--
-- Name: instances instances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instances
    ADD CONSTRAINT instances_pkey PRIMARY KEY (id);


--
-- Name: mock_provider_instances mock_provider_instances_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.mock_provider_instances
    ADD CONSTRAINT mock_provider_instances_pkey PRIMARY KEY (provider_instance_id);


--
-- Name: models models_model_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.models
    ADD CONSTRAINT models_model_id_key UNIQUE (model_id);


--
-- Name: models models_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.models
    ADD CONSTRAINT models_pkey PRIMARY KEY (id);


--
-- Name: providers providers_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.providers
    ADD CONSTRAINT providers_code_key UNIQUE (code);


--
-- Name: providers providers_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.providers
    ADD CONSTRAINT providers_name_key UNIQUE (name);


--
-- Name: providers providers_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.providers
    ADD CONSTRAINT providers_pkey PRIMARY KEY (id);


--
-- Name: regions regions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.regions
    ADD CONSTRAINT regions_pkey PRIMARY KEY (id);


--
-- Name: regions regions_provider_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.regions
    ADD CONSTRAINT regions_provider_code_key UNIQUE (provider_id, code);


--
-- Name: regions regions_provider_id_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.regions
    ADD CONSTRAINT regions_provider_id_name_key UNIQUE (provider_id, name);


--
-- Name: ssh_keys ssh_keys_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ssh_keys
    ADD CONSTRAINT ssh_keys_pkey PRIMARY KEY (id);


--
-- Name: users users_email_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_email_key UNIQUE (email);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: zones zones_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_pkey PRIMARY KEY (id);


--
-- Name: zones zones_region_code_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_region_code_key UNIQUE (region_id, code);


--
-- Name: zones zones_region_id_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_region_id_name_key UNIQUE (region_id, name);


--
-- Name: idx_finops_api_keys_customer; Type: INDEX; Schema: finops; Owner: -
--

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
-- Name: idx_mock_provider_instances_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mock_provider_instances_status ON public.mock_provider_instances USING btree (status);


--
-- Name: idx_mock_provider_instances_zone; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_mock_provider_instances_zone ON public.mock_provider_instances USING btree (zone_code);


--
-- Name: idx_state_history_instance; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_state_history_instance ON public.instance_state_history USING btree (instance_id, created_at DESC);


--
-- Name: api_keys api_keys_customer_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.api_keys
    ADD CONSTRAINT api_keys_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES finops.customers(id) ON DELETE CASCADE;


--
-- Name: cost_actual_cumulative_minute cost_actual_cumulative_minute_instance_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.cost_actual_cumulative_minute
    ADD CONSTRAINT cost_actual_cumulative_minute_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES public.instances(id);


--
-- Name: cost_actual_cumulative_minute cost_actual_cumulative_minute_provider_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.cost_actual_cumulative_minute
    ADD CONSTRAINT cost_actual_cumulative_minute_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id);


--
-- Name: cost_actual_minute cost_actual_minute_instance_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.cost_actual_minute
    ADD CONSTRAINT cost_actual_minute_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES public.instances(id);


--
-- Name: cost_actual_minute cost_actual_minute_provider_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.cost_actual_minute
    ADD CONSTRAINT cost_actual_minute_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id);


--
-- Name: cost_forecast_minute cost_forecast_minute_provider_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.cost_forecast_minute
    ADD CONSTRAINT cost_forecast_minute_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id);


--
-- Name: inference_usage inference_usage_api_key_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.inference_usage
    ADD CONSTRAINT inference_usage_api_key_id_fkey FOREIGN KEY (api_key_id) REFERENCES finops.api_keys(id) ON DELETE SET NULL;


--
-- Name: inference_usage inference_usage_customer_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.inference_usage
    ADD CONSTRAINT inference_usage_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES finops.customers(id) ON DELETE SET NULL;


--
-- Name: inference_usage inference_usage_instance_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.inference_usage
    ADD CONSTRAINT inference_usage_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES public.instances(id);


--
-- Name: inference_usage inference_usage_model_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.inference_usage
    ADD CONSTRAINT inference_usage_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.models(id);


--
-- Name: provider_costs provider_costs_instance_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.provider_costs
    ADD CONSTRAINT provider_costs_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES public.instances(id);


--
-- Name: provider_costs provider_costs_provider_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.provider_costs
    ADD CONSTRAINT provider_costs_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id);


--
-- Name: provider_costs provider_costs_region_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.provider_costs
    ADD CONSTRAINT provider_costs_region_id_fkey FOREIGN KEY (region_id) REFERENCES public.regions(id);


--
-- Name: provider_costs provider_costs_zone_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.provider_costs
    ADD CONSTRAINT provider_costs_zone_id_fkey FOREIGN KEY (zone_id) REFERENCES public.zones(id);


--
-- Name: subscription_charges subscription_charges_customer_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.subscription_charges
    ADD CONSTRAINT subscription_charges_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES finops.customers(id) ON DELETE CASCADE;


--
-- Name: token_purchases token_purchases_api_key_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.token_purchases
    ADD CONSTRAINT token_purchases_api_key_id_fkey FOREIGN KEY (api_key_id) REFERENCES finops.api_keys(id) ON DELETE SET NULL;


--
-- Name: token_purchases token_purchases_customer_id_fkey; Type: FK CONSTRAINT; Schema: finops; Owner: -
--

ALTER TABLE ONLY finops.token_purchases
    ADD CONSTRAINT token_purchases_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES finops.customers(id) ON DELETE CASCADE;


--
-- Name: action_logs action_logs_instance_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.action_logs
    ADD CONSTRAINT action_logs_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES public.instances(id) ON DELETE SET NULL;


--
-- Name: action_logs action_logs_parent_log_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.action_logs
    ADD CONSTRAINT action_logs_parent_log_id_fkey FOREIGN KEY (parent_log_id) REFERENCES public.action_logs(id) ON DELETE SET NULL;


--
-- Name: instance_state_history instance_state_history_instance_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_state_history
    ADD CONSTRAINT instance_state_history_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES public.instances(id) ON DELETE CASCADE;


--
-- Name: instance_type_zones instance_type_zones_instance_type_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_type_zones
    ADD CONSTRAINT instance_type_zones_instance_type_id_fkey FOREIGN KEY (instance_type_id) REFERENCES public.instance_types(id) ON DELETE CASCADE;


--
-- Name: instance_type_zones instance_type_zones_zone_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_type_zones
    ADD CONSTRAINT instance_type_zones_zone_id_fkey FOREIGN KEY (zone_id) REFERENCES public.zones(id) ON DELETE CASCADE;


--
-- Name: instance_volumes instance_volumes_instance_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_volumes
    ADD CONSTRAINT instance_volumes_instance_id_fkey FOREIGN KEY (instance_id) REFERENCES public.instances(id) ON DELETE CASCADE;


--
-- Name: instance_volumes instance_volumes_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_volumes
    ADD CONSTRAINT instance_volumes_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id);


--
-- Name: mock_provider_instances mock_provider_instances_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.mock_provider_instances
    ADD CONSTRAINT mock_provider_instances_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id);


--
-- Name: ssh_keys ssh_keys_provider_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.ssh_keys
    ADD CONSTRAINT ssh_keys_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.providers(id);


--
-- PostgreSQL database dump complete
--

