# Multi-Tenant Migration Tracker

## Vue d'ensemble des phases

| Phase | Nom | Statut | Priorité | Dépendances | Impact |
|-------|-----|--------|----------|-------------|--------|
| 0 | Préparation DB (PK/FK) | ⏳ À faire | 🔴 Critique | - | Bloque tout |
| 1 | CRUD Organisation | ✅ Fait | - | - | Fondation |
| 2 | CRUD Memberships + RBAC | ✅ Fait | - | Phase 1 | Fondation |
| 3 | Invitations | ⏳ À faire | 🟡 Haute | Phase 2 | Onboarding |
| 4 | Scoping API Keys | ⏳ À faire | 🟢 Moyenne | Phase 2 | Impact faible |
| 5 | Scoping Instances | ⏳ À faire | 🟡 Haute | Phase 2 | Core feature |
| 6 | Scoping Models | ⏳ À faire | 🟡 Haute | Phase 2 | Core feature |
| 7 | Scoping Users | ⏳ À faire | 🟢 Moyenne | Phase 2 | Impact moyen |
| 8 | Scoping Settings | ⏳ Optionnel | 🔵 Basse | Phase 2 | Optionnel |
| 9 | Scoping FinOps | ⏳ À faire | 🟢 Moyenne | Phase 2 | Dashboards |
| 10 | Migration Frontend Modules | ⏳ À faire | 🟡 Haute | Phases 4-9 | UX |
| 11 | Double Activation | ⏳ À faire | 🔵 Basse | Phase 10 | Feature avancée |
| 12 | Model Sharing & Billing | ⏳ À faire | 🔵 Basse | Phase 6 | Feature avancée |

**Légende** :
- ✅ Fait
- ⏳ À faire
- 🔴 Critique (bloque autres phases)
- 🟡 Haute (core feature)
- 🟢 Moyenne (important mais non bloquant)
- 🔵 Basse (nice-to-have)

---

## Détail par phase

### Phase 0 : Préparation DB (PK/FK)
**Fichiers** : `sqlx-migrations/20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql`

**Checklist** :
- [ ] Migration créée
- [ ] Test local (DB de test)
- [ ] Vérifier contraintes PRIMARY KEY
- [ ] Vérifier contraintes FOREIGN KEY
- [ ] Vérifier index
- [ ] Commit + push
- [ ] Déploiement staging
- [ ] Déploiement prod

**Blocage** : Aucun (prérequis)

---

### Phase 3 : Invitations
**Fichiers** :
- Migration SQL (à créer)
- `inventiv-api/src/organizations.rs` (endpoints invitations)
- `inventiv-frontend/src/components/account/OrganizationMembersDialog.tsx` (UI)

**Checklist** :
- [ ] Migration SQL créée
- [ ] API endpoint `POST /organizations/current/invitations`
- [ ] API endpoint `GET /organizations/current/invitations`
- [ ] API endpoint `DELETE /organizations/current/invitations/:id`
- [ ] API endpoint `POST /organizations/invitations/:token/accept` (public)
- [ ] Tests unitaires API
- [ ] UI formulaire invitation
- [ ] UI liste invitations
- [ ] Tests manuels (inviter user existant)
- [ ] Tests manuels (inviter user inexistant → création compte)
- [ ] Tests RBAC (Owner/Admin/Manager peuvent inviter)
- [ ] Commit + push
- [ ] Déploiement staging
- [ ] Déploiement prod

**Blocage** : Phase 0 (PK/FK)

---

### Phase 4 : Scoping API Keys
**Fichiers** :
- `inventiv-api/src/api_keys.rs` (modifier handlers)
- `inventiv-frontend/src/app/(app)/api-keys/page.tsx` (modifier UI)

**Checklist** :
- [ ] Modifier `list_api_keys()` → filtrer par `organization_id`
- [ ] Modifier `create_api_key()` → définir `organization_id` si workspace org
- [ ] Modifier `update_api_key()` → vérifier RBAC
- [ ] Modifier `revoke_api_key()` → vérifier RBAC
- [ ] Tests unitaires API
- [ ] UI badge "Personal" vs "Org: <Name>"
- [ ] UI filtre workspace
- [ ] Tests manuels (mode Personal → clé user-owned)
- [ ] Tests manuels (mode Org → clé org-owned)
- [ ] Tests RBAC (User ne peut pas modifier clés org)
- [ ] Commit + push
- [ ] Déploiement staging
- [ ] Déploiement prod

**Blocage** : Phase 0 (PK/FK)

---

### Phase 5 : Scoping Instances
**Fichiers** :
- Migration SQL (ajouter `instances.organization_id`)
- `inventiv-api/src/main.rs` (modifier handlers instances)
- `inventiv-frontend/src/app/(app)/instances/page.tsx` (modifier UI)

**Checklist** :
- [ ] Migration SQL créée
- [ ] Modifier `list_instances()` → filtrer par `organization_id`
- [ ] Modifier `create_deployment()` → définir `organization_id`
- [ ] Modifier `get_instance()` → vérifier accès
- [ ] Modifier `terminate_instance()` → vérifier RBAC
- [ ] Modifier `reinstall_instance()` → vérifier RBAC
- [ ] Tests unitaires API
- [ ] UI badge "Personal" vs "Org: <Name>"
- [ ] UI filtre workspace
- [ ] WorkspaceBanner visible
- [ ] Tests manuels (mode Personal → instance legacy)
- [ ] Tests manuels (mode Org → instance org-owned)
- [ ] Tests RBAC (User ne peut pas terminer instances org)
- [ ] Commit + push
- [ ] Déploiement staging
- [ ] Déploiement prod

**Blocage** : Phase 0 (PK/FK)

---

### Phase 6 : Scoping Models
**Fichiers** :
- Migration SQL (ajouter `models.organization_id`)
- `inventiv-api/src/main.rs` (modifier handlers models)
- `inventiv-frontend/src/app/(app)/models/page.tsx` (modifier UI)

**Checklist** :
- [ ] Migration SQL créée
- [ ] Modifier `list_models()` → filtrer par `organization_id` + publics
- [ ] Modifier `create_model()` → définir `organization_id`
- [ ] Modifier `update_model()` → vérifier RBAC
- [ ] Modifier `delete_model()` → vérifier RBAC
- [ ] Tests unitaires API
- [ ] UI badge "Public" vs "Org: <Name>"
- [ ] UI filtre workspace
- [ ] Tests manuels (mode Personal → modèle public)
- [ ] Tests manuels (mode Org → modèle privé org)
- [ ] Tests RBAC (User ne peut pas modifier modèles org)
- [ ] Commit + push
- [ ] Déploiement staging
- [ ] Déploiement prod

**Blocage** : Phase 0 (PK/FK)

---

### Phase 7 : Scoping Users
**Fichiers** :
- `inventiv-api/src/users_endpoint.rs` (modifier handlers)
- `inventiv-frontend/src/app/(app)/users/page.tsx` (modifier UI)

**Checklist** :
- [ ] Modifier `list_users()` → filtrer membres org si workspace org
- [ ] Modifier `create_user()` → créer membership automatique si workspace org
- [ ] Modifier `update_user()` → vérifier RBAC
- [ ] Modifier `delete_user()` → vérifier RBAC + invariant dernier owner
- [ ] Tests unitaires API
- [ ] UI WorkspaceBanner visible
- [ ] UI liste filtrée membres org
- [ ] Tests manuels (mode Personal → voir tous users)
- [ ] Tests manuels (mode Org → voir seulement membres)
- [ ] Tests RBAC (User ne peut pas modifier membres)
- [ ] Commit + push
- [ ] Déploiement staging
- [ ] Déploiement prod

**Blocage** : Phase 0 (PK/FK)

---

### Phase 9 : Scoping FinOps
**Fichiers** :
- `inventiv-api/src/finops.rs` (modifier handlers)
- `inventiv-frontend/src/app/(app)/(dashboard)/page.tsx` (modifier UI)

**Checklist** :
- [ ] Modifier `get_cost_current()` → filtrer par `organization_id`
- [ ] Modifier `get_costs_dashboard_*()` → filtrer par `organization_id`
- [ ] Tests unitaires API
- [ ] UI WorkspaceBanner visible
- [ ] UI dashboards filtrés selon workspace
- [ ] Tests manuels (mode Personal → coûts user)
- [ ] Tests manuels (mode Org → coûts org)
- [ ] Commit + push
- [ ] Déploiement staging
- [ ] Déploiement prod

**Blocage** : Phase 0 (PK/FK)

---

### Phase 10 : Migration Frontend Modules
**Fichiers** :
- `inventiv-frontend/src/app/(app)/layout.tsx`
- `inventiv-frontend/src/components/Sidebar.tsx`

**Checklist** :
- [ ] Identifier modules "For All Users" vs "Admin Only"
- [ ] Modifier `layout.tsx` → vérifier `current_organization_id` pour modules admin
- [ ] Modifier `Sidebar.tsx` → masquer/cacher liens selon workspace
- [ ] Badge "Org required" sur liens admin
- [ ] Redirection création org si nécessaire
- [ ] Tests manuels (mode Personal → voir seulement modules user)
- [ ] Tests manuels (mode Org → voir tous modules)
- [ ] Commit + push
- [ ] Déploiement staging
- [ ] Déploiement prod

**Blocage** : Phases 4-9 (pour avoir du contenu à scoper)

---

## Ordre de déploiement recommandé

1. **Phase 0** (Critique) → Bloque tout
2. **Phase 3** (Invitations) → Nécessaire pour onboarding
3. **Phase 4** (API Keys) → Impact faible, facile à tester
4. **Phase 5** (Instances) → Core feature
5. **Phase 6** (Models) → Core feature
6. **Phase 7** (Users) → Impact moyen
7. **Phase 9** (FinOps) → Dashboards
8. **Phase 10** (Frontend Modules) → Réorganisation UI
9. **Phase 8** (Settings) → Optionnel
10. **Phase 11** (Double Activation) → Feature avancée
11. **Phase 12** (Model Sharing) → Feature avancée

---

## Notes de déploiement

### Compatibilité Backward
- ✅ Mode Personal (`current_organization_id = NULL`) fonctionne toujours
- ✅ Ressources legacy (`organization_id = NULL`) restent accessibles
- ✅ Nouvelles features sont opt-in (workspace org = optionnel)

### Tests par phase
- Tests unitaires API (Rust)
- Tests manuels Frontend (mode Personal + mode Org)
- Tests RBAC (rôles Owner/Admin/Manager/User)
- Tests compatibilité backward (mode Personal)

### Rollback
- Chaque migration SQL doit être réversible
- Endpoints doivent fonctionner sans nouvelles colonnes (migration partielle)

