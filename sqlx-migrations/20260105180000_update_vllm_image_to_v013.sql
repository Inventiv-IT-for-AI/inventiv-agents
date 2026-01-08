-- Migration: Update vLLM image versions to v0.13.0 (available on Docker Hub)
-- Previous migration used v0.6.2.post1 which doesn't exist on Docker Hub

-- Update L4 instances to use v0.13.0
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.13.0"'::jsonb
)
WHERE code LIKE 'L4-%'
  AND (allocation_params->>'vllm_image' IS NULL 
       OR allocation_params->>'vllm_image' = ''
       OR allocation_params->>'vllm_image' = '"vllm/vllm-openai:v0.6.2.post1"');

-- Update L40S instances to use v0.13.0
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.13.0"'::jsonb
)
WHERE code LIKE 'L40S-%'
  AND (allocation_params->>'vllm_image' IS NULL 
       OR allocation_params->>'vllm_image' = ''
       OR allocation_params->>'vllm_image' = '"vllm/vllm-openai:v0.6.2.post1"');

-- Update default provider setting to use v0.13.0
UPDATE provider_settings
SET value_text = 'vllm/vllm-openai:v0.13.0'
WHERE key = 'WORKER_VLLM_IMAGE'
  AND (value_text IS NULL 
       OR value_text = '' 
       OR value_text LIKE '%latest%'
       OR value_text = 'vllm/vllm-openai:v0.6.2.post1');

