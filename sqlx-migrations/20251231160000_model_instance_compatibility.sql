-- Model-Instance Type Compatibility System (Simplified)
-- Single system: VRAM-based compatibility check
-- Rule: instance.vram_total >= model.required_vram_gb
-- Exception: Mock Provider only accepts mock-echo-model

-- Simple function to check compatibility
CREATE OR REPLACE FUNCTION check_model_instance_compatibility(
    p_model_id uuid,
    p_instance_type_id uuid
) RETURNS boolean AS $$
DECLARE
    v_model_vram integer;
    v_instance_vram_total integer;
    v_provider_code text;
    v_model_id text;
BEGIN
    -- Get model VRAM requirement and model_id
    SELECT required_vram_gb, model_id INTO v_model_vram, v_model_id
    FROM models
    WHERE id = p_model_id;
    
    -- Get instance type VRAM total and provider code
    SELECT (it.gpu_count * it.vram_per_gpu_gb), p.code INTO v_instance_vram_total, v_provider_code
    FROM instance_types it
    JOIN providers p ON p.id = it.provider_id
    WHERE it.id = p_instance_type_id;
    
    -- Mock Provider: only mock-echo-model is compatible
    IF v_provider_code = 'mock' THEN
        RETURN (v_model_id = 'mock-echo-model');
    END IF;
    
    -- Real providers: check VRAM capacity
    RETURN (v_instance_vram_total >= v_model_vram);
END;
$$ LANGUAGE plpgsql;
