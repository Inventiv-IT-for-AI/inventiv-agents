-- Enforce uniqueness of (ip_address, worker_vllm_port) and (ip_address, worker_health_port)
-- for active instances. This prevents ambiguous routing when multiple instances share the same IP.

-- VLLM port uniqueness (active)
CREATE UNIQUE INDEX IF NOT EXISTS idx_instances_unique_ip_vllm_port_active
ON instances (ip_address, worker_vllm_port)
WHERE ip_address IS NOT NULL
  AND worker_vllm_port IS NOT NULL
  AND status IN ('booting'::instance_status,'ready'::instance_status,'draining'::instance_status);

-- Health port uniqueness (active)
CREATE UNIQUE INDEX IF NOT EXISTS idx_instances_unique_ip_health_port_active
ON instances (ip_address, worker_health_port)
WHERE ip_address IS NOT NULL
  AND worker_health_port IS NOT NULL
  AND status IN ('booting'::instance_status,'ready'::instance_status,'draining'::instance_status);


