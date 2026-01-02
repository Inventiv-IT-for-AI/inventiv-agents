-- Migration: Add instance-level request metrics and enhance inference_usage with dimensions
-- This migration adds:
-- 1. Table for instance request counters (aggregated metrics per instance)
-- 2. Additional dimension columns to finops.inference_usage for better analytics

-- Table for instance request metrics (aggregated counters)
CREATE TABLE IF NOT EXISTS public.instance_request_metrics (
    instance_id uuid NOT NULL,
    total_requests bigint DEFAULT 0 NOT NULL,
    successful_requests bigint DEFAULT 0 NOT NULL,
    failed_requests bigint DEFAULT 0 NOT NULL,
    total_input_tokens bigint DEFAULT 0 NOT NULL,
    total_output_tokens bigint DEFAULT 0 NOT NULL,
    total_tokens bigint DEFAULT 0 NOT NULL,
    first_request_at timestamp with time zone,
    last_request_at timestamp with time zone,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    PRIMARY KEY (instance_id),
    FOREIGN KEY (instance_id) REFERENCES public.instances(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_instance_request_metrics_last_request ON public.instance_request_metrics(last_request_at DESC);
CREATE INDEX IF NOT EXISTS idx_instance_request_metrics_updated ON public.instance_request_metrics(updated_at DESC);

-- Add dimension columns to finops.inference_usage for better analytics
-- These columns will be populated from the instances table via JOIN or direct insert
ALTER TABLE finops.inference_usage 
    ADD COLUMN IF NOT EXISTS provider_id uuid,
    ADD COLUMN IF NOT EXISTS instance_type_id uuid,
    ADD COLUMN IF NOT EXISTS zone_id uuid,
    ADD COLUMN IF NOT EXISTS region_id uuid;

-- Add indexes for better query performance on these dimensions
CREATE INDEX IF NOT EXISTS idx_finops_inference_usage_provider_time ON finops.inference_usage(provider_id, occurred_at) WHERE provider_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_finops_inference_usage_instance_type_time ON finops.inference_usage(instance_type_id, occurred_at) WHERE instance_type_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_finops_inference_usage_zone_time ON finops.inference_usage(zone_id, occurred_at) WHERE zone_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_finops_inference_usage_instance_time ON finops.inference_usage(instance_id, occurred_at) WHERE instance_id IS NOT NULL;

-- Function to update instance request metrics atomically
CREATE OR REPLACE FUNCTION public.update_instance_request_metrics(
    p_instance_id uuid,
    p_success boolean,
    p_input_tokens integer DEFAULT NULL,
    p_output_tokens integer DEFAULT NULL,
    p_total_tokens integer DEFAULT NULL
) RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
    v_now timestamp with time zone := NOW();
BEGIN
    INSERT INTO public.instance_request_metrics (
        instance_id,
        total_requests,
        successful_requests,
        failed_requests,
        total_input_tokens,
        total_output_tokens,
        total_tokens,
        first_request_at,
        last_request_at,
        updated_at
    )
    VALUES (
        p_instance_id,
        1,
        CASE WHEN p_success THEN 1 ELSE 0 END,
        CASE WHEN p_success THEN 0 ELSE 1 END,
        COALESCE(p_input_tokens, 0),
        COALESCE(p_output_tokens, 0),
        COALESCE(p_total_tokens, 0),
        v_now,
        v_now,
        v_now
    )
    ON CONFLICT (instance_id) DO UPDATE
    SET
        total_requests = instance_request_metrics.total_requests + 1,
        successful_requests = instance_request_metrics.successful_requests + CASE WHEN p_success THEN 1 ELSE 0 END,
        failed_requests = instance_request_metrics.failed_requests + CASE WHEN p_success THEN 0 ELSE 1 END,
        total_input_tokens = instance_request_metrics.total_input_tokens + COALESCE(p_input_tokens, 0),
        total_output_tokens = instance_request_metrics.total_output_tokens + COALESCE(p_output_tokens, 0),
        total_tokens = instance_request_metrics.total_tokens + COALESCE(p_total_tokens, 0),
        last_request_at = v_now,
        updated_at = v_now;
END;
$$;

-- Function to enrich inference_usage with dimensions from instances table
CREATE OR REPLACE FUNCTION public.enrich_inference_usage_dimensions()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    -- If dimensions are not already set, fetch them from instances table
    IF NEW.provider_id IS NULL OR NEW.instance_type_id IS NULL OR NEW.zone_id IS NULL THEN
        SELECT 
            i.provider_id,
            i.instance_type_id,
            i.zone_id,
            z.region_id
        INTO
            NEW.provider_id,
            NEW.instance_type_id,
            NEW.zone_id,
            NEW.region_id
        FROM public.instances i
        LEFT JOIN public.zones z ON z.id = i.zone_id
        WHERE i.id = NEW.instance_id;
    END IF;
    
    RETURN NEW;
END;
$$;

-- Trigger to automatically enrich inference_usage with dimensions
DROP TRIGGER IF EXISTS trigger_enrich_inference_usage_dimensions ON finops.inference_usage;
CREATE TRIGGER trigger_enrich_inference_usage_dimensions
    BEFORE INSERT ON finops.inference_usage
    FOR EACH ROW
    EXECUTE FUNCTION public.enrich_inference_usage_dimensions();

