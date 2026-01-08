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
| `full-i18n` | v0.4.0 | `0f31aa1` | Support i18n complet |

### Divergence

- **Commits dans `main` non présents dans `full-i18n`**: ~25 commits (multi-tenancy, RBAC, CI fixes)
- **Commits dans `full-i18n` non présents dans `main`**: ~15 commits (i18n, locales, traductions)
- **Fichiers modifiés**: 26 fichiers modifiés entre les deux branches
- **Changements non commités**: 2 fichiers (`Sidebar.tsx`, migration `20251216000100`)

## 🔍 Analyse détaillée

### 1. Fonctionnalités dans `main` (non présentes dans `full-i18n`)

#### Multi-tenancy & RBAC
- **Tables**:
  - `organizations` (id, name, created_at, etc.)
  - `organization_memberships` (user_id, organization_id, role)
  - `organization_models` (organization_id, model_id)
  - `organization_model_shares` (provider/consumer orgs)
  - `user_sessions` (session management avec current_organization_id)

- **Fonctionnalités**:
  - Organization-scoped provider credentials
  - Organization-scoped settings
  - RBAC avec rôles: owner, admin, manager, user
  - Invitations d'organisation
  - Session management amélioré

- **Fichiers modifiés**:
  - `inventiv-api/src/main.rs` (ajout endpoints organizations)
  - `inventiv-api/src/auth_endpoints.rs` (session management)
  - `inventiv-api/src/users_endpoint.rs` (gestion users avec orgs)
  - Migrations dans `baseline.sql`

#### Autres améliorations dans `main`
- Fixes CI/CD (lightningcss, clippy, fmt)
- Améliorations déploiement (secrets sync, VM disk sizing)
- Documentation consolidée

### 2. Fonctionnalités dans `full-i18n` (non présentes dans `main`)

#### Internationalisation (i18n)
- **Tables**:
  - `locales` (code BCP47, name, native_name, direction)
  - `i18n_keys` (identifiants opaques pour traductions)
  - `i18n_texts` (key_id, locale_code, text_value)
  - Colonne `locale_code` dans `users`

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

- **Fichiers modifiés**:
  - `inventiv-api/src/main.rs` (endpoint `/locales`, backfill i18n, queries avec `i18n_get_text`)
  - `inventiv-api/src/auth_endpoints.rs` (ajout `locale_code` dans MeResponse/UpdateMeRequest)
  - `inventiv-api/src/users_endpoint.rs` (ajout `locale_code` dans UserResponse/Create/Update)
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

2. **`inventiv-api/src/auth_endpoints.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Session management amélioré
   - **`full-i18n`**: Ajout `locale_code` dans `MeResponse`, `MeRow`, `UpdateMeRequest`
   - **Action**: Merge manuel nécessaire, ajouter `locale_code` aux structures de `main`

3. **`inventiv-api/src/users_endpoint.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Gestion users avec organizations
   - **`full-i18n`**: Ajout `locale_code` dans toutes les structures et queries
   - **Action**: Merge manuel nécessaire, combiner organization_id et locale_code

4. **`inventiv-api/src/bootstrap_admin.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Bootstrap avec organizations
   - **`full-i18n`**: Bootstrap avec locale_code
   - **Action**: Merge manuel nécessaire

5. **`inventiv-api/src/settings.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Organization-scoped settings
   - **`full-i18n`**: Queries localisées avec `i18n_get_text`
   - **Action**: Merge manuel nécessaire, combiner organization filtering et i18n

6. **`inventiv-api/src/instance_type_zones.rs`**
   - **Conflit**: Les deux branches ont modifié ce fichier
   - **`main`**: Organization-scoped zones
   - **`full-i18n`**: Queries localisées
   - **Action**: Merge manuel nécessaire

#### 🟡 Conflits mineurs (résolution automatique possible)

1. **Migrations SQL**
   - **`main`**: Migrations multi-tenant dans `baseline.sql`
   - **`full-i18n`**: Migrations i18n séparées (20251216000000, 20251216001000, 20251216002000)
   - **Action**: Les migrations i18n peuvent être ajoutées après le baseline, vérifier l'ordre chronologique

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

### 4. Changements non commités

1. **`inventiv-frontend/src/components/Sidebar.tsx`**
   - Modifications non commitées
   - **Action**: Commiter ou stash avant merge

2. **`sqlx-migrations/20251216000100_models_catalog_enhancements.sql`**
   - Migration modifiée (renommée depuis `20251216000000`)
   - **Action**: Vérifier si les modifications sont nécessaires, commiter ou stash

## 📋 Plan d'action recommandé

### Phase 1: Préparation (AVANT merge)

1. **Sauvegarder l'état actuel**
   ```bash
   git stash push -m "WIP: Sidebar.tsx et migration avant merge v1.0.0"
   ```

2. **Mettre à jour `full-i18n` avec `main`**
   ```bash
   git checkout full-i18n
   git fetch origin
   git merge origin/main
   # Résoudre les conflits manuellement (voir section ci-dessus)
   ```

3. **Vérifier les migrations**
   - S'assurer que les migrations i18n sont compatibles avec le schema multi-tenant
   - Vérifier que `locale_code` peut être ajouté à `users` sans conflit avec `current_organization_id`

### Phase 2: Résolution des conflits

#### Priorité 1: Fichiers critiques

1. **`inventiv-api/src/main.rs`**
   - Ajouter les endpoints organizations de `main`
   - Conserver les endpoints i18n de `full-i18n`
   - Conserver la fonction `ensure_catalog_i18n_backfill`
   - Adapter les queries pour combiner organization filtering et i18n

2. **`inventiv-api/src/auth_endpoints.rs`**
   - Conserver le session management de `main`
   - Ajouter `locale_code` aux structures (MeResponse, UpdateMeRequest)
   - Adapter les queries pour inclure `locale_code`

3. **`inventiv-api/src/users_endpoint.rs`**
   - Conserver la gestion organizations de `main`
   - Ajouter `locale_code` aux structures et queries
   - S'assurer que la création/update de users gère les deux champs

#### Priorité 2: Fichiers secondaires

4. **`inventiv-api/src/bootstrap_admin.rs`**
   - Créer admin avec organization par défaut (`main`)
   - Créer admin avec locale_code='fr-FR' (`full-i18n`)
   - Combiner les deux

5. **`inventiv-api/src/settings.rs`**
   - Conserver organization-scoped filtering (`main`)
   - Ajouter i18n queries (`full-i18n`)
   - Combiner: `WHERE organization_id = $1 AND ...` + `i18n_get_text(...)`

6. **`inventiv-api/src/instance_type_zones.rs`**
   - Même approche que `settings.rs`

### Phase 3: Tests & Validation

1. **Tests unitaires**
   - Vérifier que les endpoints organizations fonctionnent
   - Vérifier que les endpoints i18n fonctionnent
   - Vérifier que les deux fonctionnent ensemble

2. **Tests d'intégration**
   - Créer un user avec organization et locale_code
   - Vérifier que les queries catalog retournent les bonnes traductions
   - Vérifier que le filtering par organization fonctionne

3. **Tests de migration**
   - Tester les migrations sur une DB vide
   - Tester les migrations sur une DB existante (baseline → i18n)
   - Vérifier le backfill i18n

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

### 2. Ordre des migrations

L'ordre chronologique des migrations doit être respecté:
- Baseline (multi-tenant) → Migrations i18n (20251216000000+) → Autres migrations

### 3. Données existantes

Si des données existent déjà en production:
- Les users existants doivent avoir un `locale_code` par défaut (`en-US`)
- Les catalog entries doivent avoir des `*_i18n_id` générés et backfillés
- Les organizations existantes doivent être compatibles avec le système i18n

### 4. Frontend

Le frontend i18n dans `full-i18n` doit être compatible avec:
- Le système de sessions multi-tenant (`current_organization_id`)
- Les endpoints organizations
- La gestion des users avec organizations

## 📝 Checklist de merge

- [ ] Stash des changements non commités
- [ ] Merge `main` dans `full-i18n`
- [ ] Résolution conflit `main.rs`
- [ ] Résolution conflit `auth_endpoints.rs`
- [ ] Résolution conflit `users_endpoint.rs`
- [ ] Résolution conflit `bootstrap_admin.rs`
- [ ] Résolution conflit `settings.rs`
- [ ] Résolution conflit `instance_type_zones.rs`
- [ ] Vérification migrations SQL
- [ ] Tests unitaires
- [ ] Tests d'intégration
- [ ] Tests de migration
- [ ] Mise à jour VERSION → 1.0.0
- [ ] Commit de merge
- [ ] Tag v1.0.0
- [ ] Documentation mise à jour
- [ ] Changelog créé

## 🎯 Recommandations finales

1. **Approche progressive**: Faire le merge dans une branche de test d'abord, tester complètement avant de merger dans `main`

2. **Tests exhaustifs**: Les deux fonctionnalités (multi-tenancy et i18n) sont critiques, tester chaque scénario

3. **Documentation**: Mettre à jour la documentation pour expliquer:
   - Comment les organizations et i18n fonctionnent ensemble
   - Comment configurer les locales par organization (si nécessaire)
   - Comment migrer les données existantes

4. **Rollback plan**: Préparer un plan de rollback au cas où des problèmes surviennent en production

5. **Communication**: Informer l'équipe des changements majeurs avant le déploiement

---

**Prochaine étape**: Exécuter la Phase 1 (Préparation) et commencer la résolution des conflits.

