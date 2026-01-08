# Analyse de Merge pour V1.0.0 - full-i18n → main

**Date**: 2025-01-XX  
**Branche source**: `full-i18n`  
**Branche cible**: `main`  
**Version cible**: V1.0.0

## 📊 Vue d'ensemble

### État des branches

| Branche | Version | Dernier commit | Statut |
|---------|---------|----------------|--------|
| `main` | v0.6.1 | `0c9e694` | Multi-tenancy complet + RBAC |
| `full-i18n` | v0.4.0 | `5945bab` | Support i18n complet |

### Divergence

- **Commits dans `main` non présents dans `full-i18n`**: ~77 commits (multi-tenancy, RBAC, CI fixes)
- **Commits dans `full-i18n` non présents dans `main`**: ~15 commits (i18n, locales, traductions)
- **Fichiers modifiés**: 29 fichiers modifiés entre les deux branches
- **Note importante**: Il y a déjà eu 2 merges de `main` dans `full-i18n` (`dea7900`, `514ea6f`), mais les fonctionnalités multi-tenant ne sont **PAS** présentes dans le code actuel de `full-i18n`

## 🔍 Analyse détaillée

### 1. Fonctionnalités dans `main` (non présentes dans `full-i18n`)

#### Multi-tenancy & RBAC
- **Tables**:
  - `organizations` (id, name, created_at, etc.)
  - `organization_memberships` (user_id, organization_id, role)
  - `organization_models` (organization_id, model_id)
  - `organization_model_shares` (provider/consumer orgs)
  - `user_sessions` (session management avec current_organization_id)
  - Colonnes `organization_id` dans plusieurs tables (api_keys, provider_settings, etc.)

- **Fonctionnalités**:
  - Organization-scoped provider credentials
  - Organization-scoped settings
  - RBAC avec rôles: owner, admin, manager, user
  - Invitations d'organisation
  - Session management amélioré avec `current_organization_id`
  - FinOps multi-tenant (provider_organization_id, consumer_organization_id)

- **Fichiers modifiés**:
  - `inventiv-api/src/main.rs` (ajout endpoints organizations)
  - `inventiv-api/src/auth_endpoints.rs` (session management avec organizations)
  - `inventiv-api/src/users_endpoint.rs` (gestion users avec orgs)
  - `inventiv-api/src/bootstrap_admin.rs` (bootstrap avec organizations)
  - `inventiv-api/src/settings.rs` (organization-scoped filtering)
  - `inventiv-api/src/instance_type_zones.rs` (organization-scoped zones)
  - Nouveau module `inventiv-api/src/organizations.rs` (probablement)
  - Migrations dans `baseline.sql` (tables organizations)

#### Autres améliorations dans `main`
- Fixes CI/CD (lightningcss, clippy, fmt)
- Améliorations déploiement (secrets sync, VM disk sizing)
- Documentation consolidée
- Session management endpoints (`/auth/sessions`, `/auth/sessions/:id`)

### 2. Fonctionnalités dans `full-i18n` (non présentes dans `main`)

#### Internationalisation (i18n)
- **Tables**:
  - `locales` (code BCP47, name, native_name, direction)
  - `i18n_keys` (identifiants opaques pour traductions)
  - `i18n_texts` (key_id, locale_code, text_value)
  - Colonne `locale_code` dans `users`
  - Colonnes `*_i18n_id` dans catalog (providers, regions, zones, instance_types, action_types)

- **Migrations**:
  - `20251216000000_add_locales_and_user_locale.sql`
  - `20251216001000_add_generic_i18n_tables.sql`
  - `20251216002000_catalog_add_i18n_ids_and_drop_name_uniques.sql`

- **Fonctionnalités**:
  - Support 3 locales: `fr-FR`, `en-US`, `ar` (RTL)
  - Fonction SQL `i18n_get_text()` avec fallback
  - Colonnes `*_i18n_id` dans catalog (providers, regions, zones, instance_types, action_types)
  - Frontend i18n avec messages JSON (fr-FR.json, en-US.json, ar.json)
  - Hook React `useI18n()` avec provider context
  - Localisation des labels catalog selon user locale
  - Sélecteur de locale dans Sidebar

- **Fichiers modifiés**:
  - `inventiv-api/src/main.rs` (endpoint `/locales`, backfill i18n, queries avec `i18n_get_text`)
  - `inventiv-api/src/auth_endpoints.rs` (ajout `locale_code` dans MeResponse/UpdateMeRequest)
  - `inventiv-api/src/users_endpoint.rs` (ajout `locale_code` dans UserResponse/Create/Update)
  - `inventiv-api/src/bootstrap_admin.rs` (bootstrap avec locale_code='fr-FR')
  - `inventiv-api/src/settings.rs` (queries localisées)
  - `inventiv-api/src/instance_type_zones.rs` (queries localisées)
  - `inventiv-api/src/locales_endpoint.rs` (nouveau module)
  - `inventiv-api/src/user_locale.rs` (nouveau module)
  - `inventiv-frontend/src/i18n/*` (nouveau système i18n)
  - `inventiv-frontend/src/app/layout.tsx` (I18nProvider)
  - `inventiv-frontend/src/components/Sidebar.tsx` (sélecteur locale)

### 3. Conflits identifiés

#### 🔴 Conflits majeurs (nécessitent résolution manuelle)

1. **`inventiv-api/src/main.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Ajout endpoints organizations, session management
   - **`full-i18n`**: Ajout endpoint `/locales`, module `locales_endpoint`, fonction `ensure_catalog_i18n_backfill`, queries avec `i18n_get_text`
   - **Action**: Merge manuel nécessaire, les deux fonctionnalités sont complémentaires
   - **Résolution**: Ajouter les endpoints organizations de `main` + conserver les endpoints i18n de `full-i18n`

2. **`inventiv-api/src/auth_endpoints.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Session management amélioré avec organizations (`current_organization_id`)
   - **`full-i18n`**: Ajout `locale_code` dans `MeResponse`, `MeRow`, `UpdateMeRequest`
   - **Action**: Merge manuel nécessaire, ajouter `locale_code` aux structures de `main` + conserver le session management

3. **`inventiv-api/src/users_endpoint.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Gestion users avec organizations (organization_id dans UserResponse)
   - **`full-i18n`**: Ajout `locale_code` dans toutes les structures et queries
   - **Action**: Merge manuel nécessaire, combiner organization_id et locale_code

4. **`inventiv-api/src/bootstrap_admin.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Bootstrap avec organizations (`ensure_default_organization`)
   - **`full-i18n`**: Bootstrap avec locale_code='fr-FR'
   - **Action**: Merge manuel nécessaire, combiner les deux (créer admin avec organization ET locale_code)

5. **`inventiv-api/src/settings.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Organization-scoped settings (WHERE organization_id = $1)
   - **`full-i18n`**: Queries localisées avec `i18n_get_text`
   - **Action**: Merge manuel nécessaire, combiner organization filtering et i18n
   - **Résolution**: `WHERE organization_id = $1 AND ...` + `i18n_get_text(..., $2)` avec locale_code

6. **`inventiv-api/src/instance_type_zones.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Organization-scoped zones
   - **`full-i18n`**: Queries localisées
   - **Action**: Merge manuel nécessaire, combiner organization filtering et i18n

#### 🟡 Conflits mineurs (résolution automatique possible)

1. **Migrations SQL**
   - **`main`**: Migrations multi-tenant dans `baseline.sql` (tables organizations)
   - **`full-i18n`**: Migrations i18n séparées (20251216000000, 20251216001000, 20251216002000)
   - **Action**: Les migrations i18n peuvent être ajoutées après le baseline, vérifier l'ordre chronologique
   - **Note**: Les migrations i18n doivent être compatibles avec le schéma multi-tenant

2. **`VERSION`**
   - **`main`**: v0.6.1
   - **`full-i18n`**: v0.4.0
   - **Action**: Mettre à jour vers v1.0.0 après merge

#### 🟢 Pas de conflit (ajouts complémentaires)

1. **Frontend i18n** (`inventiv-frontend/src/i18n/*`)
   - Nouveau code dans `full-i18n`, pas de conflit avec `main`
   - Peut être ajouté tel quel

2. **Modules API i18n** (`locales_endpoint.rs`, `user_locale.rs`)
   - Nouveaux modules dans `full-i18n`
   - Pas de conflit avec `main`

3. **Module organizations** (`organizations.rs` dans `main`)
   - Nouveau module dans `main`
   - Pas de conflit avec `full-i18n`

## 📋 Plan d'action recommandé

### Phase 1: Préparation (AVANT merge)

1. **Sauvegarder l'état actuel**
   ```bash
   git checkout full-i18n
   git status  # Vérifier qu'il n'y a pas de changements non commités
   ```

2. **Créer une branche de test pour le merge**
   ```bash
   git checkout -b merge-v1.0.0-test
   git merge origin/main
   # Résoudre les conflits manuellement (voir section ci-dessous)
   ```

3. **Vérifier les migrations**
   - S'assurer que les migrations i18n sont compatibles avec le schema multi-tenant
   - Vérifier que `locale_code` peut être ajouté à `users` sans conflit avec `current_organization_id`
   - Vérifier l'ordre chronologique des migrations

### Phase 2: Résolution des conflits

#### Priorité 1: Fichiers critiques

1. **`inventiv-api/src/main.rs`**
   - Ajouter les endpoints organizations de `main` (après les endpoints i18n)
   - Conserver les endpoints i18n de `full-i18n` (`/locales`)
   - Conserver la fonction `ensure_catalog_i18n_backfill`
   - Adapter les queries pour combiner organization filtering et i18n
   - Vérifier que le module `organizations` est importé

2. **`inventiv-api/src/auth_endpoints.rs`**
   - Conserver le session management de `main` (avec `current_organization_id`)
   - Ajouter `locale_code` aux structures (MeResponse, UpdateMeRequest, MeRow)
   - Adapter les queries pour inclure `locale_code`
   - S'assurer que les deux fonctionnent ensemble

3. **`inventiv-api/src/users_endpoint.rs`**
   - Conserver la gestion organizations de `main` (`organization_id` dans UserResponse)
   - Ajouter `locale_code` aux structures et queries
   - S'assurer que la création/update de users gère les deux champs
   - Adapter les queries pour inclure `organization_id` ET `locale_code`

#### Priorité 2: Fichiers secondaires

4. **`inventiv-api/src/bootstrap_admin.rs`**
   - Créer admin avec organization par défaut (`main`: `ensure_default_organization`)
   - Créer admin avec locale_code='fr-FR' (`full-i18n`)
   - Combiner les deux: créer organization d'abord, puis admin avec organization_id ET locale_code

5. **`inventiv-api/src/settings.rs`**
   - Conserver organization-scoped filtering (`main`: `WHERE organization_id = $1`)
   - Ajouter i18n queries (`full-i18n`: `i18n_get_text(...)`)
   - Combiner: `WHERE organization_id = $1 AND ...` + `i18n_get_text(..., $2)` avec locale_code
   - Adapter toutes les fonctions (list_providers, list_regions, list_zones, list_instance_types)

6. **`inventiv-api/src/instance_type_zones.rs`**
   - Même approche que `settings.rs`
   - Combiner organization filtering et i18n dans toutes les queries

### Phase 3: Tests & Validation

1. **Tests unitaires**
   - Vérifier que les endpoints organizations fonctionnent
   - Vérifier que les endpoints i18n fonctionnent
   - Vérifier que les deux fonctionnent ensemble

2. **Tests d'intégration**
   - Créer un user avec organization et locale_code
   - Vérifier que les queries catalog retournent les bonnes traductions
   - Vérifier que le filtering par organization fonctionne
   - Vérifier que les deux fonctionnent ensemble

3. **Tests de migration**
   - Tester les migrations sur une DB vide
   - Tester les migrations sur une DB existante (baseline → i18n)
   - Vérifier le backfill i18n
   - Vérifier la compatibilité avec les données multi-tenant existantes

4. **Tests frontend**
   - Vérifier que le sélecteur de locale fonctionne
   - Vérifier que les traductions s'affichent correctement
   - Vérifier que les organizations fonctionnent dans l'UI
   - Vérifier que les deux fonctionnent ensemble

### Phase 4: Finalisation

1. **Mise à jour de la version**
   ```bash
   echo "1.0.0" > VERSION
   ```

2. **Commit de merge**
   ```bash
   git commit -m "chore(release): merge full-i18n into main for v1.0.0

   - Merge i18n support (fr-FR, en-US, ar) with multi-tenancy
   - Combine organization-scoped resources with localized catalog
   - Add locale_code to users alongside organization_id
   - Resolve conflicts in main.rs, auth_endpoints.rs, users_endpoint.rs
   - Update migrations to support both features"
   ```

3. **Tag de release**
   ```bash
   git tag -a v1.0.0 -m "Release v1.0.0: Multi-tenancy + i18n"
   ```

## ⚠️ Points d'attention

### 1. Compatibilité des migrations

Les migrations i18n dans `full-i18n` supposent que la table `users` existe avec certaines colonnes. Vérifier que:
- La migration `20251216000000_add_locales_and_user_locale.sql` est compatible avec le schema multi-tenant
- Le backfill `ensure_catalog_i18n_backfill` fonctionne avec les colonnes `*_i18n_id` même si certaines sont NULL
- Les migrations i18n peuvent être appliquées après les migrations multi-tenant

### 2. Ordre des migrations

L'ordre chronologique des migrations doit être respecté:
- Baseline (multi-tenant) → Migrations i18n (20251216000000+) → Autres migrations

### 3. Données existantes

Si des données existent déjà en production:
- Les users existants doivent avoir un `locale_code` par défaut (`en-US`)
- Les users existants doivent avoir un `organization_id` (via `ensure_default_organization`)
- Les catalog entries doivent avoir des `*_i18n_id` générés et backfillés
- Les organizations existantes doivent être compatibles avec le système i18n

### 4. Frontend

Le frontend i18n dans `full-i18n` doit être compatible avec:
- Le système de sessions multi-tenant (`current_organization_id`)
- Les endpoints organizations
- La gestion des users avec organizations
- Le sélecteur de locale doit fonctionner avec le système multi-tenant

### 5. Queries SQL combinées

Toutes les queries qui filtrent par organization doivent aussi utiliser `i18n_get_text`:
- `list_providers`: `WHERE organization_id = $1` + `i18n_get_text(name_i18n_id, $2)`
- `list_regions`: `WHERE organization_id = $1` + `i18n_get_text(name_i18n_id, $2)`
- `list_zones`: `WHERE organization_id = $1` + `i18n_get_text(name_i18n_id, $2)`
- `list_instance_types`: `WHERE organization_id = $1` + `i18n_get_text(name_i18n_id, $2)`

## 📝 Checklist de merge

- [ ] Vérifier qu'il n'y a pas de changements non commités
- [ ] Créer une branche de test pour le merge
- [ ] Merge `main` dans la branche de test
- [ ] Résolution conflit `main.rs`
- [ ] Résolution conflit `auth_endpoints.rs`
- [ ] Résolution conflit `users_endpoint.rs`
- [ ] Résolution conflit `bootstrap_admin.rs`
- [ ] Résolution conflit `settings.rs`
- [ ] Résolution conflit `instance_type_zones.rs`
- [ ] Vérification migrations SQL (ordre chronologique)
- [ ] Tests unitaires (organizations + i18n)
- [ ] Tests d'intégration (organizations + i18n ensemble)
- [ ] Tests de migration (DB vide + DB existante)
- [ ] Tests frontend (locale selector + organizations)
- [ ] Mise à jour VERSION → 1.0.0
- [ ] Commit de merge
- [ ] Tag v1.0.0
- [ ] Documentation mise à jour
- [ ] Changelog créé

## 🎯 Recommandations finales

1. **Approche progressive**: Faire le merge dans une branche de test d'abord, tester complètement avant de merger dans `main`

2. **Tests exhaustifs**: Les deux fonctionnalités (multi-tenancy et i18n) sont critiques, tester chaque scénario:
   - User avec organization + locale_code
   - Queries catalog avec organization filtering + i18n
   - Frontend avec sélecteur locale + gestion organizations

3. **Documentation**: Mettre à jour la documentation pour expliquer:
   - Comment les organizations et i18n fonctionnent ensemble
   - Comment configurer les locales par organization (si nécessaire)
   - Comment migrer les données existantes

4. **Rollback plan**: Préparer un plan de rollback au cas où des problèmes surviennent en production

5. **Communication**: Informer l'équipe des changements majeurs avant le déploiement

---

**Prochaine étape**: Exécuter la Phase 1 (Préparation) et commencer la résolution des conflits dans une branche de test.

