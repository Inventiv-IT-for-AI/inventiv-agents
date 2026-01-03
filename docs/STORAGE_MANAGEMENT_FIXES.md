# Corrections de la gestion des volumes (Storage Management)

**Date**: 2026-01-03  
**Problème**: Volumes non trackés et création de volumes data échouant

---

## Problèmes identifiés

### 1. Création de volumes data échoue

**Symptôme**: `PROVIDER_CREATE_VOLUME` échoue avec "Provider does not support volume creation"

**Cause**: La méthode `create_volume` n'était pas implémentée dans le provider Scaleway. Le trait `CloudProvider` retourne `Ok(None)` par défaut, ce qui génère cette erreur.

**Solution**: Implémentation de `create_volume` et `attach_volume` dans `inventiv-providers/src/scaleway.rs` :
- Utilise l'API Block Storage de Scaleway (`/block/v1/zones/{zone}/volumes`)
- Convertit les bytes en GB (Scaleway attend des GB)
- Gère les erreurs et logging appropriés

### 2. Volume de boot (400GB) non tracké

**Symptôme**: Un volume de 400GB est créé automatiquement par Scaleway mais n'apparaît pas dans `instance_volumes` ni dans l'API (`storage_count=0`, `storage_sizes_gb=[]`)

**Cause**: 
- Le code filtrait les volumes par `volume_type != "sbs_volume"` avant la création du volume data
- Les volumes boot créés automatiquement par Scaleway peuvent être de type différent (local volumes, etc.)
- La découverte des volumes se faisait en deux endroits avec des logiques différentes

**Solution**: 
- **Découverte immédiate** après `PROVIDER_CREATE` : Tous les volumes attachés sont trackés (pas de filtre par type)
- **Mise à jour métadonnées** avant création volume data : Seulement mise à jour des volumes déjà trackés si métadonnées incomplètes
- **delete_on_terminate** : Les volumes boot sont marqués `delete_on_terminate=true` pour suppression automatique

---

## Modifications apportées

### 1. `inventiv-providers/src/scaleway.rs`

**Ajout de `create_volume`**:
```rust
async fn create_volume(
    &self,
    zone: &str,
    name: &str,
    size_bytes: i64,
    volume_type: &str,
    _perf_iops: Option<i32>,
) -> Result<Option<String>> {
    // Utilise Block Storage API de Scaleway
    // Convertit bytes → GB
    // Retourne volume_id
}
```

**Ajout de `attach_volume`**:
```rust
async fn attach_volume(
    &self,
    zone: &str,
    server_id: &str,
    volume_id: &str,
    _delete_on_termination: bool,
) -> Result<bool> {
    // Attache le volume à l'instance via Instance API
}
```

### 2. `inventiv-orchestrator/src/services.rs`

**Découverte des volumes après `PROVIDER_CREATE`** (ligne ~1427):
- **AVANT**: Trackait tous les volumes mais avec `delete_on_terminate=TRUE` pour tous
- **APRÈS**: Tracke tous les volumes avec `delete_on_terminate` basé sur `av.boot` (boot volumes = true)

**Simplification avant création volume data** (ligne ~1666):
- **AVANT**: Filtrait par `volume_type != "sbs_volume"` et créait des entrées manquantes
- **APRÈS**: Met seulement à jour les métadonnées des volumes déjà trackés (pas de création, pas de filtre)

---

## Comportement attendu après corrections

### 1. Création d'instance

1. **PROVIDER_CREATE** : Instance créée avec volume boot automatique (ex: 400GB)
2. **Découverte volumes** : Tous les volumes attachés sont trackés dans `instance_volumes`
   - Volume boot : `is_boot=true`, `delete_on_terminate=true`
   - Volume data : Sera créé et tracké ensuite
3. **PROVIDER_CREATE_VOLUME** : Volume data créé (ex: 200GB pour Qwen 7B)
4. **PROVIDER_ATTACH_VOLUME** : Volume data attaché à l'instance
5. **API** : `storage_count=2`, `storage_sizes_gb=[400, 200]`

### 2. Terminaison d'instance

1. **Découverte volumes** : Tous les volumes attachés sont trackés (même si pas dans DB)
2. **Suppression volumes** : Tous les volumes avec `delete_on_terminate=true` sont supprimés
   - Volume boot : Supprimé
   - Volume data : Supprimé (si `delete_on_terminate=true`)

---

## Questions résolues

### D'où vient le volume de 400GB ?

**Réponse**: Scaleway crée automatiquement un volume boot lors de la création d'instance. La taille dépend du type d'instance et de l'image utilisée. Pour RENDER-S, Scaleway crée généralement un volume boot de 400GB.

### Où est définie la valeur de 400GB ?

**Réponse**: C'est Scaleway qui définit cette taille automatiquement lors de la création de l'instance. Ce n'est pas configurable côté Inventiv-Agents. La taille peut varier selon :
- Le type d'instance
- L'image utilisée
- Les paramètres par défaut de Scaleway

### Est-il possible d'allouer plus ou moins ?

**Réponse**: 
- **Volume boot** : Non configurable (défini par Scaleway)
- **Volume data** : Oui, configurable via :
  - `models.data_volume_gb` (par modèle)
  - `provider_settings.worker_data_volume_gb_default` (par défaut)
  - `WORKER_DATA_VOLUME_GB` (env var, force pour tous)
  - Logique heuristique dans `worker_storage::recommended_data_volume_gb()` (ex: 200GB pour 7B)

### Comment garantir que cette info soit gérée dans les attributs de l'instance ?

**Réponse**: 
- ✅ Tous les volumes sont trackés dans `instance_volumes` après `PROVIDER_CREATE`
- ✅ `storage_count` et `storage_sizes_gb` sont calculés depuis `instance_volumes` dans l'API
- ✅ Endpoint `GET /instances/:id` retourne `storages[]` avec détails complets

### Comment garantir que le volume soit libéré à la terminaison ?

**Réponse**:
- ✅ Tous les volumes sont découverts lors de la terminaison (même si pas dans DB)
- ✅ Les volumes avec `delete_on_terminate=true` sont supprimés automatiquement
- ✅ Les volumes boot sont marqués `delete_on_terminate=true` par défaut
- ✅ Les volumes data suivent la config `delete_on_terminate` (généralement `true`)

---

## Tests recommandés

1. **Création instance Scaleway** :
   - Vérifier que `storage_count > 0` après `PROVIDER_CREATE`
   - Vérifier que `storage_sizes_gb` contient la taille du volume boot
   - Vérifier que `PROVIDER_CREATE_VOLUME` réussit
   - Vérifier que `storage_count` augmente après création volume data

2. **Terminaison instance** :
   - Vérifier que tous les volumes sont découverts
   - Vérifier que les volumes avec `delete_on_terminate=true` sont supprimés
   - Vérifier qu'aucun volume ne reste dans Scaleway après terminaison

3. **API** :
   - Vérifier que `GET /instances/:id` retourne `storages[]` avec tous les volumes
   - Vérifier que `storage_count` et `storage_sizes_gb` sont corrects

---

## Notes techniques

- **Volume boot** : Créé automatiquement par Scaleway, taille variable selon instance/image
- **Volume data** : Créé par Inventiv-Agents via Block Storage API, taille configurable
- **Tracking** : Tous les volumes sont trackés dans `instance_volumes` avec `is_boot` flag
- **Suppression** : Basée sur `delete_on_terminate` flag, découverte automatique lors de terminaison

---

## Prochaines étapes

1. ✅ Implémenter `create_volume` et `attach_volume` dans Scaleway provider
2. ✅ Corriger la découverte des volumes boot
3. ✅ S'assurer que tous les volumes sont trackés
4. ⏳ Tester en staging avec une instance réelle
5. ⏳ Vérifier que les volumes sont supprimés lors de la terminaison

