-- Migration: Add PRIMARY KEY and FOREIGN KEY constraints for multi-tenant tables
-- These constraints were missing from the baseline schema and are critical for referential integrity

-- PRIMARY KEY constraints
ALTER TABLE ONLY public.organizations
    ADD CONSTRAINT organizations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_pkey PRIMARY KEY (organization_id, user_id);

ALTER TABLE ONLY public.organization_models
    ADD CONSTRAINT organization_models_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.organization_model_shares
    ADD CONSTRAINT organization_model_shares_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.workbench_projects
    ADD CONSTRAINT workbench_projects_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.workbench_runs
    ADD CONSTRAINT workbench_runs_pkey PRIMARY KEY (id);

-- Note: api_keys already has PRIMARY KEY (api_keys_pkey) in baseline

-- FOREIGN KEY constraints for organization_memberships
ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_organization_id_fkey 
    FOREIGN KEY (organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.organization_memberships
    ADD CONSTRAINT organization_memberships_user_id_fkey 
    FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

-- FOREIGN KEY constraints for organization_models
ALTER TABLE ONLY public.organization_models
    ADD CONSTRAINT organization_models_organization_id_fkey 
    FOREIGN KEY (organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.organization_models
    ADD CONSTRAINT organization_models_model_id_fkey 
    FOREIGN KEY (model_id) REFERENCES public.models(id) ON DELETE RESTRICT;

-- FOREIGN KEY constraints for organization_model_shares
ALTER TABLE ONLY public.organization_model_shares
    ADD CONSTRAINT organization_model_shares_provider_organization_id_fkey 
    FOREIGN KEY (provider_organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.organization_model_shares
    ADD CONSTRAINT organization_model_shares_consumer_organization_id_fkey 
    FOREIGN KEY (consumer_organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.organization_model_shares
    ADD CONSTRAINT organization_model_shares_organization_model_id_fkey 
    FOREIGN KEY (organization_model_id) REFERENCES public.organization_models(id) ON DELETE CASCADE;

-- FOREIGN KEY constraints for workbench_projects
ALTER TABLE ONLY public.workbench_projects
    ADD CONSTRAINT workbench_projects_owner_user_id_fkey 
    FOREIGN KEY (owner_user_id) REFERENCES public.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.workbench_projects
    ADD CONSTRAINT workbench_projects_organization_id_fkey 
    FOREIGN KEY (organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

-- FOREIGN KEY constraints for workbench_runs
ALTER TABLE ONLY public.workbench_runs
    ADD CONSTRAINT workbench_runs_project_id_fkey 
    FOREIGN KEY (project_id) REFERENCES public.workbench_projects(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.workbench_runs
    ADD CONSTRAINT workbench_runs_organization_id_fkey 
    FOREIGN KEY (organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

-- FOREIGN KEY constraints for users.current_organization_id
ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_current_organization_id_fkey 
    FOREIGN KEY (current_organization_id) REFERENCES public.organizations(id) ON DELETE SET NULL;

-- FOREIGN KEY constraints for api_keys.organization_id
ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_organization_id_fkey 
    FOREIGN KEY (organization_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

-- FOREIGN KEY constraints for organizations.created_by_user_id
ALTER TABLE ONLY public.organizations
    ADD CONSTRAINT organizations_created_by_user_id_fkey 
    FOREIGN KEY (created_by_user_id) REFERENCES public.users(id) ON DELETE RESTRICT;

