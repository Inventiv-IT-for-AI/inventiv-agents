-- Add Cost to Instance Types (e.g. 1.5000)
ALTER TABLE instance_types ADD COLUMN cost_per_hour NUMERIC(10, 4) DEFAULT 0.0;

-- Add Archive Flag to Instances
ALTER TABLE instances ADD COLUMN is_archived BOOLEAN NOT NULL DEFAULT FALSE;
