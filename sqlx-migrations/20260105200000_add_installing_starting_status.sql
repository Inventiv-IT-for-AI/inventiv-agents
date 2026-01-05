-- Migration: Add 'installing' and 'starting' statuses to instance_status enum
-- These statuses provide more granular progress tracking during worker installation

-- Add 'installing' status (after 'booting', before 'ready')
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_enum 
        WHERE enumlabel = 'installing' 
        AND enumtypid = (SELECT oid FROM pg_type WHERE typname = 'instance_status')
    ) THEN
        ALTER TYPE public.instance_status ADD VALUE 'installing' AFTER 'booting';
    END IF;
END $$;

-- Add 'starting' status (after 'installing', before 'ready')
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_enum 
        WHERE enumlabel = 'starting' 
        AND enumtypid = (SELECT oid FROM pg_type WHERE typname = 'instance_status')
    ) THEN
        ALTER TYPE public.instance_status ADD VALUE 'starting' AFTER 'installing';
    END IF;
END $$;

