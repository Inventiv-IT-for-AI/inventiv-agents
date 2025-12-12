-- Comprehensive Catalog Seeding
-- Provider
INSERT INTO providers (id, name, description) VALUES (gen_random_uuid(), 'Scaleway', 'Scaleway Cloud Provider') ON CONFLICT DO NOTHING; -- Assuming name is not unique constraint? Actually providers usually just ID. Let's check constraints or just select.
-- Actually unique constraint on name is likely. Let's try inserting.
INSERT INTO providers (id, name, description) SELECT gen_random_uuid(), 'Scaleway', 'Cloud Provider' WHERE NOT EXISTS (SELECT 1 FROM providers WHERE name = 'Scaleway');

-- Regions
INSERT INTO regions (id, provider_id, name, code, is_active) VALUES
    (gen_random_uuid(), (SELECT id FROM providers WHERE name='scaleway' LIMIT 1), 'Paris', 'fr-par', true),
    (gen_random_uuid(), (SELECT id FROM providers WHERE name='scaleway' LIMIT 1), 'Amsterdam', 'nl-ams', true),
    (gen_random_uuid(), (SELECT id FROM providers WHERE name='scaleway' LIMIT 1), 'Warsaw', 'pl-waw', false)
ON CONFLICT (provider_id, code) DO NOTHING;

-- Zones (Assuming IDs from select or hardcoding conceptually, here using subqueries is safer)
INSERT INTO zones (id, region_id, name, code, is_active) VALUES
    (gen_random_uuid(), (SELECT id FROM regions WHERE code='fr-par' AND provider_id=(SELECT id FROM providers WHERE name='scaleway' LIMIT 1)), 'Paris 1', 'fr-par-1', true),
    (gen_random_uuid(), (SELECT id FROM regions WHERE code='fr-par' AND provider_id=(SELECT id FROM providers WHERE name='scaleway' LIMIT 1)), 'Paris 2', 'fr-par-2', true),
    (gen_random_uuid(), (SELECT id FROM regions WHERE code='nl-ams' AND provider_id=(SELECT id FROM providers WHERE name='scaleway' LIMIT 1)), 'Amsterdam 1', 'nl-ams-1', true)
ON CONFLICT (region_id, code) DO NOTHING;

-- Instance Types
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, is_active, cost_per_hour) VALUES
    (gen_random_uuid(), (SELECT id FROM providers WHERE name='scaleway' LIMIT 1), 'H100 PCIe', 'H100-PCIe', 1, 80, true, 4.50),
    (gen_random_uuid(), (SELECT id FROM providers WHERE name='scaleway' LIMIT 1), 'H100 SXM5', 'H100-SXM5', 1, 80, true, 5.20),
    (gen_random_uuid(), (SELECT id FROM providers WHERE name='scaleway' LIMIT 1), 'L40S', 'L40S', 1, 48, true, 2.10),
    (gen_random_uuid(), (SELECT id FROM providers WHERE name='scaleway' LIMIT 1), 'A100 80G', 'A100-80G', 1, 80, true, 3.80)
ON CONFLICT (provider_id, code) DO UPDATE SET cost_per_hour = EXCLUDED.cost_per_hour;
