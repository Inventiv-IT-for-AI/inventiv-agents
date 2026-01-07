# Phase 1 : Analyse de Couverture des Tests - Architecture Sessions

**Date** : 2025-01-XX  
**Objectif** : Identifier les gaps dans les tests et proposer des tests supplémentaires

---

## 📊 Tests Existants

### Tests Unitaires (`inventiv-api/src/auth.rs`)

✅ **5 tests créés** :
1. `test_create_session` - Création session en DB
2. `test_verify_session_db` - Validation session (hash correct/incorrect/inexistant)
3. `test_revoke_session` - Révocation session (soft delete)
4. `test_update_session_org` - Mise à jour org + rôle
5. `test_update_session_last_used` - Mise à jour timestamp

**Couverture** : Fonctions helpers de base ✅

---

### Tests d'Intégration (`inventiv-api/tests/auth_test.rs`)

✅ **7 tests existants** :
1. `test_login_success` - Login réussi
2. `test_login_invalid_credentials` - Login avec mauvais credentials
3. `test_me_endpoint` - Endpoint `/auth/me` basique
4. `test_logout` - Logout réussi
5. `test_list_sessions` - Liste sessions avec multi-sessions
6. `test_revoke_session` - Révocation session non courante
7. `test_revoke_current_session_forbidden` - Interdiction révoquer session courante
8. `test_revoke_other_user_session_forbidden` - Interdiction révoquer session autre user
9. `test_me_endpoint_includes_organization_role` - `MeResponse` avec rôle org

**Couverture** : Endpoints principaux ✅

---

## ❌ Tests Manquants (Gaps Identifiés)

### 1. Tests Unitaires - Fonctions Helpers Manquantes

#### `update_session_token_hash()`
- ❌ Pas de test unitaire
- **Scénario** : Rotation de token (mise à jour hash)

#### `get_user_last_org()`
- ❌ Pas de test unitaire
- **Scénario** : Récupération dernière org utilisée par un user

#### `extract_ip_address()` et `extract_user_agent()`
- ❌ Pas de tests unitaires
- **Scénarios** :
  - Extraction IP depuis headers (X-Forwarded-For, X-Real-IP, Remote-Addr)
  - Extraction User-Agent depuis headers

#### `sign_session_jwt()` et `decode_session_jwt()`
- ❌ Pas de tests unitaires
- **Scénarios** :
  - Signature JWT avec tous les champs
  - Décodage JWT valide
  - Décodage JWT expiré
  - Décodage JWT avec mauvais secret
  - Décodage JWT avec mauvais issuer

#### `hash_session_token()`
- ❌ Pas de test unitaire
- **Scénario** : Hash SHA256 d'un token

---

### 2. Tests d'Intégration - Scénarios Manquants

#### Login avec Organisation
- ❌ Pas de test pour login avec org par défaut
- **Scénario** : User avec dernière org utilisée → login crée session avec cette org

#### Login sans Organisation
- ❌ Pas de test explicite pour login sans org
- **Scénario** : User sans org → login crée session sans org

#### Switch Organisation (`PUT /organizations/current`)
- ❌ Pas de test d'intégration
- **Scénarios** :
  - Switch vers org (user membre)
  - Switch vers org (user non membre) → 403
  - Switch vers Personal (org_id = null)
  - Vérifier que session DB est mise à jour
  - Vérifier que nouveau JWT est retourné

#### Session Expirée
- ❌ Pas de test pour session expirée
- **Scénario** : Requête avec session expirée → 401

#### Session Révoquée
- ❌ Pas de test pour session révoquée (après logout)
- **Scénario** : Requête avec session révoquée → 401

#### Session avec Hash Incorrect
- ❌ Pas de test pour session avec hash token incorrect
- **Scénario** : Token JWT valide mais hash en DB différent → 401

#### Multi-Sessions avec Orgs Différentes
- ❌ Pas de test explicite
- **Scénario** : User avec 2 sessions actives avec orgs différentes → liste correcte

#### `GET /auth/sessions` - Filtres
- ❌ Pas de test pour sessions expirées (ne doivent pas apparaître)
- ❌ Pas de test pour sessions révoquées (ne doivent pas apparaître)

#### `POST /auth/sessions/:id/revoke` - Cas Limites
- ❌ Pas de test pour session inexistante → 404
- ❌ Pas de test pour session déjà révoquée → 404 ou 400

#### `GET /auth/me` - Cas Limites
- ❌ Pas de test pour session invalide → 401
- ❌ Pas de test pour user supprimé → 401

---

### 3. Tests E2E - Scénarios Manquants

#### Flow Complet Login → Switch Org → Logout
- ❌ Pas de test E2E
- **Scénario** :
  1. Login → vérifier session créée
  2. Switch org → vérifier session mise à jour + nouveau JWT
  3. Logout → vérifier session révoquée

#### Flow Multi-Sessions
- ❌ Pas de test E2E
- **Scénario** :
  1. Login session 1 avec org A
  2. Login session 2 avec org B (même user)
  3. Liste sessions → vérifier 2 sessions actives
  4. Révoquer session 1 → vérifier seule session 2 active

#### Sécurité - Token Rotation
- ❌ Pas de test E2E
- **Scénario** : Vérifier que rotation de token invalide ancien token

---

## 🎯 Tests à Ajouter (Priorisés)

### Priorité Haute (Critique)

1. **`test_switch_organization`** (Intégration)
   - Switch vers org valide
   - Switch vers org invalide (non membre)
   - Switch vers Personal
   - Vérifier mise à jour session DB
   - Vérifier nouveau JWT

2. **`test_session_expired`** (Intégration)
   - Créer session expirée
   - Requête avec session expirée → 401

3. **`test_session_revoked_after_logout`** (Intégration)
   - Login → Logout → Requête avec ancien token → 401

4. **`test_sign_and_decode_jwt`** (Unitaire)
   - Signature JWT complète
   - Décodage JWT valide
   - Décodage JWT expiré
   - Décodage JWT avec mauvais secret

### Priorité Moyenne

5. **`test_get_user_last_org`** (Unitaire)
   - User avec dernière org utilisée
   - User sans org utilisée

6. **`test_extract_ip_and_user_agent`** (Unitaire)
   - Extraction IP depuis différents headers
   - Extraction User-Agent

7. **`test_list_sessions_filters`** (Intégration)
   - Sessions expirées ne doivent pas apparaître
   - Sessions révoquées ne doivent pas apparaître

8. **`test_revoke_session_edge_cases`** (Intégration)
   - Session inexistante → 404
   - Session déjà révoquée → 404 ou 400

### Priorité Basse (Nice-to-Have)

9. **`test_login_with_default_org`** (Intégration)
   - Login avec dernière org utilisée

10. **`test_multi_sessions_different_orgs`** (Intégration)
    - 2 sessions avec orgs différentes

11. **`test_token_rotation`** (E2E)
    - Rotation de token invalide ancien token

---

## 📋 Plan d'Implémentation

### Étape 1 : Tests Unitaires Manquants (1-2h)
- `test_sign_and_decode_jwt`
- `test_get_user_last_org`
- `test_extract_ip_and_user_agent`
- `test_hash_session_token`
- `test_update_session_token_hash`

### Étape 2 : Tests d'Intégration Critiques (2-3h)
- `test_switch_organization`
- `test_session_expired`
- `test_session_revoked_after_logout`
- `test_list_sessions_filters`
- `test_revoke_session_edge_cases`

### Étape 3 : Tests E2E (1-2h)
- Flow complet login → switch org → logout
- Flow multi-sessions

---

## 🔍 Métriques de Couverture

### Couverture Actuelle (Estimation)

**Tests Unitaires** :
- Fonctions helpers : ~60% (5/8 fonctions testées)
- Scénarios de sécurité : ~40%

**Tests d'Intégration** :
- Endpoints principaux : ~70% (7/10 scénarios critiques)
- Cas limites : ~30%
- Scénarios de sécurité : ~50%

**Tests E2E** :
- Flow complet : ~0% (pas de tests E2E spécifiques sessions)

### Couverture Cible

**Tests Unitaires** : 90%+ (toutes les fonctions helpers)
**Tests d'Intégration** : 85%+ (tous les scénarios critiques + cas limites)
**Tests E2E** : 70%+ (flows principaux)

---

## ✅ Checklist Complète

### Tests Unitaires
- [x] `test_create_session`
- [x] `test_verify_session_db`
- [x] `test_revoke_session`
- [x] `test_update_session_org`
- [x] `test_update_session_last_used`
- [ ] `test_sign_and_decode_jwt`
- [ ] `test_get_user_last_org`
- [ ] `test_extract_ip_and_user_agent`
- [ ] `test_hash_session_token`
- [ ] `test_update_session_token_hash`

### Tests d'Intégration
- [x] `test_login_success`
- [x] `test_login_invalid_credentials`
- [x] `test_me_endpoint`
- [x] `test_logout`
- [x] `test_list_sessions`
- [x] `test_revoke_session`
- [x] `test_revoke_current_session_forbidden`
- [x] `test_revoke_other_user_session_forbidden`
- [x] `test_me_endpoint_includes_organization_role`
- [ ] `test_switch_organization`
- [ ] `test_session_expired`
- [ ] `test_session_revoked_after_logout`
- [ ] `test_list_sessions_filters`
- [ ] `test_revoke_session_edge_cases`
- [ ] `test_login_with_default_org`
- [ ] `test_multi_sessions_different_orgs`

### Tests E2E
- [ ] `test_e2e_login_switch_logout_flow`
- [ ] `test_e2e_multi_sessions_flow`
- [ ] `test_e2e_token_rotation`

---

**Prochaine étape** : Implémenter les tests prioritaires (Haute priorité).

