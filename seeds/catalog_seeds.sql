-- Comprehensive Catalog Seeding

-- Mock Provider
INSERT INTO providers (id, name, code, description, is_active) VALUES 
    (gen_random_uuid(), 'Mock', 'mock', 'Mock provider (dev) - no real allocations', true)
ON CONFLICT (code) DO NOTHING;

-- Deactivate old Mock regions/zones/types (if they exist)
UPDATE regions SET is_active = false 
WHERE provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1) 
  AND code != 'local';
UPDATE zones SET is_active = false 
WHERE region_id IN (SELECT id FROM regions WHERE provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1))
  AND code != 'local';
UPDATE instance_types SET is_active = false 
WHERE provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1)
  AND code != 'mock-local-instance';

-- Mock Provider Regions
INSERT INTO regions (id, provider_id, name, code, is_active) VALUES
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='mock' LIMIT 1), 'Local', 'local', true)
ON CONFLICT (provider_id, code) DO UPDATE SET is_active = true, name = 'Local';

-- Mock Provider Zones
INSERT INTO zones (id, region_id, name, code, is_active) VALUES
    (gen_random_uuid(), (SELECT id FROM regions WHERE code='local' AND provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1) LIMIT 1), 'Local', 'local', true)
ON CONFLICT (region_id, code) DO UPDATE SET is_active = true, name = 'Local';

-- Mock Provider Instance Types
-- mock-local-instance: configured for local CPU-only testing (vLLM CPU-only mode)
-- GPU: 1 (simulated, for metrics compatibility)
-- VRAM: 2GB (simulated, sufficient for Qwen2.5-0.5B testing)
-- CPU: 4 cores (minimum recommended)
-- RAM: 8GB (sufficient for vLLM CPU-only with Qwen2.5-0.5B)
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) VALUES
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='mock' LIMIT 1), 'Local Instance', 'mock-local-instance', 1, 2, 4, 8, 1000000000, true, 0.0000)
ON CONFLICT (provider_id, code) DO UPDATE SET 
    name = 'Local Instance',
    gpu_count = 1,
    vram_per_gpu_gb = 2,
    cpu_count = 4,
    ram_gb = 8,
    bandwidth_bps = 1000000000,
    is_active = true,
    cost_per_hour = 0.0000;

-- Availability: link mock-local-instance to zone local
INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available)
    SELECT it.id, z.id, true FROM instance_types it
    JOIN zones z ON z.code = 'local'
    WHERE it.provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1)
      AND it.code = 'mock-local-instance'
ON CONFLICT (instance_type_id, zone_id) DO NOTHING;


-- Scaleway Provider
INSERT INTO providers (id, name, code, description, is_active) VALUES 
    (gen_random_uuid(), 'Scaleway', 'scaleway', 'Scaleway Cloud Provider', true)
ON CONFLICT (code) DO NOTHING;
-- Regions
INSERT INTO regions (id, provider_id, name, code, is_active) VALUES
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'Paris', 'fr-par', true),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'Amsterdam', 'nl-ams', true),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'Warsaw', 'pl-waw', false)
ON CONFLICT (provider_id, code) DO NOTHING;

-- Zones (Assuming IDs from select or hardcoding conceptually, here using subqueries is safer)
INSERT INTO zones (id, region_id, name, code, is_active) VALUES
    (gen_random_uuid(), (SELECT id FROM regions WHERE code='fr-par' LIMIT 1), 'Paris 1', 'fr-par-1', true),
    (gen_random_uuid(), (SELECT id FROM regions WHERE code='fr-par' LIMIT 1), 'Paris 2', 'fr-par-2', true),
    (gen_random_uuid(), (SELECT id FROM regions WHERE code='nl-ams' LIMIT 1), 'Amsterdam 1', 'nl-ams-1', true)
ON CONFLICT (region_id, code) DO NOTHING;

-- Instance Types
INSERT INTO instance_types (
    id, provider_id, name, code,
    gpu_count, vram_per_gpu_gb,
    cpu_count, ram_gb, bandwidth_bps,
    is_active, cost_per_hour
) VALUES
    -- name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'B300-SXM-8-288G', 'B300-SXM-8-288G', 8, 288, 224, 3840, 20000000000, true, 60.0000),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-SXM-8-80G',  'H100-SXM-8-80G',  8,  80, 128,  960, 20000000000, true, 23.0280),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-SXM-4-80G',  'H100-SXM-4-80G',  4,  80,  64,  480, 20000000000, true, 11.6100),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-SXM-2-80G',  'H100-SXM-2-80G',  2,  80,  32,  240, 20000000000, true,  6.0180),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-2-80G',      'H100-2-80G',      2,  80,  48,  480, 20000000000, true,  5.4600),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-1-80G',      'H100-1-80G',      1,  80,  24,  240, 10000000000, true,  2.7300),

    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L40S-8-48G',      'L40S-8-48G',      8,  48,  64,  768, 20000000000, true, 11.1994),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L40S-4-48G',      'L40S-4-48G',      4,  48,  32,  384, 10000000000, true,  5.5997),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L40S-2-48G',      'L40S-2-48G',      2,  48,  16,  192,  5000000000, true,  2.7998),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L40S-1-48G',      'L40S-1-48G',      1,  48,   8,   96,  2500000000, true,  1.3999),

    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L4-8-24G',        'L4-8-24G',        8,  24,  64,  384, 20000000000, true,  6.0000),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L4-4-24G',        'L4-4-24G',        4,  24,  32,  192, 10000000000, true,  3.0000),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L4-2-24G',        'L4-2-24G',        2,  24,  16,   96,  5000000000, true,  1.5000),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L4-1-24G',        'L4-1-24G',        1,  24,   8,   48,  2500000000, true,  0.7500),

    (gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'RENDER-S',        'RENDER-S',        1,  16,  10,   42,  2000000000, true,  1.2210)
ON CONFLICT (provider_id, code) DO NOTHING;

-- Availability: link ALL Scaleway types to zone fr-par-2 (Paris 2)
INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available)
    SELECT it.id, z.id, true FROM instance_types it
    JOIN zones z ON z.code = 'fr-par-2'
    WHERE it.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1)
ON CONFLICT (instance_type_id, zone_id) DO NOTHING;


-- OVH Provider
INSERT INTO providers (id, name, code, description, is_active) VALUES 
    (gen_random_uuid(), 'OVH', 'ovh', 'OVH Cloud Provider', true)
ON CONFLICT (code) DO NOTHING;
-- To be implemented

-- ------------------------------------------------------------
-- Settings definitions (provider-scoped)
-- ------------------------------------------------------------
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, description)
VALUES
  ('WORKER_INSTANCE_STARTUP_TIMEOUT_S', 'provider', 'int', 30, 86400, 3600, 'BOOTING->STARTUP_FAILED timeout (worker targets): includes image pulls + model download/load.'),
  ('INSTANCE_STARTUP_TIMEOUT_S',        'provider', 'int', 30, 86400,  300, 'BOOTING->STARTUP_FAILED timeout (non-worker targets).')
ON CONFLICT (key) DO UPDATE SET
  scope = EXCLUDED.scope,
  value_type = EXCLUDED.value_type,
  min_int = EXCLUDED.min_int,
  max_int = EXCLUDED.max_int,
  default_int = EXCLUDED.default_int,
  description = EXCLUDED.description;

-- Worker bootstrap / runtime knobs (provider-scoped)
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, default_bool, default_text, description)
VALUES
  ('WORKER_SSH_BOOTSTRAP_TIMEOUT_S', 'provider', 'int', 60, 86400, 900, NULL, NULL, 'SSH bootstrap timeout for worker auto-install.'),
  ('WORKER_HEALTH_PORT',            'provider', 'int', 1, 65535, 8080, NULL, NULL, 'Worker health server port (agent /readyz).'),
  ('WORKER_VLLM_PORT',              'provider', 'int', 1, 65535, 8000, NULL, NULL, 'vLLM OpenAI-compatible port on the worker.'),
  ('WORKER_DATA_VOLUME_GB_DEFAULT', 'provider', 'int', 50, 5000, 200, NULL, NULL, 'Fallback data volume size when model has no explicit recommendation.'),
  ('WORKER_EXPOSE_PORTS',           'provider', 'bool', NULL, NULL, NULL, true, NULL, 'Provider security group opens inbound worker ports (dev convenience).'),
  ('WORKER_VLLM_MODE',              'provider', 'text', NULL, NULL, NULL, NULL, 'mono', 'vLLM mode: mono|multi (multi = 1 vLLM per GPU behind HAProxy).'),
  ('WORKER_VLLM_IMAGE',             'provider', 'text', NULL, NULL, NULL, NULL, 'vllm/vllm-openai:latest', 'Docker image for vLLM OpenAI server.')
ON CONFLICT (key) DO UPDATE SET
  scope = EXCLUDED.scope,
  value_type = EXCLUDED.value_type,
  min_int = EXCLUDED.min_int,
  max_int = EXCLUDED.max_int,
  default_int = EXCLUDED.default_int,
  default_bool = EXCLUDED.default_bool,
  default_text = EXCLUDED.default_text,
  description = EXCLUDED.description;

-- Global knobs
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, description)
VALUES
  ('OPENAI_WORKER_STALE_SECONDS', 'global', 'int', 10, 86400, 120, 'Worker staleness window for OpenAI proxy discovery (/v1/models).')
ON CONFLICT (key) DO UPDATE SET
  scope = EXCLUDED.scope,
  value_type = EXCLUDED.value_type,
  min_int = EXCLUDED.min_int,
  max_int = EXCLUDED.max_int,
  default_int = EXCLUDED.default_int,
  description = EXCLUDED.description;

-- ------------------------------------------------------------
-- Models (LLM catalog) — curated defaults
-- Notes:
-- - `model_id` is the Hugging Face repository id (or local path) used by vLLM.
-- - Values like required_vram_gb / context_length / data_volume_gb are reasonable defaults
--   and can be adjusted from the UI at any time.
-- - Idempotent: upserts by UNIQUE(models.model_id).
-- ------------------------------------------------------------
INSERT INTO models (
  id, name, model_id, required_vram_gb, context_length,
  is_active, data_volume_gb, metadata, created_at, updated_at
) VALUES
  -- Meta Llama 3 / 3.1
  (gen_random_uuid(), 'Meta Llama 3 8B Instruct',  'meta-llama/Meta-Llama-3-8B-Instruct',   16,  8192,  true,  200, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Meta Llama 3 70B Instruct', 'meta-llama/Meta-Llama-3-70B-Instruct', 160,  8192,  true, 1000, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Llama 3.1 8B Instruct',     'meta-llama/Llama-3.1-8B-Instruct',      16, 131072, true,  200, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Llama 3.1 70B Instruct',    'meta-llama/Llama-3.1-70B-Instruct',    160, 131072, true, 1200, '{}'::jsonb, NOW(), NOW()),

  -- Mistral / Mixtral
  (gen_random_uuid(), 'Mistral 7B Instruct v0.2',  'mistralai/Mistral-7B-Instruct-v0.2',    16,  32768, true,  200, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Mixtral 8x7B Instruct',     'mistralai/Mixtral-8x7B-Instruct-v0.1', 48,  32768, true,  400, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Mixtral 8x22B Instruct',    'mistralai/Mixtral-8x22B-Instruct-v0.1',96,  65536, true,  800, '{}'::jsonb, NOW(), NOW()),

  -- Qwen 2.5
  (gen_random_uuid(), 'Qwen 2.5 0.5B Instruct',    'Qwen/Qwen2.5-0.5B-Instruct',             2,   2048, true,   50, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Qwen 2.5 7B Instruct',      'Qwen/Qwen2.5-7B-Instruct',              16,  32768, true,  200, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Qwen 2.5 14B Instruct',     'Qwen/Qwen2.5-14B-Instruct',             28,  32768, true,  300, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Qwen 2.5 32B Instruct',     'Qwen/Qwen2.5-32B-Instruct',             64,  32768, true,  500, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Qwen 2.5 72B Instruct',     'Qwen/Qwen2.5-72B-Instruct',            160,  32768, true, 1200, '{}'::jsonb, NOW(), NOW()),

  -- Gemma 2
  (gen_random_uuid(), 'Gemma 2 9B IT',             'google/gemma-2-9b-it',                  20,   8192, true,  200, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Gemma 2 27B IT',            'google/gemma-2-27b-it',                 48,   8192, true,  500, '{}'::jsonb, NOW(), NOW()),

  -- Phi-3
  (gen_random_uuid(), 'Phi-3 Mini 4K Instruct',    'microsoft/Phi-3-mini-4k-instruct',       8,   4096, true,   80, '{}'::jsonb, NOW(), NOW()),
  (gen_random_uuid(), 'Phi-3 Medium 4K Instruct',  'microsoft/Phi-3-medium-4k-instruct',    28,   4096, true,  200, '{}'::jsonb, NOW(), NOW())
ON CONFLICT (model_id) DO NOTHING;

-- ------------------------------------------------------------
-- Action Types (code -> label/icon/color) for Monitoring badges
-- Keep in sync with frontend Tailwind safelist.
-- ------------------------------------------------------------
INSERT INTO action_types (code, label, icon, color_class, category, is_active) VALUES
  -- Create workflow
  ('REQUEST_CREATE', 'Request Create', 'Zap', 'bg-blue-500 hover:bg-blue-600 text-white', 'create', TRUE),
  ('EXECUTE_CREATE', 'Execute Create', 'Server', 'bg-purple-500 hover:bg-purple-600 text-white', 'create', TRUE),
  ('PROVIDER_CREATE', 'Provider Create', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'create', TRUE),
  ('PERSIST_PROVIDER_ID', 'Persist Provider ID', 'Database', 'bg-indigo-500 hover:bg-indigo-600 text-white', 'create', TRUE),
  ('PROVIDER_START', 'Provider Start', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'create', TRUE),
  ('PROVIDER_GET_IP', 'Provider Get IP', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'create', TRUE),
  ('INSTANCE_CREATED', 'Instance Created', 'Database', 'bg-green-500 hover:bg-green-600 text-white', 'create', TRUE),
  ('HEALTH_CHECK', 'Health Check', 'Clock', 'bg-teal-600 hover:bg-teal-700 text-white', 'health', TRUE),
  ('WORKER_MODEL_READY_CHECK', 'Worker Model Ready Check', 'Activity', 'bg-sky-600 hover:bg-sky-700 text-white', 'health', TRUE),
  ('WORKER_VLLM_HTTP_OK', 'vLLM HTTP Ready', 'Activity', 'bg-sky-600 hover:bg-sky-700 text-white', 'health', TRUE),
  ('WORKER_MODEL_LOADED', 'Model Loaded', 'CheckCircle', 'bg-sky-600 hover:bg-sky-700 text-white', 'health', TRUE),
  ('WORKER_VLLM_WARMUP', 'vLLM Warmup', 'Activity', 'bg-sky-600 hover:bg-sky-700 text-white', 'health', TRUE),
  ('INSTANCE_READY', 'Instance Ready', 'CheckCircle', 'bg-green-600 hover:bg-green-700 text-white', 'health', TRUE),
  ('INSTANCE_STARTUP_FAILED', 'Instance Startup Failed', 'AlertTriangle', 'bg-gray-600 hover:bg-gray-700 text-white', 'health', TRUE),

  -- Termination workflow
  ('REQUEST_TERMINATE', 'Request Terminate', 'Zap', 'bg-blue-600 hover:bg-blue-700 text-white', 'terminate', TRUE),
  ('EXECUTE_TERMINATE', 'Execute Terminate', 'Server', 'bg-purple-600 hover:bg-purple-700 text-white', 'terminate', TRUE),
  ('PROVIDER_TERMINATE', 'Provider Terminate', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'terminate', TRUE),
  ('TERMINATION_PENDING', 'Termination Pending', 'Clock', 'bg-yellow-500 hover:bg-yellow-600 text-white', 'terminate', TRUE),
  ('TERMINATOR_RETRY', 'Terminator Retry', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'terminate', TRUE),
  ('TERMINATION_CONFIRMED', 'Termination Confirmed', 'Database', 'bg-red-500 hover:bg-red-600 text-white', 'terminate', TRUE),
  ('INSTANCE_TERMINATED', 'Instance Terminated', 'Database', 'bg-red-500 hover:bg-red-600 text-white', 'terminate', TRUE),
  -- Reinstall workflow
  ('REQUEST_REINSTALL', 'Request Reinstall', 'Wrench', 'bg-sky-600 hover:bg-sky-700 text-white', 'repair', TRUE),
  ('EXECUTE_REINSTALL', 'Execute Reinstall', 'Server', 'bg-sky-600 hover:bg-sky-700 text-white', 'repair', TRUE),

  -- Archive / reconciliation
  ('ARCHIVE_INSTANCE', 'Archive Instance', 'Archive', 'bg-gray-600 hover:bg-gray-700 text-white', 'archive', TRUE),
  ('PROVIDER_DELETED_DETECTED', 'Provider Deleted', 'AlertTriangle', 'bg-yellow-600 hover:bg-yellow-700 text-white', 'reconcile', TRUE),

  -- Legacy (keep active for display)
  ('TERMINATE_INSTANCE', 'Terminate Instance', 'Server', 'bg-purple-600 hover:bg-purple-700 text-white', 'legacy', TRUE),
  ('SCALEWAY_CREATE', 'Provider Create', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'legacy', TRUE),
  ('SCALEWAY_DELETE', 'Provider Delete', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'legacy', TRUE)
ON CONFLICT (code) DO NOTHING;
