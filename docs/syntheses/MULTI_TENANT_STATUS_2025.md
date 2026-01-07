# État des Lieux Multi-Tenant - Janvier 2025

**Date de mise à jour** : 2025-01-06  
**Contexte** : Après refactoring majeur, ajout de tests, gestion mail SMTP, password reset, réorganisation code API

---

## 📊 Vue d'Ensemble

### ✅ Ce qui est Fait (Fondations)

#### 1. Base de Données
- ✅ **Tables créées** :
  - `organizations` (id, name, slug, created_by_user_id)
  - `organization_memberships` (organization_id, user_id, role)
  - `organization_models` (pré-câblage pour partage de modèles)
  - `organization_model_shares` (pré-câblage pour contrats provider→consumer)
  - `workbench_projects` (projets avec organization_id)
  - `workbench_runs` (sessions avec organization_id, shared_with_org)
- ✅ **Colonnes enrichies** :
  - `users.current_organization_id` (nullable) → **⚠️ À migrer vers `user_sessions`**
  - `api_keys.organization_id` (nullable) → prêt pour scoping
  - `finops.inference_usage` (provider_organization_id, consumer_organization_id, etc.)
- ⏳ **Migration PK/FK** : `20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql` créée mais pas encore appliquée

#### 2. API Backend (Rust)

**Module Organizations** (`inventiv-api/src/organizations.rs`) :
- ✅ `GET /organizations` - Liste des orgs du user
- ✅ `POST /organizations` - Créer une org (avec owner automatique)
- ✅ `PUT /organizations/current` - Changer workspace courant
- ✅ `GET /organizations/current/members` - Liste membres avec rôles
- ✅ `PUT /organizations/current/members/:user_id` - Changer rôle (RBAC)
- ✅ `DELETE /organizations/current/members/:user_id` - Retirer membre (invariant dernier owner)
- ✅ `POST /organizations/current/leave` - Quitter org

**Module RBAC** (`inventiv-api/src/rbac.rs`) :
- ✅ Enum `OrgRole` : `Owner`, `Admin`, `Manager`, `User`
- ✅ Fonctions de permission :
  - `can_invite(role)` → Owner/Admin/Manager
  - `can_set_activation_flag(role, flag)` → Owner (tech+eco), Admin (tech), Manager (eco)
  - `can_assign_role(actor, from, to)` → règles de délégation
- ✅ Tests unitaires RBAC

**Module Bootstrap** (`inventiv-api/src/bootstrap_admin.rs`) :
- ✅ Création automatique org "Inventiv IT" avec admin comme owner
- ✅ Idempotent (peut être réexécuté)

**Auth & Sessions** (`inventiv-api/src/auth.rs`, `auth_endpoints.rs`) :
- ✅ JWT contient `current_organization_id`
- ⏳ **MANQUE** : `current_organization_role` dans JWT
- ⏳ **MANQUE** : Table `user_sessions` pour multi-sessions
- ⏳ **MANQUE** : `session_id` dans JWT

#### 3. Frontend (Next.js/React)

**Composants** :
- ✅ `AccountSection.tsx` - Switch workspace (Personal vs Org)
- ✅ `OrganizationMembersDialog.tsx` - Gestion membres + rôles
- ✅ `WorkspaceBanner.tsx` - Affichage workspace courant
- ✅ `Sidebar.tsx` - Navigation avec badge workspace

**Types TypeScript** :
- ✅ `Organization`, `OrganizationMember` dans `lib/types.ts`
- ✅ `Me` type avec `current_organization_id`, `current_organization_name`, `current_organization_slug`
- ⏳ **MANQUE** : `current_organization_role` dans `Me`

---

## ❌ Ce qui Manque (Prochaines Étapes)

### 🔴 Critique (Bloque autres phases)

#### 1. Architecture de Sessions Multi-Organisation
**Problème** : `current_organization_id` est dans `users` → un seul "current" par user  
**Solution** : Implémenter `user_sessions` table (voir `docs/SESSION_ARCHITECTURE_PROPOSAL.md`)

**À faire** :
- [ ] Créer table `user_sessions` avec `session_id`, `current_organization_id`, `organization_role`
- [ ] Retirer `current_organization_id` de `users`
- [ ] Enrichir JWT avec `session_id`, `current_organization_role`
- [ ] Modifier `login()` pour créer session en DB
- [ ] Modifier `set_current_organization()` pour mettre à jour session en DB
- [ ] Modifier `require_user()` pour valider session en DB
- [ ] Ajouter endpoints `/auth/sessions` (liste/révocation)

**Impact** : Permet plusieurs sessions simultanées avec orgs différentes

---

### 🟡 Haute Priorité (Core Features)

#### 2. Scoping Instances par Organisation
**Objectif** : Isoler les instances par `organization_id`

**À faire** :
- [ ] Migration SQL : Ajouter `instances.organization_id` (nullable pour backward compat)
- [ ] Modifier `list_instances()` → filtrer par `organization_id` si workspace org
- [ ] Modifier `create_deployment()` → définir `organization_id` si workspace org
- [ ] Modifier `get_instance()`, `terminate_instance()`, `reinstall_instance()` → vérifier accès RBAC
- [ ] UI : Badge "Personal" vs "Org: <Name>" sur instances
- [ ] UI : WorkspaceBanner visible sur page instances
- [ ] Tests : Mode Personal → instances legacy accessibles
- [ ] Tests : Mode Org → seulement instances org-owned
- [ ] Tests RBAC : User ne peut pas terminer instances org

**Fichiers** :
- Migration SQL (à créer)
- `inventiv-api/src/handlers/deployments.rs`
- `inventiv-frontend/src/app/(app)/instances/page.tsx`

---

#### 3. Scoping Models par Organisation
**Objectif** : Isoler les modèles par `organization_id` + visibilité publique/privée

**À faire** :
- [ ] Migration SQL : Ajouter `models.organization_id` (nullable pour backward compat)
- [ ] Modifier `list_models()` → filtrer par `organization_id` + modèles publics
- [ ] Modifier `create_model()` → définir `organization_id` si workspace org
- [ ] Modifier `update_model()`, `delete_model()` → vérifier RBAC
- [ ] UI : Badge "Public" vs "Org: <Name>" sur modèles
- [ ] UI : Filtre workspace sur page modèles
- [ ] Tests : Mode Personal → modèles publics accessibles
- [ ] Tests : Mode Org → modèles privés org + publics
- [ ] Tests RBAC : User ne peut pas modifier modèles org

**Fichiers** :
- Migration SQL (à créer)
- `inventiv-api/src/handlers/models.rs` (si existe) ou `main.rs`
- `inventiv-frontend/src/app/(app)/models/page.tsx`

---

#### 4. Invitations d'Utilisateurs
**Objectif** : Permettre d'inviter des users par email dans une organisation

**À faire** :
- [ ] Migration SQL : Créer table `organization_invitations`
- [ ] API : `POST /organizations/current/invitations` (inviter par email)
- [ ] API : `GET /organizations/current/invitations` (liste pending/accepted/revoked)
- [ ] API : `DELETE /organizations/current/invitations/:id` (révoquer)
- [ ] API : `POST /organizations/invitations/:token/accept` (public, peut créer user si inexistant)
- [ ] UI : Section "Invitations" dans `OrganizationMembersDialog`
- [ ] UI : Formulaire inviter (email + rôle)
- [ ] UI : Liste invitations avec statut + actions
- [ ] Tests : Inviter user existant → membership créé
- [ ] Tests : Inviter user inexistant → compte créé + membership
- [ ] Tests RBAC : Owner/Admin/Manager peuvent inviter

**Fichiers** :
- Migration SQL (à créer)
- `inventiv-api/src/organizations.rs` (endpoints invitations)
- `inventiv-frontend/src/components/account/OrganizationMembersDialog.tsx`

---

### 🟢 Moyenne Priorité

#### 5. Scoping API Keys par Organisation
**Objectif** : Isoler les clés API par `organization_id`

**À faire** :
- [ ] Modifier `list_api_keys()` → filtrer par `organization_id`
- [ ] Modifier `create_api_key()` → définir `organization_id` si workspace org
- [ ] Modifier `update_api_key()`, `revoke_api_key()` → vérifier RBAC
- [ ] UI : Badge "Personal" vs "Org: <Name>" sur clés
- [ ] UI : Filtre workspace
- [ ] Tests : Mode Personal → clés user-owned
- [ ] Tests : Mode Org → clés org-owned
- [ ] Tests RBAC : User ne peut pas modifier clés org

**Fichiers** :
- `inventiv-api/src/handlers/api_keys.rs` (si existe) ou `main.rs`
- `inventiv-frontend/src/app/(app)/api-keys/page.tsx`

---

#### 6. Scoping Users par Organisation
**Objectif** : Filtrer la liste des users selon le workspace

**À faire** :
- [ ] Modifier `list_users()` → filtrer membres org si workspace org
- [ ] Modifier `create_user()` → créer membership automatique si workspace org
- [ ] Modifier `update_user()`, `delete_user()` → vérifier RBAC + invariant dernier owner
- [ ] UI : WorkspaceBanner visible
- [ ] UI : Liste filtrée membres org
- [ ] Tests : Mode Personal → voir tous users (admin)
- [ ] Tests : Mode Org → voir seulement membres
- [ ] Tests RBAC : User ne peut pas modifier membres

**Fichiers** :
- `inventiv-api/src/handlers/users.rs` (si existe) ou `main.rs`
- `inventiv-frontend/src/app/(app)/users/page.tsx`

---

#### 7. Scoping FinOps par Organisation
**Objectif** : Filtrer les dashboards financiers selon le workspace

**À faire** :
- [ ] Modifier `get_cost_current()` → filtrer par `organization_id`
- [ ] Modifier `get_costs_dashboard_*()` → filtrer par `organization_id`
- [ ] UI : WorkspaceBanner visible
- [ ] UI : Dashboards filtrés selon workspace
- [ ] Tests : Mode Personal → coûts user
- [ ] Tests : Mode Org → coûts org

**Fichiers** :
- `inventiv-api/src/handlers/finops.rs` (si existe) ou `main.rs`
- `inventiv-frontend/src/app/(app)/(dashboard)/page.tsx`

---

### 🔵 Basse Priorité (Nice-to-Have)

#### 8. Double Activation (Tech/Eco)
**Objectif** : Activation technique (Admin) + économique (Manager) par ressource

**À faire** :
- [ ] Ajouter colonnes `tech_activated_by`, `eco_activated_by` sur ressources (instances, models, etc.)
- [ ] Modifier endpoints pour vérifier double activation
- [ ] UI : État "non opérationnel" + alerte flag manquant
- [ ] Tests : Ressource non opérationnelle si un flag manque

---

#### 9. Model Sharing & Billing
**Objectif** : Partage de modèles entre orgs avec facturation au token

**À faire** :
- [ ] CRUD `organization_models` (publish/unpublish)
- [ ] CRUD `organization_model_shares` (grant/pause/revoke + pricing)
- [ ] Résolution `org_slug/model_code` dans OpenAI proxy
- [ ] Ingestion `finops.inference_usage` avec chargeback
- [ ] Dashboards consommation par org/provider/consumer

---

#### 10. Migration Frontend Modules
**Objectif** : Masquer/afficher modules selon workspace + rôle

**À faire** :
- [ ] Identifier modules "For All Users" vs "Admin Only" vs "Org Required"
- [ ] Modifier `layout.tsx` → vérifier `current_organization_id` pour modules admin
- [ ] Modifier `Sidebar.tsx` → masquer liens selon workspace + rôle org
- [ ] Badge "Org required" sur liens admin
- [ ] Redirection création org si nécessaire

---

## 📋 Plan d'Action Recommandé

### Phase Immédiate (Sprint 1)

1. **Architecture de Sessions** (Critique)
   - Créer table `user_sessions`
   - Migrer `current_organization_id` vers sessions
   - Enrichir JWT avec `session_id` + `current_organization_role`
   - Tests : Multi-sessions avec orgs différentes

2. **Migration PK/FK** (Fondation)
   - Appliquer migration `20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql`
   - Vérifier contraintes sur DB de test
   - Déployer staging

### Phase Court Terme (Sprint 2-3)

3. **Scoping Instances** (Core Feature)
   - Migration SQL + API + UI + Tests

4. **Scoping Models** (Core Feature)
   - Migration SQL + API + UI + Tests

5. **Invitations** (Onboarding)
   - Migration SQL + API + UI + Tests

### Phase Moyen Terme (Sprint 4-6)

6. **Scoping API Keys**
7. **Scoping Users**
8. **Scoping FinOps**
9. **Migration Frontend Modules**

### Phase Long Terme (Sprint 7+)

10. **Double Activation**
11. **Model Sharing & Billing**

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
- `docs/RBAC_ANALYSIS.md` - Analyse détaillée RBAC
- `docs/SESSION_ARCHITECTURE_PROPOSAL.md` - Proposition architecture sessions
- `docs/SESSION_AUTH_ANALYSIS.md` - Analyse session/auth actuelle
- `docs/MULTI_TENANT_ROADMAP.md` - Roadmap cible (users first-class + org workspaces)
- `docs/MULTI_TENANT_MODEL_SHARING_BILLING.md` - Design partage modèles + billing

---

## 🎯 Objectifs pour la Suite

1. **Isolation complète** : Instances, Models, API Keys, Users, FinOps scopés par organisation
2. **RBAC complet** : Permissions selon rôle org (Owner/Admin/Manager/User)
3. **Visibilité conditionnelle** : Modules/fonctions affichés selon workspace + rôle
4. **Multi-sessions** : Plusieurs sessions simultanées avec orgs différentes
5. **Onboarding fluide** : Invitations par email avec création de compte automatique

---

**Prochaine étape** : Valider ce document et commencer par l'architecture de sessions (Phase Immédiate).

