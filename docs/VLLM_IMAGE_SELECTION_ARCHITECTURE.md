# Architecture de sélection d'image vLLM par type d'instance/GPU

## Problème

Les différentes instances Scaleway ont des GPUs avec des compute capabilities différentes :
- **RENDER-S** : Tesla P100 (compute capability 6.0) - nécessite vLLM compilé avec support `sm_60`
- **L4-2-24G** : NVIDIA L4 (compute capability 8.9) - compatible avec vLLM récent
- **L40S-2-48G** : NVIDIA L40S (compute capability 8.9) - compatible avec vLLM récent

L'image `vllm/vllm-openai:latest` n'est pas compatible avec P100 car elle est compilée pour `sm_70+` uniquement.

## Solution recommandée : Hiérarchie de résolution

### Principe

Utiliser une hiérarchie de résolution avec fallback pour déterminer l'image vLLM à utiliser :

```
1. instance_types.allocation_params.vllm_image (spécifique au type d'instance)
   ↓ (si non défini)
2. provider_settings.WORKER_VLLM_IMAGE_<INSTANCE_TYPE_CODE> (par type d'instance)
   ↓ (si non défini)
3. provider_settings.WORKER_VLLM_IMAGE (par défaut pour le provider)
   ↓ (si non défini)
4. WORKER_VLLM_IMAGE (variable d'environnement)
   ↓ (si non défini)
5. Défaut hardcodé (version stable, pas "latest")
```

### Avantages

- **Flexibilité** : Permet de définir une image spécifique par type d'instance
- **Maintenabilité** : Configuration centralisée dans la DB
- **Stabilité** : Pas de "latest", versions explicites
- **Évolutivité** : Facile d'ajouter de nouveaux types d'instances
- **Fallback** : Système de secours si configuration spécifique manquante

### Implémentation

#### 1. Migration SQL : Ajouter les images vLLM dans `allocation_params`

```sql
-- Mettre à jour RENDER-S avec image vLLM compatible P100
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.6.2.post1"'::jsonb
)
WHERE code = 'RENDER-S';

-- Mettre à jour L4 avec image vLLM récente (version stable)
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.6.2.post1"'::jsonb
)
WHERE code LIKE 'L4-%';

-- Mettre à jour L40S avec image vLLM récente (version stable)
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.6.2.post1"'::jsonb
)
WHERE code LIKE 'L40S-%';
```

#### 2. Fonction de résolution dans `inventiv-orchestrator/src/services.rs`

```rust
/// Résout l'image vLLM à utiliser pour une instance donnée
/// Hiérarchie de résolution :
/// 1. instance_types.allocation_params.vllm_image
/// 2. provider_settings.WORKER_VLLM_IMAGE_<INSTANCE_TYPE_CODE>
/// 3. provider_settings.WORKER_VLLM_IMAGE
/// 4. WORKER_VLLM_IMAGE (env var)
/// 5. Défaut hardcodé (version stable)
async fn resolve_vllm_image(
    pool: &Pool<Postgres>,
    instance_type_id: Uuid,
    provider_id: Option<Uuid>,
    instance_type_code: &str,
) -> String {
    // 1. Vérifier allocation_params de l'instance_type
    if let Ok(Some(vllm_image)) = sqlx::query_scalar::<_, Option<String>>(
        "SELECT allocation_params->>'vllm_image' FROM instance_types WHERE id = $1"
    )
    .bind(instance_type_id)
    .fetch_optional(pool)
    .await
    {
        if let Some(img) = vllm_image {
            if !img.trim().is_empty() {
                return img;
            }
        }
    }
    
    // 2. Vérifier provider_settings.WORKER_VLLM_IMAGE_<INSTANCE_TYPE_CODE>
    if let Some(pid) = provider_id {
        let setting_key = format!("WORKER_VLLM_IMAGE_{}", instance_type_code.replace("-", "_").to_uppercase());
        if let Ok(Some(img)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT value_text FROM provider_settings WHERE provider_id = $1 AND key = $2"
        )
        .bind(pid)
        .bind(&setting_key)
        .fetch_optional(pool)
        .await
        {
            if let Some(img) = img {
                if !img.trim().is_empty() {
                    return img;
                }
            }
        }
    }
    
    // 3. Vérifier provider_settings.WORKER_VLLM_IMAGE (défaut provider)
    if let Some(pid) = provider_id {
        if let Ok(Some(img)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT value_text FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_IMAGE'"
        )
        .bind(pid)
        .fetch_optional(pool)
        .await
        {
            if let Some(img) = img {
                if !img.trim().is_empty() {
                    return img;
                }
            }
        }
    }
    
    // 4. Vérifier variable d'environnement
    if let Ok(img) = std::env::var("WORKER_VLLM_IMAGE") {
        if !img.trim().is_empty() {
            return img;
        }
    }
    
    // 5. Défaut hardcodé (version stable, pas "latest")
    // Note: v0.6.2.post1 est une version stable qui supporte P100 si compilée avec sm_60
    // Pour les instances récentes (L4, L40S), utiliser une version plus récente
    // TODO: Déterminer la meilleure version par défaut selon le type d'instance
    "vllm/vllm-openai:v0.6.2.post1".to_string()
}
```

#### 3. Utilisation dans `process_create` et `maybe_trigger_worker_install_over_ssh`

Remplacer la logique actuelle de résolution de `vllm_image` par un appel à `resolve_vllm_image`.

### Versions vLLM recommandées

#### Pour RENDER-S (P100, compute capability 6.0)
⚠️ **IMPORTANT** : Les images officielles vLLM (`vllm/vllm-openai:*`) ne supportent **PAS** P100 (sm_60) car elles sont compilées pour sm_70+ uniquement.

**Options disponibles** :
- **Option 1** : Compiler vLLM depuis les sources avec `TORCH_CUDA_ARCH_LIST="6.0;7.0;7.5;8.0;8.6;8.9"` et créer une image Docker personnalisée
- **Option 2** : Utiliser une version plus ancienne de vLLM qui incluait le support P100 (à identifier)
- **Option 3** : Utiliser une image alternative ou un fork de vLLM avec support P100
- **Option 4** : Migrer vers des instances L4/L40S qui sont pleinement supportées

**Note actuelle** : La migration SQL utilise `v0.6.2.post1` comme placeholder pour RENDER-S, mais cette version **ne fonctionnera pas** sur P100. Il faut soit compiler une version personnalisée, soit utiliser une alternative.

#### Pour L4/L40S (compute capability 8.9)
- **Recommandé** : `vllm/vllm-openai:v0.6.2.post1` ou version plus récente stable
- **Éviter** : `latest` (instabilité)

### Configuration dans la base de données

#### Exemple de configuration pour Scaleway

```sql
-- Configuration par défaut pour Scaleway (pour instances sans config spécifique)
INSERT INTO provider_settings (provider_id, key, value_text)
SELECT id, 'WORKER_VLLM_IMAGE', 'vllm/vllm-openai:v0.6.2.post1'
FROM providers WHERE code = 'scaleway'
ON CONFLICT (provider_id, key) DO UPDATE SET value_text = EXCLUDED.value_text;

-- Configuration spécifique pour RENDER-S (si nécessaire)
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.6.2.post1"'::jsonb  -- Version compatible P100
)
WHERE code = 'RENDER-S';

-- Configuration spécifique pour L4 (version récente)
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.6.2.post1"'::jsonb  -- Version stable récente
)
WHERE code LIKE 'L4-%';

-- Configuration spécifique pour L40S (version récente)
UPDATE instance_types 
SET allocation_params = jsonb_set(
    COALESCE(allocation_params, '{}'::jsonb),
    '{vllm_image}',
    '"vllm/vllm-openai:v0.6.2.post1"'::jsonb  -- Version stable récente
)
WHERE code LIKE 'L40S-%';
```

### Alternative : Configuration par GPU compute capability

Si on veut être plus générique, on peut mapper par compute capability plutôt que par type d'instance :

```sql
-- Ajouter une colonne compute_capability dans instance_types (ou dans allocation_params)
-- Puis mapper :
-- compute_capability = 6.0 -> vllm/vllm-openai:v0.6.2.post1 (P100)
-- compute_capability >= 7.0 -> vllm/vllm-openai:v0.6.2.post1 (L4, L40S, etc.)
```

### Avantages de cette approche

1. **Centralisé** : Configuration dans la DB, facilement modifiable
2. **Flexible** : Permet des configurations spécifiques par type d'instance
3. **Maintenable** : Pas de hardcoding dans le code
4. **Évolutif** : Facile d'ajouter de nouveaux types d'instances
5. **Stable** : Pas de "latest", versions explicites
6. **Fallback** : Système de secours si configuration manquante

### Prochaines étapes

1. Créer la migration SQL pour ajouter les images vLLM dans `allocation_params`
2. Implémenter la fonction `resolve_vllm_image` dans `services.rs`
3. Mettre à jour `process_create` et `maybe_trigger_worker_install_over_ssh` pour utiliser cette fonction
4. Tester avec RENDER-S (P100) et L4/L40S
5. Documenter les versions vLLM recommandées pour chaque type d'instance

