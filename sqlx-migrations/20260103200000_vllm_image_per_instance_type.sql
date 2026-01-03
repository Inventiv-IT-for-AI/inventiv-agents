-- Migration: Add vLLM image configuration per instance type
-- This allows different vLLM Docker images for different instance types/GPUs
-- Example: RENDER-S (P100) needs vLLM compiled with sm_60 support, while L4/L40S can use newer versions

-- Update RENDER-S instances to use a vLLM image compatible with P100 (compute capability 6.0)
-- IMPORTANT: v0.6.2.post1 standard image does NOT support P100 (sm_60)
-- For P100, we need a custom-built image or an older version that includes sm_60 support
-- TODO: Replace with actual P100-compatible image once identified/tested
-- For now, using v0.6.2.post1 as placeholder - this will fail on P100 and needs to be updated
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.6.2.post1"'::jsonb
)
WHERE code = 'RENDER-S'
  AND (allocation_params->>'vllm_image' IS NULL OR allocation_params->>'vllm_image' = '');

-- Update L4 instances to use a stable vLLM version (compatible with compute capability 8.9)
-- L4 GPUs (sm_89) are fully supported by v0.6.2.post1 and newer versions
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.6.2.post1"'::jsonb
)
WHERE code LIKE 'L4-%'
  AND (allocation_params->>'vllm_image' IS NULL OR allocation_params->>'vllm_image' = '');

-- Update L40S instances to use a stable vLLM version (compatible with compute capability 8.9)
-- L40S GPUs (sm_89) are fully supported by v0.6.2.post1 and newer versions
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.6.2.post1"'::jsonb
)
WHERE code LIKE 'L40S-%'
  AND (allocation_params->>'vllm_image' IS NULL OR allocation_params->>'vllm_image' = '');

-- Update default provider setting to use stable version instead of "latest"
UPDATE provider_settings
SET value_text = 'vllm/vllm-openai:v0.6.2.post1'
WHERE key = 'WORKER_VLLM_IMAGE'
  AND (value_text IS NULL OR value_text = '' OR value_text LIKE '%latest%');

