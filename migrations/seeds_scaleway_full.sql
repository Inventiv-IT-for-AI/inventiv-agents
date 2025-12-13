-- Comprehensive Catalog Seeding for Scaleway GPU Instances
-- Based on user provided screenshot

WITH provider AS (
    SELECT id FROM providers WHERE name = 'Scaleway' LIMIT 1
)
INSERT INTO instance_types (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cost_per_hour, is_active)
SELECT 
    gen_random_uuid(), 
    provider.id, 
    val.name, 
    val.code, 
    val.gpu_count, 
    val.vram_per_gpu_gb, 
    val.cost, 
    true
FROM provider, (VALUES
    ('B300-SXM-8-288G', 'B300-SXM-8-288G', 8, 288, 60.00),
    ('H100-SXM-8-80G', 'H100-SXM-8-80G', 8, 80, 23.028),
    ('H100-SXM-4-80G', 'H100-SXM-4-80G', 4, 80, 11.61),
    ('H100-SXM-2-80G', 'H100-SXM-2-80G', 2, 80, 6.018),
    ('H100-2-80G', 'H100-2-80G', 2, 80, 5.46),
    ('H100-1-80G', 'H100-1-80G', 1, 80, 2.73),
    ('L40S-8-48G', 'L40S-8-48G', 8, 48, 11.1994),
    ('L40S-4-48G', 'L40S-4-48G', 4, 48, 5.5997),
    ('L40S-2-48G', 'L40S-2-48G', 2, 48, 2.7998),
    ('L40S-1-48G', 'L40S-1-48G', 1, 48, 1.3999),
    ('L4-8-24G', 'L4-8-24G', 8, 24, 6.00),
    ('L4-4-24G', 'L4-4-24G', 4, 24, 3.00),
    ('L4-2-24G', 'L4-2-24G', 2, 24, 1.50),
    ('L4-1-24G', 'L4-1-24G', 1, 24, 0.75),
    ('RENDER-S', 'RENDER-S', 1, 16, 1.221)
) as val(name, code, gpu_count, vram_per_gpu_gb, cost)
ON CONFLICT (provider_id, code) 
DO UPDATE SET 
    cost_per_hour = EXCLUDED.cost_per_hour,
    gpu_count = EXCLUDED.gpu_count,
    vram_per_gpu_gb = EXCLUDED.vram_per_gpu_gb,
    is_active = true;
