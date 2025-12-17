-- Comprehensive Catalog Seeding

-- Mock Provider
INSERT INTO providers (id, name, code, description, is_active) VALUES 
    (gen_random_uuid(), 'Mock', 'mock', 'Mock provider (dev) - no real allocations', true)
ON CONFLICT (code) DO NOTHING;
-- Mock Provider Regions
INSERT INTO regions (id, provider_id, name, code, is_active) VALUES
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='mock' LIMIT 1), 'Mock EU', 'mock-eu', true)
ON CONFLICT (provider_id, code) DO NOTHING;

-- Mock Provider Zones
INSERT INTO zones (id, region_id, name, code, is_active) VALUES
    (gen_random_uuid(), (SELECT id FROM regions WHERE code='mock-eu' LIMIT 1), 'Mock EU 1', 'mock-eu-1', true)
ON CONFLICT (region_id, code) DO NOTHING;

-- Mock Provider Instance Types
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) VALUES
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='mock' LIMIT 1), 'MOCK-GPU-S', 'MOCK-GPU-S', 1, 24, 8, 32, 1000000000, true, 0.2500),
    (gen_random_uuid(), (SELECT id FROM providers WHERE code='mock' LIMIT 1), 'MOCK-4GPU-M', 'MOCK-4GPU-M', 4, 48, 16, 64, 2000000000, true, 10.0000)
ON CONFLICT (provider_id, code) DO NOTHING;

-- Availability: link ALL Mock types to zone mock-eu-1
INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available)
    SELECT it.id, z.id, true FROM instance_types it
    JOIN zones z ON z.code = 'mock-eu-1'
    WHERE it.provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1)
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

  -- Archive / reconciliation
  ('ARCHIVE_INSTANCE', 'Archive Instance', 'Archive', 'bg-gray-600 hover:bg-gray-700 text-white', 'archive', TRUE),
  ('PROVIDER_DELETED_DETECTED', 'Provider Deleted', 'AlertTriangle', 'bg-yellow-600 hover:bg-yellow-700 text-white', 'reconcile', TRUE),

  -- Legacy (keep active for display)
  ('TERMINATE_INSTANCE', 'Terminate Instance', 'Server', 'bg-purple-600 hover:bg-purple-700 text-white', 'legacy', TRUE),
  ('SCALEWAY_CREATE', 'Provider Create', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'legacy', TRUE),
  ('SCALEWAY_DELETE', 'Provider Delete', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'legacy', TRUE)
ON CONFLICT (code) DO NOTHING;
