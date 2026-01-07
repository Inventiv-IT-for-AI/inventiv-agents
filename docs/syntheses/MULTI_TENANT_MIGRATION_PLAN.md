# Plan de Migration Multi-Tenant Progressive

## Objectif
Migrer progressivement la plateforme vers un modèle multi-tenant complet, avec des déploiements intermédiaires compatibles et testables.

## Principe de compatibilité backward
- **Chaque phase doit être déployable indépendamment**
- **Les endpoints existants continuent de fonctionner** (mode "legacy" ou "personal")
- **Les nouvelles fonctionnalités sont opt-in** (workspace org = optionnel au début)
- **Migration des données progressive** (pas de big-bang)

---

## Phase 0 : Préparation DB (✅ DÉJÀ FAIT)
**Statut** : Tables créées dans baseline, PRIMARY KEY / FOREIGN KEY à ajouter

### Migration SQL
- ✅ Tables : `organizations`, `organization_memberships`, `organization_models`, `organization_model_shares`, `workbench_projects`, `workbench_runs`
- ✅ Colonnes enrichies : `users.current_organization_id`, `api_keys.organization_id`, `finops.inference_usage` (provider/consumer org)
- ⏳ **À faire** : Migration `20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql`

### Tests
- [ ] Appliquer migration sur DB de test
- [ ] Vérifier contraintes PRIMARY KEY / FOREIGN KEY
- [ ] Vérifier index de performance

---

## Phase 1 : CRUD Organisation (✅ DÉJÀ FAIT)
**Statut** : Endpoints de base implémentés

### API Endpoints (✅ Existent)
- `GET /organizations` - Liste des orgs du user
- `POST /organizations` - Créer une org
- `PUT /organizations/current` - Changer workspace courant

### Frontend (✅ Existe)
- Switch workspace dans `AccountSection`
- Dialog création org

### Tests
- [ ] Créer org → vérifier membership owner automatique
- [ ] Switch workspace → vérifier JWT mis à jour
- [ ] Mode Personal → vérifier `current_organization_id = NULL`

---

## Phase 2 : CRUD Memberships + RBAC (✅ DÉJÀ FAIT)
**Statut** : Gestion membres + rôles implémentée

### API Endpoints (✅ Existent)
- `GET /organizations/current/members` - Liste membres
- `PUT /organizations/current/members/:user_id` - Changer rôle
- `DELETE /organizations/current/members/:user_id` - Retirer membre
- `POST /organizations/current/leave` - Quitter org

### Frontend (✅ Existe)
- Dialog `OrganizationMembersDialog` avec liste + changement rôle

### Tests
- [ ] Invariant "dernier owner" → bloqué
- [ ] RBAC délégation → Owner peut tout, Admin/Manager limités
- [ ] Audit logs → vérifier écriture `action_logs`

---

## Phase 3 : Invitations (⏳ À FAIRE)
**Statut** : À implémenter

### Migration SQL
```sql
CREATE TABLE public.organization_invitations (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email text NOT NULL,
    role text NOT NULL DEFAULT 'user' CHECK (role IN ('owner', 'admin', 'manager', 'user')),
    token text NOT NULL UNIQUE,
    invited_by_user_id uuid NOT NULL REFERENCES users(id),
    expires_at timestamp with time zone NOT NULL,
    accepted_at timestamp with time zone,
    revoked_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT organization_invitations_org_email_unique UNIQUE (organization_id, email) WHERE (accepted_at IS NULL AND revoked_at IS NULL)
);

CREATE INDEX idx_organization_invitations_token ON organization_invitations(token) WHERE (accepted_at IS NULL AND revoked_at IS NULL);
CREATE INDEX idx_organization_invitations_org ON organization_invitations(organization_id, created_at DESC);
```

### API Endpoints (À créer)
- `POST /organizations/current/invitations` - Inviter par email
- `GET /organizations/current/invitations` - Liste invitations (pending/accepted/revoked)
- `DELETE /organizations/current/invitations/:id` - Révoquer invitation
- `POST /organizations/invitations/:token/accept` - Accepter invitation (public, peut créer user si inexistant)

### Frontend (À créer)
- Section "Invitations" dans `OrganizationMembersDialog`
- Formulaire inviter (email + rôle)
- Liste invitations avec statut + actions (révoquer)

### Tests
- [ ] Inviter user existant → vérifier email reçu (mock)
- [ ] Inviter user inexistant → vérifier création compte + membership
- [ ] Token expiration → vérifier rejet après `expires_at`
- [ ] RBAC invitations → Owner/Admin/Manager peuvent inviter

---

## Phase 4 : Scoping API Keys (⏳ À FAIRE)
**Statut** : Colonne `organization_id` existe, logique à ajouter

### Migration SQL
- ✅ Colonne `api_keys.organization_id` existe déjà (nullable)

### API Endpoints (À modifier)
**Fichier** : `inventiv-api/src/api_keys.rs`

**Changements** :
- `list_api_keys()` : Filtrer par `organization_id` si workspace org
- `create_api_key()` : Définir `organization_id` si workspace org
- `update_api_key()` : Vérifier RBAC (Admin/Owner pour org-owned keys)
- `revoke_api_key()` : Vérifier RBAC

**Compatibilité backward** :
- Si `current_organization_id IS NULL` → `organization_id = NULL` (user-owned)
- Si `current_organization_id IS NOT NULL` → `organization_id = current_organization_id` (org-owned)

### Frontend (À modifier)
**Fichier** : `inventiv-frontend/src/app/(app)/api-keys/page.tsx`

**Changements** :
- Badge "Personal" vs "Org: <Name>" sur chaque clé
- Filtre workspace (si org) → voir seulement les clés org
- Création → automatiquement scoped au workspace courant

### Tests
- [ ] Mode Personal → créer clé → `organization_id = NULL`
- [ ] Mode Org → créer clé → `organization_id = org_id`
- [ ] Lister clés → filtrer selon workspace
- [ ] RBAC → User ne peut pas modifier clés org-owned (sauf Owner/Admin)

---

## Phase 5 : Scoping Instances (⏳ À FAIRE)
**Statut** : Colonne `organization_id` à ajouter, logique à implémenter

### Migration SQL
```sql
-- Ajouter colonne organization_id à instances
ALTER TABLE public.instances
    ADD COLUMN organization_id uuid REFERENCES organizations(id) ON DELETE CASCADE;

CREATE INDEX idx_instances_organization_id ON instances(organization_id) WHERE (organization_id IS NOT NULL);

-- Migration des données existantes (optionnel, peut rester NULL pour instances legacy)
-- UPDATE instances SET organization_id = ... WHERE ... (si nécessaire)
```

### API Endpoints (À modifier)
**Fichier** : `inventiv-api/src/main.rs` (handlers `list_instances`, `create_deployment`, etc.)

**Changements** :
- `list_instances()` : Filtrer par `organization_id` si workspace org
- `create_deployment()` : Définir `organization_id` si workspace org
- `get_instance()` : Vérifier accès (membre org ou instance legacy)
- `terminate_instance()` : Vérifier RBAC (Admin/Owner pour org-owned instances)
- `reinstall_instance()` : Vérifier RBAC

**Compatibilité backward** :
- Instances existantes → `organization_id = NULL` (legacy, accessibles à tous)
- Nouvelles instances en mode Personal → `organization_id = NULL`
- Nouvelles instances en mode Org → `organization_id = current_organization_id`

### Frontend (À modifier)
**Fichier** : `inventiv-frontend/src/app/(app)/instances/page.tsx`

**Changements** :
- Badge "Personal" vs "Org: <Name>" sur chaque instance
- Filtre workspace (si org) → voir seulement les instances org
- Création → automatiquement scoped au workspace courant
- WorkspaceBanner visible sur la page

### Tests
- [ ] Mode Personal → créer instance → `organization_id = NULL`
- [ ] Mode Org → créer instance → `organization_id = org_id`
- [ ] Lister instances → filtrer selon workspace
- [ ] RBAC → User ne peut pas terminer instances org-owned (sauf Owner/Admin)
- [ ] Instances legacy (`organization_id = NULL`) → accessibles à tous

---

## Phase 6 : Scoping Models (Catalog) (⏳ À FAIRE)
**Statut** : Colonne `organization_id` à ajouter, logique à implémenter

### Migration SQL
```sql
-- Ajouter colonne organization_id à models (catalog)
ALTER TABLE public.models
    ADD COLUMN organization_id uuid REFERENCES organizations(id) ON DELETE CASCADE;

CREATE INDEX idx_models_organization_id ON models(organization_id) WHERE (organization_id IS NOT NULL);

-- Migration des données existantes (optionnel, peut rester NULL pour models globaux/publics)
```

### API Endpoints (À modifier)
**Fichier** : `inventiv-api/src/main.rs` (handlers `list_models`, `create_model`, etc.)

**Changements** :
- `list_models()` : Filtrer par `organization_id` si workspace org + modèles publics (`organization_id IS NULL`)
- `create_model()` : Définir `organization_id` si workspace org
- `update_model()` : Vérifier RBAC (Admin/Owner pour org-owned models)
- `delete_model()` : Vérifier RBAC

**Compatibilité backward** :
- Models existants → `organization_id = NULL` (globaux/publics)
- Nouveaux models en mode Personal → `organization_id = NULL` (publics)
- Nouveaux models en mode Org → `organization_id = current_organization_id` (privés à l'org)

### Frontend (À modifier)
**Fichier** : `inventiv-frontend/src/app/(app)/models/page.tsx`

**Changements** :
- Badge "Public" vs "Org: <Name>" sur chaque modèle
- Filtre workspace (si org) → voir modèles org + publics
- Création → automatiquement scoped au workspace courant

### Tests
- [ ] Mode Personal → créer modèle → `organization_id = NULL` (public)
- [ ] Mode Org → créer modèle → `organization_id = org_id` (privé)
- [ ] Lister modèles → voir publics + modèles org (si workspace org)
- [ ] RBAC → User ne peut pas modifier modèles org-owned (sauf Owner/Admin)

---

## Phase 7 : Scoping Users Management (⏳ À FAIRE)
**Statut** : Endpoints existent, logique RBAC à ajouter

### API Endpoints (À modifier)
**Fichier** : `inventiv-api/src/users_endpoint.rs`

**Changements** :
- `list_users()` : Si workspace org → voir seulement membres de l'org
- `create_user()` : Si workspace org → créer user + membership automatique (rôle par défaut)
- `update_user()` : Vérifier RBAC (Admin/Owner pour modifier membres org)
- `delete_user()` : Vérifier RBAC + invariant "dernier owner"

**Compatibilité backward** :
- Mode Personal → voir tous les users (comportement actuel, admin-only)
- Mode Org → voir seulement membres de l'org

### Frontend (À modifier)
**Fichier** : `inventiv-frontend/src/app/(app)/users/page.tsx`

**Changements** :
- WorkspaceBanner visible
- Si workspace org → liste filtrée membres org uniquement
- Création user → automatiquement ajouté comme membre org (rôle par défaut)

### Tests
- [ ] Mode Personal → voir tous users (admin)
- [ ] Mode Org → voir seulement membres org
- [ ] Créer user en mode Org → vérifier membership automatique
- [ ] RBAC → User ne peut pas modifier membres (sauf Owner/Admin)

---

## Phase 8 : Scoping Settings (Infrastructure) (⏳ À FAIRE)
**Statut** : Settings globaux → à scoper par org (optionnel)

### Migration SQL (Optionnel)
```sql
-- Ajouter colonne organization_id aux tables settings (si nécessaire)
-- Exemples :
-- ALTER TABLE public.provider_settings ADD COLUMN organization_id uuid REFERENCES organizations(id) ON DELETE CASCADE;
-- ALTER TABLE public.instance_types ADD COLUMN organization_id uuid REFERENCES organizations(id) ON DELETE CASCADE;
```

**Note** : Cette phase est **optionnelle** selon le besoin métier :
- **Option A** : Settings restent globaux (partagés entre toutes les orgs)
- **Option B** : Settings org-scopés (chaque org a ses propres providers/regions/zones/types)

### API Endpoints (À modifier si Option B)
**Fichier** : `inventiv-api/src/settings.rs`, `provider_settings.rs`

**Changements** :
- Filtrer par `organization_id` si workspace org
- Création → définir `organization_id` si workspace org

### Frontend (À modifier si Option B)
**Fichier** : `inventiv-frontend/src/app/(app)/settings/page.tsx`

**Changements** :
- WorkspaceBanner visible
- Filtre workspace (si org) → voir settings org uniquement

---

## Phase 9 : Scoping FinOps (⏳ À FAIRE)
**Statut** : Colonnes enrichies existent, logique à implémenter

### Migration SQL
- ✅ Colonnes `finops.inference_usage.provider_organization_id`, `consumer_organization_id` existent déjà

### API Endpoints (À modifier)
**Fichier** : `inventiv-api/src/finops.rs`

**Changements** :
- `get_cost_current()` : Filtrer par `organization_id` si workspace org
- `get_costs_dashboard_*()` : Filtrer par `organization_id` si workspace org
- Dashboards → voir coûts org uniquement (ou coûts en tant que provider si applicable)

**Compatibilité backward** :
- Mode Personal → voir coûts user uniquement (`customer_id`)
- Mode Org → voir coûts org (`consumer_organization_id` ou `provider_organization_id`)

### Frontend (À modifier)
**Fichier** : `inventiv-frontend/src/app/(app)/(dashboard)/page.tsx`

**Changements** :
- WorkspaceBanner visible
- Dashboards → filtrer selon workspace courant

### Tests
- [ ] Mode Personal → voir coûts user
- [ ] Mode Org → voir coûts org (consumer + provider si applicable)

---

## Phase 10 : Migration Frontend - Modules "For All Users" vs "Admin Only" (⏳ À FAIRE)
**Statut** : Réorganisation UI selon workspace

### Modules "For All Users" (Personal + Org)
Ces modules doivent fonctionner **sans organisation** :
- ✅ `/chat` - Chat (déjà scoped workspace)
- ✅ `/workbench` - Workbench (déjà scoped workspace)
- `/api-keys` - API Keys (après Phase 4)
- `/models` - Models catalog (après Phase 6, voir publics + org)

### Modules "Admin Only" (Org mandatory)
Ces modules nécessitent **une organisation** :
- `/instances` - Instances (après Phase 5)
- `/users` - Users management (après Phase 7)
- `/settings` - Infrastructure settings (après Phase 8, si Option B)
- `/finops` - FinOps dashboards (après Phase 9)

### Frontend Changes
**Fichier** : `inventiv-frontend/src/app/(app)/layout.tsx`

**Changements** :
- Vérifier `current_organization_id` avant d'afficher certains modules
- Rediriger vers création org si nécessaire
- Badge "Org required" sur les liens sidebar pour modules admin

**Fichier** : `inventiv-frontend/src/components/Sidebar.tsx`

**Changements** :
- Masquer/cacher liens selon workspace (Personal vs Org)
- Badge visuel "Org required" sur certains liens

### Tests
- [ ] Mode Personal → voir seulement modules "For All Users"
- [ ] Mode Org → voir tous les modules
- [ ] Tentative accès module admin sans org → redirection création org

---

## Phase 11 : Double Activation Tech/Eco (⏳ À FAIRE)
**Statut** : Concept défini dans roadmap, à implémenter

### Migration SQL
```sql
-- Ajouter colonnes double activation aux ressources
ALTER TABLE public.instances
    ADD COLUMN tech_activated boolean DEFAULT false NOT NULL,
    ADD COLUMN eco_activated boolean DEFAULT false NOT NULL;

ALTER TABLE public.models
    ADD COLUMN tech_activated boolean DEFAULT false NOT NULL,
    ADD COLUMN eco_activated boolean DEFAULT false NOT NULL;

-- (Répéter pour autres ressources si nécessaire)
```

### API Logic (À modifier)
**Fichier** : `inventiv-api/src/rbac.rs` (déjà défini)

**Changements** :
- Endpoints activation → vérifier RBAC (`can_set_activation_flag`)
- Endpoints utilisation → vérifier `tech_activated AND eco_activated`

### Frontend (À modifier)
**Fichier** : Tous les modules concernés

**Changements** :
- Badge "Non opérationnel" si un flag manque
- Alerte explicite "Activation tech requise (Admin/Owner)" ou "Activation eco requise (Manager/Owner)"
- Boutons activation selon RBAC

### Tests
- [ ] Admin peut activer tech, pas eco
- [ ] Manager peut activer eco, pas tech
- [ ] Owner peut activer les deux
- [ ] Ressource non opérationnelle → bloquée en utilisation

---

## Phase 12 : Model Sharing & Billing (⏳ À FAIRE)
**Statut** : Tables existent, logique à implémenter

### Migration SQL
- ✅ Tables `organization_models`, `organization_model_shares` existent déjà
- ✅ Colonnes `finops.inference_usage` enrichies existent déjà

### API Endpoints (À créer)
- `POST /organizations/current/models` - Publier un modèle (créer `organization_model`)
- `GET /organizations/current/models` - Liste offerings de l'org
- `PUT /organizations/current/models/:id` - Mettre à jour offering (visibility, access_policy, pricing)
- `POST /organizations/current/models/:id/shares` - Partager avec une org consumer
- `GET /organizations/current/models/:id/shares` - Liste shares actifs
- `GET /models/community` - Catalogue community (offerings publics/unlisted)

### Frontend (À créer)
- Page `/organizations/current/models` - Gestion offerings
- Page `/models/community` - Catalogue community
- Workflow demande accès → approbation → entitlement

### Tests
- [ ] Publier offering → vérifier `organization_model` créé
- [ ] Partage inter-org → vérifier `organization_model_share` créé
- [ ] Usage → vérifier chargeback dans `finops.inference_usage`

---

## Checklist de Déploiement par Phase

Pour chaque phase :
- [ ] Migration SQL testée localement
- [ ] API endpoints modifiés + tests unitaires
- [ ] Frontend modifié + tests manuels
- [ ] Compatibilité backward vérifiée (mode Personal fonctionne toujours)
- [ ] Documentation mise à jour
- [ ] Commit + push
- [ ] Déploiement staging → tests
- [ ] Déploiement prod (si staging OK)

---

## Ordre Recommandé de Déploiement

1. **Phase 0** : Migration PRIMARY KEY / FOREIGN KEY (critique, bloque tout)
2. **Phase 3** : Invitations (nécessaire pour onboarding)
3. **Phase 4** : API Keys scoping (impact faible, facile à tester)
4. **Phase 5** : Instances scoping (impact moyen, core feature)
5. **Phase 6** : Models scoping (impact moyen)
6. **Phase 7** : Users scoping (impact moyen)
7. **Phase 9** : FinOps scoping (impact faible, dashboards)
8. **Phase 10** : Migration Frontend modules (réorganisation UI)
9. **Phase 8** : Settings scoping (optionnel, selon besoin)
10. **Phase 11** : Double activation (feature avancée)
11. **Phase 12** : Model sharing & billing (feature avancée)

---

## Notes Importantes

### Compatibilité Backward
- **Toujours** permettre `current_organization_id = NULL` (mode Personal)
- Les ressources existantes (`organization_id = NULL`) restent accessibles (legacy)
- Les nouveaux endpoints/features sont **opt-in** (workspace org = optionnel)

### Tests
- Chaque phase doit être testée **indépendamment**
- Vérifier mode Personal + mode Org pour chaque changement
- Tests RBAC pour chaque endpoint modifié

### Rollback
- Chaque migration SQL doit être **réversible** (DROP COLUMN, etc.)
- Les endpoints doivent fonctionner **sans** les nouvelles colonnes (si migration partielle)

