# Analyse Approfondie des Logs - Instance Failed

## Instance Analysée

- **ID**: `4cdf43a1-d5db-4829-93e9-f3059a2a41b7`
- **Type**: `L40S-2-48G` (Scaleway)
- **Zone**: `fr-par-2`
- **Modèle**: `Qwen/Qwen2.5-14B-Instruct`
- **Statut final**: `failed`
- **Error Code**: `SCW_DISKLESS_BOOT_IMAGE_REQUIRED`

---

## Chronologie des Événements

### Timeline Détaillée

```
18:14:49.491716Z  │ Instance créée en DB (status='provisioning')
                  │
18:14:49.502952Z  │ ┌─ REQUEST_CREATE démarre (API)
                  │ │  - Validation requête
                  │ │  - INSERT instances (status='provisioning')
                  │ │  - LOG: REQUEST_CREATE (in_progress)
                  │ │  - Publie CMD:PROVISION dans Redis
18:14:49.537944Z  │ └─ REQUEST_CREATE complété (53ms) ✅ SUCCESS
                  │
                  │ [Redis Pub/Sub: événement CMD:PROVISION transmis]
                  │
18:14:49.584540Z  │ ┌─ EXECUTE_CREATE démarre (Orchestrator)
                  │ │  - Reçoit CMD:PROVISION depuis Redis
                  │ │  - LOG: EXECUTE_CREATE (in_progress)
                  │ │  - Résout provider (Scaleway)
                  │ │  - Vérifie instance_type (L40S-2-48G)
                  │ │  - Détecte: scaleway_requires_diskless_boot_image() = true
                  │ │  - Cherche boot_image_id dans allocation_params → NULL
                  │ │  - Appelle provider.resolve_boot_image()
                  │ │    └─ ❌ Retourne Ok(None) immédiatement (STUB!)
                  │ │  - Erreur: "Auto-discovery did not find a suitable image"
                  │ │  - UPDATE instances SET status='failed'
                  │ │  - LOG: EXECUTE_CREATE (failed)
18:14:49.675226Z  │ └─ EXECUTE_CREATE complété (137ms) ❌ FAILED
```

---

## Analyse de la Séquence

### 1. Phase API (REQUEST_CREATE) - ✅ SUCCESS

**Durée**: 53ms

**Actions**:
- Validation de la requête (`model_id` présent, zone/type valides)
- Création de l'instance en DB avec `status='provisioning'`
- Publication de l'événement `CMD:PROVISION` dans Redis (`orchestrator_events`)

**Résultat**: Succès, l'instance est créée et l'événement est publié.

### 2. Phase Orchestrator (EXECUTE_CREATE) - ❌ FAILED

**Durée**: 137ms (très rapide pour un échec)

**Actions détaillées**:

#### 2.1 Résolution du Provider
- ✅ Provider résolu: `Scaleway` (code: `scaleway`)
- ✅ Zone validée: `fr-par-2`
- ✅ Instance type validé: `L40S-2-48G`

#### 2.2 Détection du Besoin d'Image Diskless
```rust
fn scaleway_requires_diskless_boot_image(instance_type: &str) -> bool {
    let t = instance_type.trim().to_ascii_uppercase();
    t.starts_with("L4-") || t.starts_with("L40S-")  // ✅ L40S-2-48G match
}
```
**Résultat**: `true` → L'instance nécessite une image diskless.

#### 2.3 Recherche de Configuration Manuelle
```sql
SELECT NULLIF(TRIM(it.allocation_params->'scaleway'->>'boot_image_id'), '')
FROM instance_types it
WHERE it.id = $1
```
**Résultat**: `NULL` → Aucune image boot configurée manuellement.

#### 2.4 Tentative d'Auto-Découverte ⚠️ **POINT DE DÉFAILLANCE**

```rust
match provider.resolve_boot_image(&zone, &instance_type).await {
    Ok(Some(img)) => { /* succès */ },
    Ok(None) => { /* échec - notre cas */ },
    Err(e) => { /* erreur API */ },
}
```

**Problème identifié**: La méthode `resolve_boot_image()` dans `ScalewayProvider` était un **stub** :

```rust
// AVANT (stub)
async fn resolve_boot_image(&self, _zone: &str, _instance_type: &str) -> Result<Option<String>> {
    Ok(None)  // ❌ Retourne immédiatement None sans appeler l'API Scaleway
}
```

**Conséquence**: 
- L'appel retourne `Ok(None)` en **< 1ms** (pas d'appel API réel)
- Le code détecte l'échec et marque l'instance comme `failed`
- Durée totale de 137ms inclut uniquement les opérations DB et la gestion d'erreur

#### 2.5 Gestion de l'Erreur

```rust
Ok(None) => {
    let msg = "Scaleway requires a diskless/compatible boot image for this instance type. Auto-discovery did not find a suitable image. Configure instance_types.allocation_params.scaleway.boot_image_id for this type.";
    // UPDATE instances SET status='failed', error_code='SCW_DISKLESS_BOOT_IMAGE_REQUIRED'
    return;
}
```

**Résultat**: Instance marquée `failed` avec message d'erreur explicite.

---

## Diagnostic Root Cause

### Cause Racine

**La méthode `resolve_boot_image()` n'était pas implémentée** dans le provider Scaleway. Elle retournait systématiquement `Ok(None)` sans tenter de découvrir une image boot diskless via l'API Scaleway.

### Pourquoi cela n'a pas été détecté plus tôt ?

1. **Tests manquants**: Aucun test E2E pour les types L4/L40S avec auto-découverte
2. **Documentation incomplète**: Le comportement attendu n'était pas documenté
3. **Fallback silencieux**: Le code échoue gracieusement mais ne signale pas le problème jusqu'à ce qu'un utilisateur tente de créer une instance

### Impact

- **Instances affectées**: Toutes les instances `L4-*` et `L40S-*` créées sans `boot_image_id` configuré manuellement
- **Workaround disponible**: Configuration manuelle via `allocation_params.scaleway.boot_image_id`
- **Expérience utilisateur**: Erreur claire mais frustrante (nécessite configuration manuelle)

---

## Solution Implémentée

### Correction Appliquée

Implémentation complète de `resolve_boot_image()` dans `inventiv-providers/src/scaleway.rs` :

```rust
async fn resolve_boot_image(&self, zone: &str, instance_type: &str) -> Result<Option<String>> {
    // 1. Appel API Scaleway pour lister les images publiques
    let url = format!("https://api.scaleway.com/instance/v1/zones/{}/images", zone);
    let resp = self.client.get(&url)
        .headers(self.headers())
        .query(&[("public", "true")])
        .send()
        .await?;

    // 2. Parse la réponse JSON
    let json_resp: serde_json::Value = resp.json().await?;
    let images = json_resp["images"].as_array()?;

    // 3. Filtre les images Ubuntu x86_64 (compatibles diskless)
    let mut candidates: Vec<(String, String, i32)> = vec![];
    for img in images {
        if arch == "x86_64" && name.contains("ubuntu") {
            // Priorise Ubuntu 22.04+ (jammy) ou 24.04+ (noble)
            let priority = if name.contains("22.04") || name.contains("jammy") { 1 }
                          else if name.contains("24.04") || name.contains("noble") { 2 }
                          else if name.contains("20.04") || name.contains("focal") { 3 }
                          else { 4 };
            candidates.push((id, name, priority));
        }
    }

    // 4. Retourne la meilleure image trouvée
    candidates.sort_by_key(|(_, _, p)| *p);
    Ok(candidates.first().map(|(id, _, _)| id.clone()))
}
```

### Comportement Attendu Après Correction

1. **Auto-découverte fonctionnelle**: L'appel API Scaleway sera effectué
2. **Durée augmentée**: ~200-500ms (appel API + parsing)
3. **Persistance automatique**: L'image trouvée sera sauvegardée dans `allocation_params` pour les provisions suivantes
4. **Fallback robuste**: Si aucune image n'est trouvée, erreur claire avec instructions

---

## Recommandations

### Court Terme

1. ✅ **Correction implémentée**: `resolve_boot_image()` fonctionnel
2. **Tester**: Créer une nouvelle instance `L40S-2-48G` pour valider
3. **Monitoring**: Surveiller les logs pour confirmer l'auto-découverte

### Moyen Terme

1. **Tests E2E**: Ajouter des tests pour l'auto-découverte d'images boot
2. **Documentation**: Documenter le comportement attendu dans la doc API
3. **Métriques**: Ajouter des métriques pour suivre le taux de succès d'auto-découverte

### Long Terme

1. **Cache**: Mettre en cache les images découvertes (éviter appels API répétés)
2. **Validation**: Valider que l'image trouvée est réellement diskless-compatible
3. **Fallback multiple**: Essayer plusieurs images si la première échoue

---

## Vérification Post-Correction

Pour vérifier que la correction fonctionne :

```bash
# 1. Vérifier que le code compile
cargo check -p inventiv-providers

# 2. Créer une nouvelle instance L40S-2-48G
# (l'auto-découverte devrait fonctionner)

# 3. Vérifier les logs orchestrator pour :
#    - "✅ Scaleway diskless boot image resolved: <uuid>"
#    - UPDATE instance_types SET allocation_params avec boot_image_id

# 4. Vérifier en DB que l'image a été persistée :
SELECT allocation_params->'scaleway'->>'boot_image_id'
FROM instance_types
WHERE code = 'L40S-2-48G';
```

---

## Conclusion

Le problème était dû à une **méthode stub non implémentée** qui retournait systématiquement `None` sans tenter de découvrir une image boot diskless. La correction implémente l'appel API Scaleway et la logique de filtrage/priorisation des images Ubuntu compatibles.

**Durée de résolution**: ~137ms (échec immédiat) → ~200-500ms (appel API + découverte)

**Impact utilisateur**: Les nouvelles instances `L40S-*` devraient maintenant fonctionner sans configuration manuelle.

