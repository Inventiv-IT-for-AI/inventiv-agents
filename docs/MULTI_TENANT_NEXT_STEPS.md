# Multi-Tenant : Prochaines Étapes - Janvier 2025

**Date** : 2025-01-XX  
**Contexte** : Après refactoring majeur, tests unitaires/intégration/E2E, gestion mail SMTP, password reset, réorganisation code API

---

## 📊 État Actuel du Projet

### ✅ Réalisations Récentes

#### 1. Refactoring & Qualité Code
- ✅ **Réorganisation API** : `main.rs` (~3500 lignes → ~86 lignes)
  - Extraction en modules : `config/`, `setup/`, `routes/`, `handlers/`
  - Meilleure maintenabilité et testabilité
- ✅ **Tests** : Infrastructure complète
  - Tests unitaires (Rust)
  - Tests d'intégration (`axum-test`)
  - Tests E2E (Mock provider uniquement)
- ✅ **Upgrade Axum 0.8** : Migration complète + compatibilité OpenAPI
- ✅ **Gestion Mail SMTP** : Intégration Scaleway TEM
- ✅ **Password Reset Flow** : Tokens sécurisés, emails, endpoints API complets

#### 2. Multi-Tenant - Fondations ✅

**Base de Données** :
- ✅ Tables créées : `organizations`, `organization_memberships`, `organization_models`, `organization_model_shares`
- ✅ Colonnes enrichies : `api_keys.organization_id`, `workbench_projects.organization_id`, `workbench_runs.organization_id`
- ✅ Migration PK/FK : `20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql` créée
- ⚠️ **À vérifier** : `users.current_organization_id` existe encore dans baseline (à migrer vers `user_sessions`)

**API Backend** :
- ✅ Module `organizations.rs` : CRUD orgs, membres, rôles
- ✅ Module `rbac.rs` : Rôles Owner/Admin/Manager/User, règles de délégation
- ✅ Bootstrap org "Inventiv IT" avec admin comme owner
- ✅ Endpoints : `GET /organizations`, `POST /organizations`, `PUT /organizations/current`, gestion membres

**Frontend** :
- ✅ `AccountSection.tsx` : Switch workspace (Personal vs Org)
- ✅ `OrganizationMembersDialog.tsx` : Gestion membres + rôles
- ✅ `WorkspaceBanner.tsx` : Affichage workspace courant
- ✅ `Sidebar.tsx` : Navigation avec badge workspace

#### 3. Architecture Sessions Multi-Org ✅

**État** : **✅ COMPLÈTE** (voir `docs/PHASE1_REALIGNMENT.md` pour détails)

**Implémenté** :
- ✅ Table `user_sessions` créée dans baseline
- ✅ `AuthUser` enrichi avec `session_id`, `current_organization_role`
- ✅ JWT Claims enrichis avec `session_id`, `current_organization_role`, `jti`
- ✅ `login()` crée session en DB
- ✅ `logout()` révoque session
- ✅ `set_current_organization()` met à jour session en DB
- ✅ `GET /auth/sessions` (liste sessions actives) - **IMPLÉMENTÉ**
- ✅ `POST /auth/sessions/:id/revoke` (révoquer session) - **IMPLÉMENTÉ**
- ✅ `MeResponse` enrichi avec `current_organization_role` - **IMPLÉMENTÉ**
- ✅ Frontend : Type `Me` enrichi + `SessionsDialog.tsx` créé et intégré - **IMPLÉMENTÉ**
- ✅ Tests unitaires complets dans `auth.rs`

---

## 🎯 Objectifs Multi-Tenant - Prochaines Étapes

### Phase 1 : Finaliser Architecture Sessions ✅ **COMPLÈTE**

**Objectif** : Permettre plusieurs sessions simultanées avec organisations différentes

**Statut** : **✅ COMPLÈTE** - Voir `docs/PHASE1_REALIGNMENT.md` pour détails complets

**Implémenté** :
- ✅ Table `user_sessions` créée et fonctionnelle
- ✅ `AuthUser` enrichi avec `session_id`, `current_organization_role`
- ✅ JWT Claims enrichis avec `session_id`, `current_organization_role`, `jti`
- ✅ `login()` crée session en DB avec org + rôle
- ✅ `logout()` révoque session en DB
- ✅ `set_current_organization()` met à jour session en DB + régénère JWT
- ✅ `GET /auth/sessions` implémenté (liste sessions actives)
- ✅ `POST /auth/sessions/:id/revoke` implémenté (révoquer session)
- ✅ `MeResponse` enrichi avec `current_organization_role`
- ✅ Type `Me` enrichi avec `current_organization_role`
- ✅ `SessionsDialog.tsx` créé et intégré dans `AccountSection.tsx`
- ✅ Tests unitaires complets dans `auth.rs`

**Note** : La Phase 1 est complètement implémentée et fonctionnelle. On peut passer directement à la Phase 2.

---

### Phase 2 : Scoping Instances par Organisation (Priorité Haute)

**Objectif** : Isoler les instances par `organization_id` + RBAC

**Tâches** :

1. **Migration SQL**
   - [ ] Créer migration : `ALTER TABLE instances ADD COLUMN organization_id uuid REFERENCES organizations(id) ON DELETE SET NULL`
   - [ ] Migration backward-compat : `organization_id` nullable (instances legacy restent accessibles)
   - [ ] Index : `CREATE INDEX idx_instances_org ON instances(organization_id) WHERE organization_id IS NOT NULL`

2. **API Backend**
   - [ ] Modifier `list_instances()` → filtrer par `organization_id` si workspace org
   - [ ] Modifier `create_deployment()` → définir `organization_id` si workspace org
   - [ ] Modifier `get_instance()`, `terminate_instance()`, `reinstall_instance()` → vérifier accès RBAC
   - [ ] RBAC :
     - Owner/Admin : tout (provision/terminate/reinstall)
     - Manager : voir + dashboards financiers
     - User : voir seulement

3. **Frontend**
   - [ ] Badge "Personal" vs "Org: <Name>" sur instances
   - [ ] `WorkspaceBanner` visible sur page instances
   - [ ] Filtre workspace (optionnel)
   - [ ] Masquer boutons selon rôle org

4. **Tests**
   - [ ] Mode Personal → instances legacy accessibles
   - [ ] Mode Org → seulement instances org-owned
   - [ ] Tests RBAC : User ne peut pas terminer instances org
   - [ ] Tests backward-compat : instances legacy restent accessibles

**Fichiers** :
- Migration SQL (à créer)
- `inventiv-api/src/handlers/deployments.rs`
- `inventiv-frontend/src/app/(app)/instances/page.tsx`

**Estimation** : 4-6h développement + 2h tests

---

### Phase 3 : Scoping Models par Organisation (Priorité Haute)

**Objectif** : Isoler les modèles par `organization_id` + visibilité publique/privée

**Tâches** :

1. **Migration SQL**
   - [ ] Créer migration : `ALTER TABLE models ADD COLUMN organization_id uuid REFERENCES organizations(id) ON DELETE SET NULL`
   - [ ] Migration backward-compat : `organization_id` nullable (modèles publics restent accessibles)
   - [ ] Index : `CREATE INDEX idx_models_org ON models(organization_id) WHERE organization_id IS NOT NULL`

2. **API Backend**
   - [ ] Modifier `list_models()` → filtrer par `organization_id` + modèles publics (`organization_id IS NULL`)
   - [ ] Modifier `create_model()` → définir `organization_id` si workspace org
   - [ ] Modifier `update_model()`, `delete_model()` → vérifier RBAC
   - [ ] RBAC :
     - Owner/Admin : tout (CRUD)
     - Manager : voir + pricing
     - User : voir seulement

3. **Frontend**
   - [ ] Badge "Public" vs "Org: <Name>" sur modèles
   - [ ] Filtre workspace sur page modèles
   - [ ] Masquer boutons selon rôle org

4. **Tests**
   - [ ] Mode Personal → modèles publics accessibles
   - [ ] Mode Org → modèles privés org + publics
   - [ ] Tests RBAC : User ne peut pas modifier modèles org

**Fichiers** :
- Migration SQL (à créer)
- `inventiv-api/src/handlers/models.rs` (si existe) ou `main.rs`
- `inventiv-frontend/src/app/(app)/models/page.tsx`

**Estimation** : 4-6h développement + 2h tests

---

### Phase 4 : Invitations d'Utilisateurs (Priorité Haute)

**Objectif** : Permettre d'inviter des users par email dans une organisation

**Tâches** :

1. **Migration SQL**
   - [ ] Créer table `organization_invitations` :
     ```sql
     CREATE TABLE organization_invitations (
         id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
         organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
         email text NOT NULL,
         role text NOT NULL CHECK (role IN ('owner', 'admin', 'manager', 'user')),
         token text NOT NULL UNIQUE,
         invited_by_user_id uuid NOT NULL REFERENCES users(id),
         created_at timestamptz NOT NULL DEFAULT now(),
         expires_at timestamptz NOT NULL,
         accepted_at timestamptz,
         revoked_at timestamptz
     );
     ```

2. **API Backend**
   - [ ] `POST /organizations/current/invitations` (inviter par email + rôle)
   - [ ] `GET /organizations/current/invitations` (liste pending/accepted/revoked)
   - [ ] `DELETE /organizations/current/invitations/:id` (révoquer)
   - [ ] `POST /organizations/invitations/:token/accept` (public, peut créer user si inexistant)
   - [ ] RBAC : Owner/Admin/Manager peuvent inviter

3. **Frontend**
   - [ ] Section "Invitations" dans `OrganizationMembersDialog`
   - [ ] Formulaire inviter (email + rôle)
   - [ ] Liste invitations avec statut + actions
   - [ ] Page publique acceptation invitation (si user inexistant)

4. **Tests**
   - [ ] Inviter user existant → membership créé
   - [ ] Inviter user inexistant → compte créé + membership
   - [ ] Tests RBAC : Owner/Admin/Manager peuvent inviter
   - [ ] Test expiration token

**Fichiers** :
- Migration SQL (à créer)
- `inventiv-api/src/organizations.rs` (endpoints invitations)
- `inventiv-frontend/src/components/account/OrganizationMembersDialog.tsx`

**Estimation** : 6-8h développement + 2h tests

---

### Phase 5 : Visibilité Modules/Fonctions selon Workspace + Rôle (Priorité Haute)

**Objectif** : Masquer/afficher modules et fonctions selon workspace (Personal vs Org) et rôle org

**Tâches** :

1. **Identifier Modules**
   - [ ] Modules "For All Users" : Chat, Workbench, API Keys (Personal)
   - [ ] Modules "Admin Only" : Settings, Users, Instances, Models
   - [ ] Modules "Org Required" : Instances (org-scopées), Models (org-scopées), Members, Invitations

2. **Backend - Middleware RBAC**
   - [ ] Créer middleware `require_org_role(roles: Vec<OrgRole>)` pour endpoints org-scopés
   - [ ] Créer middleware `require_org_or_personal()` pour endpoints flexibles
   - [ ] Ajouter vérifications RBAC dans endpoints existants

3. **Frontend - Affichage Conditionnel**
   - [ ] Modifier `Sidebar.tsx` → masquer liens selon workspace + rôle org
   - [ ] Badge "Org required" sur liens admin
   - [ ] Redirection création org si nécessaire
   - [ ] `WorkspaceBanner` visible sur toutes les pages org-scopées

4. **Tests**
   - [ ] Mode Personal → modules user-only visibles
   - [ ] Mode Org User → modules user-only + org-read-only visibles
   - [ ] Mode Org Admin → tous modules visibles
   - [ ] Tests RBAC : User ne peut pas accéder endpoints admin

**Fichiers** :
- `inventiv-api/src/rbac.rs` (middleware)
- `inventiv-frontend/src/components/Sidebar.tsx`
- `inventiv-frontend/src/app/(app)/layout.tsx`

**Estimation** : 4-6h développement + 2h tests

---

### Phase 6 : Scoping API Keys, Users, FinOps (Priorité Moyenne)

**Objectif** : Isoler API Keys, Users, FinOps par `organization_id`

**Tâches** :

#### 6.1 Scoping API Keys
- [ ] Modifier `list_api_keys()` → filtrer par `organization_id`
- [ ] Modifier `create_api_key()` → définir `organization_id` si workspace org
- [ ] UI : Badge "Personal" vs "Org: <Name>" sur clés
- [ ] Tests : Mode Personal → clés user-owned, Mode Org → clés org-owned

#### 6.2 Scoping Users
- [ ] Modifier `list_users()` → filtrer membres org si workspace org
- [ ] Modifier `create_user()` → créer membership automatique si workspace org
- [ ] UI : Liste filtrée membres org
- [ ] Tests : Mode Personal → voir tous users (admin), Mode Org → voir seulement membres

#### 6.3 Scoping FinOps
- [ ] Modifier `get_cost_current()` → filtrer par `organization_id`
- [ ] Modifier `get_costs_dashboard_*()` → filtrer par `organization_id`
- [ ] UI : Dashboards filtrés selon workspace
- [ ] Tests : Mode Personal → coûts user, Mode Org → coûts org

**Estimation** : 6-8h développement + 3h tests

---

### Phase 7 : Double Activation Tech/Eco (Priorité Basse)

**Objectif** : Activation technique (Admin) + économique (Manager) par ressource

**Tâches** :
- [ ] Ajouter colonnes `tech_activated_by`, `eco_activated_by` sur ressources (instances, models, etc.)
- [ ] Modifier endpoints pour vérifier double activation
- [ ] UI : État "non opérationnel" + alerte flag manquant
- [ ] Tests : Ressource non opérationnelle si un flag manque

**Estimation** : 8-10h développement + 3h tests

---

### Phase 8 : Model Sharing & Billing (Priorité Basse)

**Objectif** : Partage de modèles entre orgs avec facturation au token

**Tâches** :
- [ ] CRUD `organization_models` (publish/unpublish)
- [ ] CRUD `organization_model_shares` (grant/pause/revoke + pricing)
- [ ] Résolution `org_slug/model_code` dans OpenAI proxy
- [ ] Ingestion `finops.inference_usage` avec chargeback
- [ ] Dashboards consommation par org/provider/consumer

**Estimation** : 15-20h développement + 5h tests

---

## 📋 Plan d'Action Recommandé

### Sprint 1 (Semaine 1) : Finaliser Sessions + Scoping Instances

**Jour 1-2** : Finaliser Architecture Sessions
- Vérifier état DB
- Compléter endpoints sessions
- Compléter Frontend sessions
- Tests

**Jour 3-5** : Scoping Instances
- Migration SQL
- API Backend
- Frontend
- Tests

**Livrable** : Sessions multi-org fonctionnelles + Instances scopées par org

---

### Sprint 2 (Semaine 2) : Scoping Models + Invitations

**Jour 1-3** : Scoping Models
- Migration SQL
- API Backend
- Frontend
- Tests

**Jour 4-5** : Invitations
- Migration SQL
- API Backend
- Frontend
- Tests

**Livrable** : Models scopés par org + Invitations fonctionnelles

---

### Sprint 3 (Semaine 3) : Visibilité Modules + Scoping API Keys/Users/FinOps

**Jour 1-2** : Visibilité Modules
- Middleware RBAC
- Frontend affichage conditionnel
- Tests

**Jour 3-5** : Scoping API Keys/Users/FinOps
- API Backend
- Frontend
- Tests

**Livrable** : Modules visibles selon workspace/rôle + API Keys/Users/FinOps scopés

---

## 🔍 Points d'Attention

### 1. Compatibilité Backward
- ✅ Mode Personal (`current_organization_id = NULL`) doit toujours fonctionner
- ✅ Ressources legacy (`organization_id = NULL`) restent accessibles
- ✅ Nouvelles features sont opt-in (workspace org = optionnel)

### 2. Performance
- ⚠️ Ajouter `current_organization_role` dans JWT pour éviter requêtes DB supplémentaires
- ⚠️ Index sur `(organization_id, user_id)` pour `organization_memberships`
- ⚠️ Index sur `organization_id` pour toutes les tables scopées

### 3. Sécurité
- ⚠️ RBAC vérifié à chaque endpoint métier
- ⚠️ Audit logs immuables pour changements de rôles/membres
- ⚠️ Invariant "dernier owner" non révocable

### 4. Tests
- ⚠️ Tests unitaires API (Rust)
- ⚠️ Tests manuels Frontend (mode Personal + mode Org)
- ⚠️ Tests RBAC (rôles Owner/Admin/Manager/User)
- ⚠️ Tests compatibilité backward (mode Personal)

---

## 📚 Documentation Existante

- `docs/MULTI_TENANT_MIGRATION_PLAN.md` - Plan détaillé par phase
- `docs/MULTI_TENANT_MIGRATION_TRACKER.md` - Tracker visuel des phases
- `docs/MULTI_TENANT_STATUS_2025.md` - État des lieux actuel
- `docs/RBAC_ANALYSIS.md` - Analyse détaillée RBAC
- `docs/SESSION_ARCHITECTURE_PROPOSAL.md` - Proposition architecture sessions
- `docs/SESSION_IMPLEMENTATION_STATUS.md` - État implémentation sessions
- `docs/MULTI_TENANT_ROADMAP.md` - Roadmap cible
- `docs/MULTI_TENANT_MODEL_SHARING_BILLING.md` - Design partage modèles + billing

---

## 🎯 Objectifs Finaux

1. **Isolation complète** : Instances, Models, API Keys, Users, FinOps scopés par organisation
2. **RBAC complet** : Permissions selon rôle org (Owner/Admin/Manager/User)
3. **Visibilité conditionnelle** : Modules/fonctions affichés selon workspace + rôle
4. **Multi-sessions** : Plusieurs sessions simultanées avec orgs différentes
5. **Onboarding fluide** : Invitations par email avec création de compte automatique

---

**Prochaine étape** : Commencer par la Phase 1 (Finaliser Architecture Sessions) puis Phase 2 (Scoping Instances).

