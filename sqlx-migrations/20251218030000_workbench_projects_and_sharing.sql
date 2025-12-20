-- Workbench: projects + run metadata (title, soft-delete) + org sharing
-- Non-breaking: adds new tables/columns; keeps existing rows valid.
-- Safe to run multiple times.

CREATE TABLE IF NOT EXISTS public.workbench_projects (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,

  -- Owner (personal workspace) or organization workspace
  owner_user_id uuid REFERENCES public.users(id) ON DELETE SET NULL,
  organization_id uuid REFERENCES public.organizations(id) ON DELETE CASCADE,

  name text NOT NULL,

  -- If true and organization_id is set, visible to all members of the org.
  shared_with_org boolean NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_workbench_projects_owner
  ON public.workbench_projects(owner_user_id, created_at DESC)
  WHERE owner_user_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_workbench_projects_org
  ON public.workbench_projects(organization_id, created_at DESC)
  WHERE organization_id IS NOT NULL AND deleted_at IS NULL;

ALTER TABLE public.workbench_runs
  ADD COLUMN IF NOT EXISTS title text,
  ADD COLUMN IF NOT EXISTS project_id uuid REFERENCES public.workbench_projects(id) ON DELETE SET NULL,
  ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES public.organizations(id) ON DELETE SET NULL,
  ADD COLUMN IF NOT EXISTS shared_with_org boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS deleted_at timestamptz;

CREATE INDEX IF NOT EXISTS idx_workbench_runs_deleted
  ON public.workbench_runs(deleted_at)
  WHERE deleted_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_workbench_runs_project
  ON public.workbench_runs(project_id, created_at DESC)
  WHERE project_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_workbench_runs_org_shared
  ON public.workbench_runs(organization_id, created_at DESC)
  WHERE organization_id IS NOT NULL AND shared_with_org = true AND deleted_at IS NULL;


