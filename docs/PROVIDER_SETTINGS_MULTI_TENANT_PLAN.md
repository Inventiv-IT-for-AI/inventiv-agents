# Plan d'implémentation : Provider Settings Multi-Tenant par Organisation

## 📋 Analyse de l'état actuel

### Schéma actuel
- **`provider_settings`** : Table globale par provider (pas de `organization_id`)
- **`instances`** : A déjà `organization_id` (migration `20260108000003`)
- **`organizations`** : 1 organisation par défaut ("Inventiv IT")

### Problème identifié
1. Les credentials Scaleway sont stockés globalement dans `provider_settings` (sans `organization_id`)
2. `ProviderManager::get_provider()` lit les credentials sans filtre par organisation
3. Lors du resize de volume, les credentials ne sont pas trouvés car ils ne sont pas liés à l'organisation
4. `create_deployment()` ne définit pas `organization_id` sur l'instance créée

### Données actuelles
- **0 rows** dans `provider_settings` (pas encore de credentials seedés)
- **1 organisation** par défaut dans la DB
- **Pas de contrainte unique** sur `provider_settings` actuellement

## 🎯 Solution : Option A - Ajouter `organization_id` à `provider_settings`

### Principe
- Chaque organisation a ses propres credentials et settings pour chaque provider
- Le catalogue (providers, regions, zones, instance_types) reste global
- Pas de fallback ni backward compatibility (DB réinitialisée régulièrement)

## 📝 Plan d'implémentation

### Phase 1 : Migration SQL

**Fichier** : `sqlx-migrations/20260108000006_add_provider_settings_organization_id.sql`

```sql
-- Ajouter organization_id à provider_settings (NOT NULL car pas de données legacy)
ALTER TABLE provider_settings 
  ADD COLUMN organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE;

-- Contrainte unique : (provider_id, key, organization_id)
-- Permet à chaque organisation d'avoir ses propres settings pour chaque provider
ALTER TABLE provider_settings 
  ADD CONSTRAINT provider_settings_provider_key_org_uniq 
  UNIQUE (provider_id, key, organization_id);

-- Index pour performance (filtrage par organisation)
CREATE INDEX idx_provider_settings_org ON provider_settings(organization_id);
CREATE INDEX idx_provider_settings_provider_org ON provider_settings(provider_id, organization_id);
```

### Phase 2 : Modification du Seed

**Fichier** : `inventiv-api/src/setup/seeding.rs`

**Changements** :
1. Modifier `maybe_seed_provider_credentials()` pour :
   - Récupérer toutes les organisations existantes
   - Créer les credentials pour chaque organisation
   - Utiliser `organization_id` dans les INSERT

**Nouvelle logique** :
```rust
// Pour chaque organisation existante
for org_id in organizations {
    // Upsert SCALEWAY_PROJECT_ID avec organization_id
    // Upsert SCALEWAY_SECRET_KEY_ENC avec organization_id
}
```

### Phase 3 : Modification de ProviderManager

**Fichier** : `inventiv-orchestrator/src/provider_manager.rs`

**Changements** :
1. Modifier `scaleway_init_from_db()` pour accepter `organization_id: Option<Uuid>`
2. Modifier toutes les requêtes SQL pour filtrer par `organization_id` :
   ```sql
   WHERE provider_id=$1 AND key='SCALEWAY_PROJECT_ID' AND organization_id=$2
   ```
3. Modifier `get_provider()` pour accepter `organization_id: Option<Uuid>`
4. Si `organization_id` est `None`, retourner une erreur claire (pas de fallback)

### Phase 4 : Modification de l'API pour définir organization_id sur les instances

**Fichier** : `inventiv-api/src/handlers/deployments.rs`

**Changements** :
1. Ajouter `Extension(user): Extension<auth::AuthUser>` à `create_deployment()`
2. Récupérer `organization_id` depuis `user.current_organization_id`
3. Ajouter `organization_id` dans l'INSERT initial de l'instance :
   ```sql
   INSERT INTO instances (id, provider_id, zone_id, instance_type_id, organization_id, status, ...)
   VALUES ($1, $2, NULL, NULL, $3, 'provisioning', ...)
   ```
4. Si `organization_id` est `None`, retourner une erreur (l'utilisateur doit être dans une organisation)

### Phase 5 : Modification de process_create dans l'orchestrator

**Fichier** : `inventiv-orchestrator/src/services.rs`

**Changements** :
1. Récupérer `organization_id` depuis l'instance au début de `process_create()` :
   ```rust
   let organization_id: Option<Uuid> = sqlx::query_scalar(
       "SELECT organization_id FROM instances WHERE id = $1"
   )
   .bind(instance_uuid)
   .fetch_optional(&pool)
   .await
   .ok()
   .flatten();
   ```
2. Passer `organization_id` à `ProviderManager::get_provider()` :
   ```rust
   ProviderManager::get_provider(&provider_name, organization_id, pool.clone()).await
   ```
3. Si `organization_id` est `None`, échouer avec un message clair

### Phase 6 : Modification des autres jobs

**Fichiers** :
- `inventiv-orchestrator/src/health_check_job.rs`
- `inventiv-orchestrator/src/watch_dog_job.rs`
- `inventiv-orchestrator/src/terminator_job.rs`
- `inventiv-orchestrator/src/volume_reconciliation_job.rs`

**Changements** :
1. Récupérer `organization_id` depuis l'instance dans chaque job
2. Passer `organization_id` à `get_provider()`

### Phase 7 : Modification de ScalewayProvider pour organization_id et access_key

**Fichier** : `inventiv-providers/src/scaleway.rs`

**Changements** :
1. Ajouter `organization_id` et `access_key` dans `ScalewayProvider` struct (déjà fait)
2. Modifier `resize_block_storage()` pour utiliser ces valeurs (déjà fait)
3. Modifier `ProviderManager::scaleway_init_from_db()` pour lire aussi :
   - `SCALEWAY_ORGANIZATION_ID` depuis `provider_settings` (par organisation)
   - `SCALEWAY_ACCESS_KEY` depuis `provider_settings` (par organisation)
4. Stocker ces valeurs dans `ScalewayProvider` à l'initialisation

### Phase 8 : Mise à jour du seed pour SCALEWAY_ACCESS_KEY et SCALEWAY_ORGANIZATION_ID

**Fichier** : `inventiv-api/src/setup/seeding.rs`

**Changements** :
1. Lire `SCALEWAY_ACCESS_KEY` depuis `/run/secrets/scaleway_access_key`
2. Lire `SCALEWAY_ORGANIZATION_ID` depuis l'environnement (ou le calculer depuis le project_id)
3. Stocker ces valeurs dans `provider_settings` avec `organization_id`

## 🔍 Points d'attention

### 1. SCALEWAY_ORGANIZATION_ID
- Cette valeur est-elle la même pour toutes les organisations ou différente ?
- Si différente, comment la récupérer par organisation ?
- Si identique, peut-on la stocker globalement ou doit-elle être dupliquée par organisation ?

### 2. SCALEWAY_ACCESS_KEY
- Même question : identique ou différent par organisation ?
- Actuellement dans `/run/secrets/scaleway_access_key` (fichier unique)

### 3. Gestion des organisations multiples
- Le seed doit créer les credentials pour toutes les organisations existantes
- Ou seulement pour l'organisation par défaut ?
- **Recommandation** : Pour toutes les organisations existantes au moment du seed

### 4. Validation
- Si une instance n'a pas d'`organization_id`, doit-on échouer immédiatement ?
- **Recommandation** : Oui, échouer avec un message clair (pas de fallback)

## 📊 Ordre d'implémentation recommandé

1. ✅ **Migration SQL** (Phase 1)
2. ✅ **Modification du seed** (Phase 2 + Phase 8)
3. ✅ **Modification ProviderManager** (Phase 3)
4. ✅ **Modification API create_deployment** (Phase 4)
5. ✅ **Modification process_create** (Phase 5)
6. ✅ **Modification autres jobs** (Phase 6)
7. ✅ **Tests end-to-end**

## 🧪 Tests à effectuer

1. Seed des credentials pour toutes les organisations
2. Création d'instance avec organisation → vérifier que les bons credentials sont utilisés
3. Resize de volume → vérifier que les credentials sont trouvés
4. Création d'instance sans organisation → doit échouer avec message clair
5. Création d'instance avec organisation sans credentials → doit échouer avec message clair

