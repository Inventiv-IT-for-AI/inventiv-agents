-- Comprehensive Catalog Seeding

-- Mock Provider
INSERT INTO providers (id, name, code, description, is_active) 
SELECT gen_random_uuid(), 'Mock', 'mock', 'Mock provider (dev) - no real allocations', true
WHERE NOT EXISTS (SELECT 1 FROM providers WHERE code = 'mock');

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
INSERT INTO regions (id, provider_id, name, code, is_active) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='mock' LIMIT 1), 'Local', 'local', true
WHERE NOT EXISTS (SELECT 1 FROM regions WHERE provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1) AND code = 'local');
UPDATE regions SET is_active = true, name = 'Local' WHERE provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1) AND code = 'local';

-- Mock Provider Zones
INSERT INTO zones (id, region_id, provider_id, name, code, is_active) 
SELECT gen_random_uuid(), (SELECT id FROM regions WHERE code='local' AND provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1) LIMIT 1), (SELECT id FROM providers WHERE code='mock' LIMIT 1), 'Local', 'local', true
WHERE NOT EXISTS (SELECT 1 FROM zones WHERE region_id = (SELECT id FROM regions WHERE code='local' AND provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1) LIMIT 1) AND code = 'local');
UPDATE zones SET is_active = true, name = 'Local' WHERE region_id = (SELECT id FROM regions WHERE code='local' AND provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1) LIMIT 1) AND code = 'local';

-- Mock Provider Instance Types
-- mock-local-instance: configured for local CPU-only testing (vLLM CPU-only mode)
-- GPU: 1 (simulated, for metrics compatibility)
-- VRAM: 2GB (simulated, sufficient for Qwen2.5-0.5B testing)
-- CPU: 4 cores (minimum recommended)
-- RAM: 8GB (sufficient for vLLM CPU-only with Qwen2.5-0.5B)
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='mock' LIMIT 1), 'Local Instance', 'mock-local-instance', 1, 2, 4, 8, 1000000000, true, 0.0000
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1) AND code = 'mock-local-instance');
UPDATE instance_types SET 
    name = 'Local Instance',
    gpu_count = 1,
    vram_per_gpu_gb = 2,
    cpu_count = 4,
    ram_gb = 8,
    bandwidth_bps = 1000000000,
    is_active = true,
    cost_per_hour = 0.0000
WHERE provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1) AND code = 'mock-local-instance';

-- Availability: link mock-local-instance to zone local
INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available)
    SELECT it.id, z.id, true FROM instance_types it
    JOIN zones z ON z.code = 'local'
    WHERE it.provider_id = (SELECT id FROM providers WHERE code='mock' LIMIT 1)
      AND it.code = 'mock-local-instance'
      AND NOT EXISTS (SELECT 1 FROM instance_type_zones WHERE instance_type_id = it.id AND zone_id = z.id);


-- Scaleway Provider
INSERT INTO providers (id, name, code, description, is_active) 
SELECT gen_random_uuid(), 'Scaleway', 'scaleway', 'Scaleway Cloud Provider', true
WHERE NOT EXISTS (SELECT 1 FROM providers WHERE code = 'scaleway');

-- Important: deactivate legacy/unknown Scaleway regions/zones (idempotent).
-- We only keep the "official" region codes used by this project.
UPDATE regions
SET is_active = false
WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1)
  AND code NOT IN ('fr-par', 'nl-ams', 'pl-waw');
UPDATE zones
SET is_active = false
WHERE region_id IN (
    SELECT id FROM regions
    WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1)
      AND code NOT IN ('fr-par', 'nl-ams', 'pl-waw')
);
-- Regions
INSERT INTO regions (id, provider_id, name, code, is_active) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'Paris', 'fr-par', true
WHERE NOT EXISTS (SELECT 1 FROM regions WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'fr-par');
UPDATE regions SET is_active = true, name = 'Paris'
WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'fr-par';
INSERT INTO regions (id, provider_id, name, code, is_active) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'Amsterdam', 'nl-ams', true
WHERE NOT EXISTS (SELECT 1 FROM regions WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'nl-ams');
UPDATE regions SET is_active = true, name = 'Amsterdam'
WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'nl-ams';
INSERT INTO regions (id, provider_id, name, code, is_active) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'Warsaw', 'pl-waw', false
WHERE NOT EXISTS (SELECT 1 FROM regions WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'pl-waw');
UPDATE regions SET is_active = false, name = 'Warsaw'
WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'pl-waw';

-- Zones (Assuming IDs from select or hardcoding conceptually, here using subqueries is safer)
INSERT INTO zones (id, region_id, provider_id, name, code, is_active) 
SELECT gen_random_uuid(),
       (SELECT r.id FROM regions r WHERE r.code='fr-par' AND r.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) LIMIT 1),
       (SELECT id FROM providers WHERE code='scaleway' LIMIT 1),
       'Paris 1', 'fr-par-1', true
WHERE NOT EXISTS (
  SELECT 1 FROM zones z
  WHERE z.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1)
    AND z.code = 'fr-par-1'
);
UPDATE zones z
SET is_active = true, name = 'Paris 1'
WHERE z.code = 'fr-par-1'
  AND z.region_id = (SELECT r.id FROM regions r WHERE r.code='fr-par' AND r.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) LIMIT 1);
INSERT INTO zones (id, region_id, provider_id, name, code, is_active) 
SELECT gen_random_uuid(),
       (SELECT r.id FROM regions r WHERE r.code='fr-par' AND r.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) LIMIT 1),
       (SELECT id FROM providers WHERE code='scaleway' LIMIT 1),
       'Paris 2', 'fr-par-2', true
WHERE NOT EXISTS (
  SELECT 1 FROM zones z
  WHERE z.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1)
    AND z.code = 'fr-par-2'
);
UPDATE zones z
SET is_active = true, name = 'Paris 2'
WHERE z.code = 'fr-par-2'
  AND z.region_id = (SELECT r.id FROM regions r WHERE r.code='fr-par' AND r.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) LIMIT 1);
INSERT INTO zones (id, region_id, provider_id, name, code, is_active) 
SELECT gen_random_uuid(),
       (SELECT r.id FROM regions r WHERE r.code='nl-ams' AND r.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) LIMIT 1),
       (SELECT id FROM providers WHERE code='scaleway' LIMIT 1),
       'Amsterdam 1', 'nl-ams-1', true
WHERE NOT EXISTS (
  SELECT 1 FROM zones z
  WHERE z.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1)
    AND z.code = 'nl-ams-1'
);
UPDATE zones z
SET is_active = true, name = 'Amsterdam 1'
WHERE z.code = 'nl-ams-1'
  AND z.region_id = (SELECT r.id FROM regions r WHERE r.code='nl-ams' AND r.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) LIMIT 1);

-- Instance Types (Scaleway)
-- Note: Using individual INSERT statements with WHERE NOT EXISTS for idempotency
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'B300-SXM-8-288G', 'B300-SXM-8-288G', 8, 288, 224, 3840, 20000000000, true, 60.0000
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'B300-SXM-8-288G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-SXM-8-80G', 'H100-SXM-8-80G', 8, 80, 128, 960, 20000000000, true, 23.0280
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'H100-SXM-8-80G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-SXM-4-80G', 'H100-SXM-4-80G', 4, 80, 64, 480, 20000000000, true, 11.6100
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'H100-SXM-4-80G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-SXM-2-80G', 'H100-SXM-2-80G', 2, 80, 32, 240, 20000000000, true, 6.0180
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'H100-SXM-2-80G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-2-80G', 'H100-2-80G', 2, 80, 48, 480, 20000000000, true, 5.4600
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'H100-2-80G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'H100-1-80G', 'H100-1-80G', 1, 80, 24, 240, 10000000000, true, 2.7300
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'H100-1-80G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L40S-8-48G', 'L40S-8-48G', 8, 48, 64, 768, 20000000000, true, 11.1994
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'L40S-8-48G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L40S-4-48G', 'L40S-4-48G', 4, 48, 32, 384, 10000000000, true, 5.5997
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'L40S-4-48G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L40S-2-48G', 'L40S-2-48G', 2, 48, 16, 192, 5000000000, true, 2.7998
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'L40S-2-48G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L40S-1-48G', 'L40S-1-48G', 1, 48, 8, 96, 2500000000, true, 1.3999
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'L40S-1-48G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L4-8-24G', 'L4-8-24G', 8, 24, 64, 384, 20000000000, true, 6.0000
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'L4-8-24G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L4-4-24G', 'L4-4-24G', 4, 24, 32, 192, 10000000000, true, 3.0000
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'L4-4-24G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L4-2-24G', 'L4-2-24G', 2, 24, 16, 96, 5000000000, true, 1.5000
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'L4-2-24G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'L4-1-24G', 'L4-1-24G', 1, 24, 8, 48, 2500000000, true, 0.7500
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'L4-1-24G');
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour) 
SELECT gen_random_uuid(), (SELECT id FROM providers WHERE code='scaleway' LIMIT 1), 'RENDER-S', 'RENDER-S', 1, 16, 10, 42, 2000000000, true, 1.2210
WHERE NOT EXISTS (SELECT 1 FROM instance_types WHERE provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1) AND code = 'RENDER-S');

-- Availability: link ALL Scaleway types to zone fr-par-2 (Paris 2)
INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available)
    SELECT it.id, z.id, true FROM instance_types it
    JOIN zones z ON z.code = 'fr-par-2'
    JOIN regions r ON r.id = z.region_id AND r.code = 'fr-par'
    JOIN providers p ON p.id = r.provider_id AND p.code = 'scaleway'
    WHERE it.provider_id = (SELECT id FROM providers WHERE code='scaleway' LIMIT 1)
      AND NOT EXISTS (SELECT 1 FROM instance_type_zones WHERE instance_type_id = it.id AND zone_id = z.id);


-- OVH Provider
INSERT INTO providers (id, name, code, description, is_active) 
SELECT gen_random_uuid(), 'OVH', 'ovh', 'OVH Cloud Provider', true
WHERE NOT EXISTS (SELECT 1 FROM providers WHERE code = 'ovh');
-- To be implemented

-- ------------------------------------------------------------
-- Settings definitions (provider-scoped)
-- ------------------------------------------------------------
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, description)
SELECT 'WORKER_INSTANCE_STARTUP_TIMEOUT_S', 'provider', 'int', 30, 86400, 3600, 'BOOTING->STARTUP_FAILED timeout (worker targets): includes image pulls + model download/load.'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'WORKER_INSTANCE_STARTUP_TIMEOUT_S');
UPDATE settings_definitions SET scope = 'provider', value_type = 'int', min_int = 30, max_int = 86400, default_int = 3600, description = 'BOOTING->STARTUP_FAILED timeout (worker targets): includes image pulls + model download/load.' WHERE key = 'WORKER_INSTANCE_STARTUP_TIMEOUT_S';
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, description)
SELECT 'INSTANCE_STARTUP_TIMEOUT_S', 'provider', 'int', 30, 86400, 300, 'BOOTING->STARTUP_FAILED timeout (non-worker targets).'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'INSTANCE_STARTUP_TIMEOUT_S');
UPDATE settings_definitions SET scope = 'provider', value_type = 'int', min_int = 30, max_int = 86400, default_int = 300, description = 'BOOTING->STARTUP_FAILED timeout (non-worker targets).' WHERE key = 'INSTANCE_STARTUP_TIMEOUT_S';

-- Worker bootstrap / runtime knobs (provider-scoped)
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, default_bool, default_text, description)
SELECT 'WORKER_SSH_BOOTSTRAP_TIMEOUT_S', 'provider', 'int', 60, 86400, 900, NULL, NULL, 'SSH bootstrap timeout for worker auto-install.'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'WORKER_SSH_BOOTSTRAP_TIMEOUT_S');
UPDATE settings_definitions SET scope = 'provider', value_type = 'int', min_int = 60, max_int = 86400, default_int = 900, default_bool = NULL, default_text = NULL, description = 'SSH bootstrap timeout for worker auto-install.' WHERE key = 'WORKER_SSH_BOOTSTRAP_TIMEOUT_S';
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, default_bool, default_text, description)
SELECT 'WORKER_HEALTH_PORT', 'provider', 'int', 1, 65535, 8080, NULL, NULL, 'Worker health server port (agent /readyz).'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'WORKER_HEALTH_PORT');
UPDATE settings_definitions SET scope = 'provider', value_type = 'int', min_int = 1, max_int = 65535, default_int = 8080, default_bool = NULL, default_text = NULL, description = 'Worker health server port (agent /readyz).' WHERE key = 'WORKER_HEALTH_PORT';
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, default_bool, default_text, description)
SELECT 'WORKER_VLLM_PORT', 'provider', 'int', 1, 65535, 8000, NULL, NULL, 'vLLM OpenAI-compatible port on the worker.'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'WORKER_VLLM_PORT');
UPDATE settings_definitions SET scope = 'provider', value_type = 'int', min_int = 1, max_int = 65535, default_int = 8000, default_bool = NULL, default_text = NULL, description = 'vLLM OpenAI-compatible port on the worker.' WHERE key = 'WORKER_VLLM_PORT';
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, default_bool, default_text, description)
SELECT 'WORKER_DATA_VOLUME_GB_DEFAULT', 'provider', 'int', 50, 5000, 200, NULL, NULL, 'Fallback data volume size when model has no explicit recommendation.'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'WORKER_DATA_VOLUME_GB_DEFAULT');
UPDATE settings_definitions SET scope = 'provider', value_type = 'int', min_int = 50, max_int = 5000, default_int = 200, default_bool = NULL, default_text = NULL, description = 'Fallback data volume size when model has no explicit recommendation.' WHERE key = 'WORKER_DATA_VOLUME_GB_DEFAULT';
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, default_bool, default_text, description)
SELECT 'WORKER_EXPOSE_PORTS', 'provider', 'bool', NULL, NULL, NULL, true, NULL, 'Provider security group opens inbound worker ports (dev convenience).'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'WORKER_EXPOSE_PORTS');
UPDATE settings_definitions SET scope = 'provider', value_type = 'bool', min_int = NULL, max_int = NULL, default_int = NULL, default_bool = true, default_text = NULL, description = 'Provider security group opens inbound worker ports (dev convenience).' WHERE key = 'WORKER_EXPOSE_PORTS';
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, default_bool, default_text, description)
SELECT 'WORKER_VLLM_MODE', 'provider', 'text', NULL, NULL, NULL, NULL, 'mono', 'vLLM mode: mono|multi (multi = 1 vLLM per GPU behind HAProxy).'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'WORKER_VLLM_MODE');
UPDATE settings_definitions SET scope = 'provider', value_type = 'text', min_int = NULL, max_int = NULL, default_int = NULL, default_bool = NULL, default_text = 'mono', description = 'vLLM mode: mono|multi (multi = 1 vLLM per GPU behind HAProxy).' WHERE key = 'WORKER_VLLM_MODE';
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, default_bool, default_text, description)
SELECT 'WORKER_VLLM_IMAGE', 'provider', 'text', NULL, NULL, NULL, NULL, 'vllm/vllm-openai:latest', 'Docker image for vLLM OpenAI server.'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'WORKER_VLLM_IMAGE');
UPDATE settings_definitions SET scope = 'provider', value_type = 'text', min_int = NULL, max_int = NULL, default_int = NULL, default_bool = NULL, default_text = 'vllm/vllm-openai:latest', description = 'Docker image for vLLM OpenAI server.' WHERE key = 'WORKER_VLLM_IMAGE';

-- Global knobs
INSERT INTO settings_definitions (key, scope, value_type, min_int, max_int, default_int, description)
SELECT 'OPENAI_WORKER_STALE_SECONDS', 'global', 'int', 10, 86400, 120, 'Worker staleness window for OpenAI proxy discovery (/v1/models).'
WHERE NOT EXISTS (SELECT 1 FROM settings_definitions WHERE key = 'OPENAI_WORKER_STALE_SECONDS');
UPDATE settings_definitions SET scope = 'global', value_type = 'int', min_int = 10, max_int = 86400, default_int = 120, description = 'Worker staleness window for OpenAI proxy discovery (/v1/models).' WHERE key = 'OPENAI_WORKER_STALE_SECONDS';

-- ------------------------------------------------------------
-- Models (LLM catalog) — curated defaults
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Meta Llama 3 8B Instruct', 'meta-llama/Meta-Llama-3-8B-Instruct', 16, 8192, true, 200, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'meta-llama/Meta-Llama-3-8B-Instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Meta Llama 3 70B Instruct', 'meta-llama/Meta-Llama-3-70B-Instruct', 160, 8192, true, 1000, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'meta-llama/Meta-Llama-3-70B-Instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Llama 3.1 8B Instruct', 'meta-llama/Llama-3.1-8B-Instruct', 16, 131072, true, 200, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'meta-llama/Llama-3.1-8B-Instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Llama 3.1 70B Instruct', 'meta-llama/Llama-3.1-70B-Instruct', 160, 131072, true, 1200, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'meta-llama/Llama-3.1-70B-Instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Mistral 7B Instruct v0.2', 'mistralai/Mistral-7B-Instruct-v0.2', 16, 32768, true, 200, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'mistralai/Mistral-7B-Instruct-v0.2');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Mixtral 8x7B Instruct', 'mistralai/Mixtral-8x7B-Instruct-v0.1', 48, 32768, true, 400, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'mistralai/Mixtral-8x7B-Instruct-v0.1');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Mixtral 8x22B Instruct', 'mistralai/Mixtral-8x22B-Instruct-v0.1', 96, 65536, true, 800, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'mistralai/Mixtral-8x22B-Instruct-v0.1');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Mock Echo Model', 'mock-echo-model', 0, 2048, true, 0, '{"provider_restriction": "mock", "description": "Synthetic echo model for Mock Provider local testing"}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'mock-echo-model');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Qwen 2.5 0.5B Instruct', 'Qwen/Qwen2.5-0.5B-Instruct', 2, 2048, true, 50, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'Qwen/Qwen2.5-0.5B-Instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Qwen 2.5 7B Instruct', 'Qwen/Qwen2.5-7B-Instruct', 16, 32768, true, 200, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'Qwen/Qwen2.5-7B-Instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Qwen 2.5 14B Instruct', 'Qwen/Qwen2.5-14B-Instruct', 28, 32768, true, 300, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'Qwen/Qwen2.5-14B-Instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Qwen 2.5 32B Instruct', 'Qwen/Qwen2.5-32B-Instruct', 64, 32768, true, 500, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'Qwen/Qwen2.5-32B-Instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Qwen 2.5 72B Instruct', 'Qwen/Qwen2.5-72B-Instruct', 160, 32768, true, 1200, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'Qwen/Qwen2.5-72B-Instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Gemma 2 9B IT', 'google/gemma-2-9b-it', 20, 8192, true, 200, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'google/gemma-2-9b-it');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Gemma 2 27B IT', 'google/gemma-2-27b-it', 48, 8192, true, 500, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'google/gemma-2-27b-it');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Phi-3 Mini 4K Instruct', 'microsoft/Phi-3-mini-4k-instruct', 8, 4096, true, 80, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'microsoft/Phi-3-mini-4k-instruct');
INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
SELECT gen_random_uuid(), 'Phi-3 Medium 4K Instruct', 'microsoft/Phi-3-medium-4k-instruct', 28, 4096, true, 200, '{}'::jsonb, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM models WHERE model_id = 'microsoft/Phi-3-medium-4k-instruct');


-- ------------------------------------------------------------
-- Action Types (code -> label/icon/color) for Monitoring badges
-- Keep in sync with frontend Tailwind safelist.
-- Note: Using DELETE then INSERT for idempotency (no UNIQUE constraint on code)
DELETE FROM action_types WHERE code IN (
  'REQUEST_CREATE', 'EXECUTE_CREATE', 'PROVIDER_CREATE', 'PERSIST_PROVIDER_ID', 'PROVIDER_START', 'PROVIDER_GET_IP', 'INSTANCE_CREATED',
  'HEALTH_CHECK', 'WORKER_MODEL_READY_CHECK', 'WORKER_VLLM_HTTP_OK', 'WORKER_MODEL_LOADED', 'WORKER_VLLM_WARMUP', 'INSTANCE_READY', 'INSTANCE_STARTUP_FAILED',
  'REQUEST_TERMINATE', 'EXECUTE_TERMINATE', 'PROVIDER_TERMINATE', 'TERMINATION_PENDING', 'TERMINATOR_RETRY', 'TERMINATION_CONFIRMED', 'INSTANCE_TERMINATED',
  'REQUEST_REINSTALL', 'EXECUTE_REINSTALL', 'ARCHIVE_INSTANCE', 'PROVIDER_DELETED_DETECTED', 'TERMINATE_INSTANCE', 'SCALEWAY_CREATE', 'SCALEWAY_DELETE'
);
INSERT INTO action_types (code, label, icon, color_class, category, is_active) VALUES
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
  ('REQUEST_TERMINATE', 'Request Terminate', 'Zap', 'bg-blue-600 hover:bg-blue-700 text-white', 'terminate', TRUE),
  ('EXECUTE_TERMINATE', 'Execute Terminate', 'Server', 'bg-purple-600 hover:bg-purple-700 text-white', 'terminate', TRUE),
  ('PROVIDER_TERMINATE', 'Provider Terminate', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'terminate', TRUE),
  ('TERMINATION_PENDING', 'Termination Pending', 'Clock', 'bg-yellow-500 hover:bg-yellow-600 text-white', 'terminate', TRUE),
  ('TERMINATOR_RETRY', 'Terminator Retry', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'terminate', TRUE),
  ('TERMINATION_CONFIRMED', 'Termination Confirmed', 'Database', 'bg-red-500 hover:bg-red-600 text-white', 'terminate', TRUE),
  ('INSTANCE_TERMINATED', 'Instance Terminated', 'Database', 'bg-red-500 hover:bg-red-600 text-white', 'terminate', TRUE),
  ('REQUEST_REINSTALL', 'Request Reinstall', 'Wrench', 'bg-sky-600 hover:bg-sky-700 text-white', 'repair', TRUE),
  ('EXECUTE_REINSTALL', 'Execute Reinstall', 'Server', 'bg-sky-600 hover:bg-sky-700 text-white', 'repair', TRUE),
  ('ARCHIVE_INSTANCE', 'Archive Instance', 'Archive', 'bg-gray-600 hover:bg-gray-700 text-white', 'archive', TRUE),
  ('PROVIDER_DELETED_DETECTED', 'Provider Deleted', 'AlertTriangle', 'bg-yellow-600 hover:bg-yellow-700 text-white', 'reconcile', TRUE),
  ('TERMINATE_INSTANCE', 'Terminate Instance', 'Server', 'bg-purple-600 hover:bg-purple-700 text-white', 'legacy', TRUE),
  ('SCALEWAY_CREATE', 'Provider Create', 'Cloud', 'bg-orange-500 hover:bg-orange-600 text-white', 'legacy', TRUE),
  ('SCALEWAY_DELETE', 'Provider Delete', 'Cloud', 'bg-orange-600 hover:bg-orange-700 text-white', 'legacy', TRUE);

