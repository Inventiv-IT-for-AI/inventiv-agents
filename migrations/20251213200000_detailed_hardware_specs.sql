-- Add detailed hardware specs to instance_types
ALTER TABLE instance_types 
ADD COLUMN IF NOT EXISTS cpu_count INTEGER DEFAULT 0,
ADD COLUMN IF NOT EXISTS ram_gb INTEGER DEFAULT 0,
ADD COLUMN IF NOT EXISTS n_gpu INTEGER DEFAULT 0, -- Renaming gpu_count to n_gpu for consistency or keep gpu_count? Let's check existing schema.
ADD COLUMN IF NOT EXISTS bandwidth_bps BIGINT DEFAULT 0; -- Bandwidth in bits per second

-- Let's check if gpu_count exists.
DO $$ 
BEGIN 
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='instance_types' AND column_name='gpu_count') THEN
        ALTER TABLE instance_types ADD COLUMN gpu_count INTEGER DEFAULT 0;
    END IF;
END $$;
