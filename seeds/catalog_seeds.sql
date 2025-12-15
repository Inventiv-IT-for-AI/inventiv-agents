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
