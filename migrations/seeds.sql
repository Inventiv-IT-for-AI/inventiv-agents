-- Provider Mock
INSERT INTO providers (id, name, description)
VALUES ('00000000-0000-0000-0000-000000000000', 'mock', 'Simulated Provider')
ON CONFLICT (name) DO NOTHING;

-- Provider Scaleway
INSERT INTO providers (id, name, description)
VALUES ('00000000-0000-0000-0000-000000000001', 'scaleway', 'Scaleway GPU Instances')
ON CONFLICT (name) DO NOTHING;

-- Regions (Mock)
INSERT INTO regions (id, provider_id, name)
VALUES ('00000000-0000-0000-0000-000000000010', '00000000-0000-0000-0000-000000000000', 'local')
ON CONFLICT (provider_id, name) DO NOTHING;

-- Zones (Mock)
INSERT INTO zones (id, region_id, name)
VALUES ('00000000-0000-0000-0000-000000000020', '00000000-0000-0000-0000-000000000010', 'local-1')
ON CONFLICT (region_id, name) DO NOTHING;

-- Instance Types (Mock H100)
INSERT INTO instance_types (id, provider_id, name, gpu_count, vram_per_gpu_gb)
VALUES ('00000000-0000-0000-0000-000000000030', '00000000-0000-0000-0000-000000000000', 'mock-h100', 1, 80)
ON CONFLICT (provider_id, name) DO NOTHING;

-- Availability
INSERT INTO instance_availability (instance_type_id, zone_id)
VALUES ('00000000-0000-0000-0000-000000000030', '00000000-0000-0000-0000-000000000020')
ON CONFLICT DO NOTHING;

-- Initial Admin User (Default pwd: password)
-- Note: In real app, use ARGON2 hash. Here is a placeholder hash.
INSERT INTO users (id, email, password_hash, role)
VALUES ('00000000-0000-0000-0000-000000000099', 'admin@inventiv.com', '$argon2id$v=19$m=4096,t=3,p=1$placeholder$placeholder', 'admin')
ON CONFLICT (email) DO NOTHING;
