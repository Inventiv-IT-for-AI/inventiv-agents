-- Drop legacy duplicate column (use gpu_count only)
ALTER TABLE instance_types
  DROP COLUMN IF EXISTS n_gpu;

