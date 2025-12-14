-- Add Cost to Instance Types (e.g. 1.5000)
ALTER TABLE instance_types ADD COLUMN IF NOT EXISTS cost_per_hour NUMERIC(10, 4) DEFAULT 0.0;

-- Add Archive Flag to Instances
ALTER TABLE instances ADD COLUMN IF NOT EXISTS is_archived BOOLEAN NOT NULL DEFAULT FALSE;


