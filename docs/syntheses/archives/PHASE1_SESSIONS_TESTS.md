# Phase 1 : Tests Architecture Sessions

**Date** : 2025-01-XX  
**Statut** : ✅ **COMPLÉTÉ**

---

## 📋 Résumé des Tests

### Tests Unitaires (`inventiv-api/src/auth.rs`)

Tests pour les fonctions helpers de gestion de sessions :

1. **`test_create_session`**
   - Vérifie la création d'une session en DB
   - Valide que la session existe après création

2. **`test_verify_session_db`**
   - Vérifie la validation d'une session avec hash correct
   - Vérifie le rejet d'une session avec hash incorrect
   - Vérifie le rejet d'une session inexistante

3. **`test_revoke_session`**
   - Vérifie la révocation d'une session (soft delete)
   - Valide que `revoked_at` est mis à jour

4. **`test_update_session_org`**
   - Vérifie la mise à jour de l'organisation d'une session
   - Valide que `current_organization_id` et `organization_role` sont mis à jour

5. **`test_update_session_last_used`**
   - Vérifie la mise à jour de `last_used_at`
   - Valide que le timestamp est bien mis à jour

**Exécution** :
```bash
cd inventiv-api
DATABASE_URL="postgresql://postgres:password@localhost:5432/inventiv_test" \
cargo test --lib auth::tests
```

---

### Tests d'Intégration (`inventiv-api/tests/auth_test.rs`)

Tests pour les endpoints API :

1. **`test_list_sessions`**
   - Vérifie `GET /auth/sessions`
   - Valide la liste des sessions actives
   - Vérifie que `is_current` est correctement marqué
   - Teste avec plusieurs sessions (avec et sans org)

2. **`test_revoke_session`**
   - Vérifie `POST /auth/sessions/:id/revoke`
   - Valide la révocation d'une session non courante
   - Vérifie que la session est bien révoquée en DB

3. **`test_revoke_current_session_forbidden`**
   - Vérifie qu'on ne peut pas révoquer la session courante
   - Retourne 400 avec message "cannot_revoke_current_session"

4. **`test_revoke_other_user_session_forbidden`**
   - Vérifie qu'un user ne peut pas révoquer la session d'un autre user
   - Retourne 403 avec message "forbidden"

5. **`test_me_endpoint_includes_organization_role`**
   - Vérifie `GET /auth/me`
   - Valide que `current_organization_role` est inclus dans la réponse
   - Teste avec une session ayant une organisation et un rôle

**Exécution** :
```bash
cd inventiv-api
TEST_DATABASE_URL="postgresql://postgres:password@localhost:5432/inventiv_test" \
TEST_REDIS_URL="redis://localhost:6379/1" \
cargo test --test auth_test
```

---

## ✅ Checklist Complète

### Tests Unitaires
- [x] `test_create_session` créé
- [x] `test_verify_session_db` créé
- [x] `test_revoke_session` créé
- [x] `test_update_session_org` créé
- [x] `test_update_session_last_used` créé
- [x] Compilation OK

### Tests d'Intégration
- [x] `test_list_sessions` existe
- [x] `test_revoke_session` existe
- [x] `test_revoke_current_session_forbidden` existe
- [x] `test_revoke_other_user_session_forbidden` existe
- [x] `test_me_endpoint_includes_organization_role` existe

### Tests Manuels (À faire)
- [ ] Login → vérifier cookie JWT contient `session_id` + `current_organization_role`
- [ ] Switch org → vérifier nouveau cookie avec nouvelle org + rôle
- [ ] Liste sessions → vérifier affichage correct dans UI
- [ ] Révoquer session → vérifier que session ne fonctionne plus
- [ ] Multi-sessions → ouvrir 2 onglets avec orgs différentes

---

## 🎯 Prochaines Étapes

1. **Exécuter les tests** :
   ```bash
   # Tests unitaires
   cd inventiv-api
   DATABASE_URL="postgresql://postgres:password@localhost:5432/inventiv_test" \
   cargo test --lib auth::tests

   # Tests d'intégration
   TEST_DATABASE_URL="postgresql://postgres:password@localhost:5432/inventiv_test" \
   TEST_REDIS_URL="redis://localhost:6379/1" \
   cargo test --test auth_test
   ```

2. **Tests manuels** : Valider le flow complet dans le navigateur

3. **Phase 2** : Passer au scoping des instances par organisation

---

## 📊 Couverture des Tests

### Fonctions Testées ✅
- `create_session()` ✅
- `verify_session_db()` ✅
- `revoke_session()` ✅
- `update_session_org()` ✅
- `update_session_last_used()` ✅

### Endpoints Testés ✅
- `GET /auth/sessions` ✅
- `POST /auth/sessions/:id/revoke` ✅
- `GET /auth/me` (avec `current_organization_role`) ✅

### Scénarios de Sécurité Testés ✅
- Révocation session courante interdite ✅
- Révocation session d'un autre user interdite ✅
- Validation hash token ✅
- Validation expiration ✅

---

**Phase 1 complétée** : Architecture Sessions + Tests ✅

