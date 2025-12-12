-- Clean mocks
TRUNCATE providers, regions, zones, instance_types CASCADE;

-- Provider Scaleway
INSERT INTO providers (id, name, description)
VALUES ('00000000-0000-0000-0000-000000000001', 'scaleway', 'Scaleway GPU Cloud')
ON CONFLICT (name) DO NOTHING;

-- Region Paris
INSERT INTO regions (id, provider_id, name)
VALUES ('00000000-0000-0000-0000-000000000010', '00000000-0000-0000-0000-000000000001', 'fr-par')
ON CONFLICT (provider_id, name) DO NOTHING;

-- Zone Paris-2 (Généralement GPU)
INSERT INTO zones (id, region_id, name)
VALUES ('00000000-0000-0000-0000-000000000020', '00000000-0000-0000-0000-000000000010', 'fr-par-2')
ON CONFLICT (region_id, name) DO NOTHING;

-- Instance Type: RENDER-S (L4 24GB VRAM)
-- Note: Le nom commercial est "L4", le nom API peut être "RENDER-S" ou "GP1-L". A vérifier.
-- On assume RENDER-S pour le test comme demandé.
INSERT INTO instance_types (id, provider_id, name, gpu_count, vram_per_gpu_gb, cost_per_hour)
VALUES ('00000000-0000-0000-0000-000000000030', '00000000-0000-0000-0000-000000000001', 'RENDER-S', 1, 24, 1.50)
ON CONFLICT (provider_id, name) DO NOTHING;

-- Availability
INSERT INTO instance_availability (instance_type_id, zone_id)
VALUES ('00000000-0000-0000-0000-000000000030', '00000000-0000-0000-0000-000000000020')
ON CONFLICT DO NOTHING;

-- Admin User
INSERT INTO users (id, email, password_hash, role)
VALUES ('00000000-0000-0000-0000-000000000099', 'admin@inventiv.com', '$argon2id$v=19$m=4096,t=3,p=1$placeholder$placeholder', 'admin')
ON CONFLICT (email) DO NOTHING;
