# Storage Management & Volume Lifecycle

## Vue d'ensemble

Le système gère automatiquement le cycle de vie des volumes de stockage attachés aux instances, incluant :
- **Découverte automatique** : Détection des volumes créés par le provider
- **Tracking** : Suivi dans `instance_volumes`
- **Suppression automatique** : Nettoyage lors de la terminaison
- **Types de volumes** : Boot volumes, data volumes, volumes locaux

## Table `instance_volumes`

### Schéma

```sql
CREATE TABLE instance_volumes (
    id uuid PRIMARY KEY,
    instance_id uuid NOT NULL REFERENCES instances(id),
    provider_id uuid NOT NULL REFERENCES providers(id),
    zone_code text NOT NULL,
    provider_volume_id text NOT NULL,
    provider_volume_name text,
    volume_type text NOT NULL,  -- 'b_ssd', 'l_ssd', 'unified', etc.
    size_bytes bigint NOT NULL,
    perf_iops integer,
    delete_on_terminate boolean DEFAULT true NOT NULL,
    status text DEFAULT 'attached' NOT NULL,  -- 'attached', 'detached', 'deleting', 'deleted'
    created_at timestamptz DEFAULT now(),
    attached_at timestamptz,
    deleted_at timestamptz,
    error_message text,
    is_boot boolean DEFAULT false NOT NULL
);

CREATE UNIQUE INDEX idx_instance_volumes_unique 
  ON instance_volumes(instance_id, provider_volume_id) 
  WHERE deleted_at IS NULL;
```

### Champs importants

- **`provider_volume_id`** : Identifiant unique du volume chez le provider
- **`volume_type`** : Type de volume (ex: `b_ssd` pour Block Storage, `l_ssd` pour Local SSD)
- **`delete_on_terminate`** : Flag pour suppression automatique lors de la terminaison
- **`is_boot`** : Indique si c'est un volume de boot
- **`status`** : État du volume (`attached`, `detached`, `deleting`, `deleted`)

## Découverte automatique

### Lors de la création

Après la création d'une instance, le système **découvre automatiquement** tous les volumes attachés :

```rust
// Dans process_create (services.rs)
if is_scaleway {
    if let Ok(attached_volumes) = provider.list_attached_volumes(&zone, &server_id).await {
        for av in attached_volumes {
            // Insère dans instance_volumes si pas déjà présent
        }
    }
}
```

**Cas d'usage** :
- Volumes de boot créés automatiquement par Scaleway (ex: RENDER-S)
- Volumes de données attachés explicitement
- Volumes créés par le provider sans être explicitement trackés

### Lors de la terminaison

Avant la suppression, le système **redécouvre** les volumes pour s'assurer qu'aucun n'est oublié :

```rust
// Dans process_termination (terminator_job.rs)
if let Ok(attached_volumes) = provider.list_attached_volumes(&zone, &provider_instance_id).await {
    for av in attached_volumes {
        // Insère dans instance_volumes si pas déjà tracké
        // Marque delete_on_terminate=true
    }
}
```

## Gestion du cycle de vie

### Création

#### Volumes de données explicites
- Créés via `PROVIDER_CREATE_VOLUME`
- Trackés immédiatement dans `instance_volumes`
- `delete_on_terminate=true` par défaut

#### Volumes de boot automatiques
- Créés automatiquement par Scaleway pour certains types d'instances
- Découverts après création de l'instance
- Trackés avec `is_boot=true`

### Terminaison

#### Processus de suppression

1. **Arrêt de l'instance** : `PROVIDER_STOP` (si nécessaire)
2. **Découverte des volumes** : `list_attached_volumes` pour trouver tous les volumes
3. **Marquage pour suppression** : Tous les volumes avec `delete_on_terminate=true`
4. **Suppression séquentielle** : `PROVIDER_DELETE_VOLUME` pour chaque volume
5. **Suppression de l'instance** : `PROVIDER_DELETE`
6. **Transition d'état** : `terminating → terminated`

#### Logging

Chaque suppression de volume génère une action :
- **Action type** : `PROVIDER_DELETE_VOLUME`
- **Métadonnées** : `zone`, `volume_id`, `volume_type`, `size_bytes`
- **Status** : `success` ou `failed` avec message d'erreur

### Cas spéciaux

#### Volumes locaux (L40S, L4, RENDER-S)

Certains types d'instances Scaleway nécessitent un **boot diskless** :
- **L40S-2-48G** : Pas de volumes locaux autorisés
- **L4-2-24G** : Pas de volumes locaux autorisés
- **RENDER-S** : Boot diskless avec volumes locaux créés automatiquement

**Vérification** :
- Avant `PROVIDER_START` : Vérifie qu'aucun volume local (`l_ssd`) n'est attaché
- Si détecté : Instance marquée `provisioning_failed` avec erreur explicite

#### Volumes persistants

Pour garder un volume après terminaison :
```sql
UPDATE instance_volumes 
SET delete_on_terminate = false 
WHERE instance_id = '<uuid>' AND provider_volume_id = '<volume-id>';
```

## Provider Implementation

### Trait `CloudProvider`

```rust
async fn list_attached_volumes(
    &self,
    zone: &str,
    server_id: &str,
) -> Result<Vec<AttachedVolume>>;
```

### Structure `AttachedVolume`

```rust
pub struct AttachedVolume {
    pub provider_volume_id: String,
    pub provider_volume_name: Option<String>,
    pub volume_type: String,  // 'b_ssd', 'l_ssd', etc.
    pub size_bytes: Option<i64>,
    pub boot: bool,
}
```

### Implémentation Scaleway

- **Endpoint** : `GET /instance/v1/zones/{zone}/servers/{server_id}`
- **Extraction** : Parse `server["volumes"]` array
- **Mapping** : Convertit les types Scaleway (`b_ssd`, `l_ssd`) vers notre modèle

## Monitoring & Observabilité

### Métadonnées d'instance

Les instances incluent maintenant :
- **`storage_count`** : Nombre de volumes attachés (non supprimés)
- **`storage_sizes_gb`** : Tailles des volumes en GB

### Requêtes SQL utiles

```sql
-- Volumes attachés à une instance
SELECT 
  iv.provider_volume_id,
  iv.volume_type,
  iv.size_bytes / 1024 / 1024 / 1024 as size_gb,
  iv.is_boot,
  iv.delete_on_terminate,
  iv.status
FROM instance_volumes iv
WHERE iv.instance_id = '<uuid>' 
  AND iv.deleted_at IS NULL;

-- Volumes non supprimés après terminaison
SELECT 
  i.id as instance_id,
  i.status,
  iv.provider_volume_id,
  iv.volume_type,
  iv.size_bytes
FROM instances i
JOIN instance_volumes iv ON iv.instance_id = i.id
WHERE i.status = 'terminated'
  AND iv.deleted_at IS NULL
  AND iv.delete_on_terminate = true;

-- Volumes de boot créés automatiquement
SELECT 
  i.id as instance_id,
  i.provider_instance_id,
  iv.provider_volume_id,
  iv.volume_type,
  iv.size_bytes / 1024 / 1024 / 1024 as size_gb
FROM instances i
JOIN instance_volumes iv ON iv.instance_id = i.id
WHERE iv.is_boot = true
  AND iv.deleted_at IS NULL;
```

### Action Logs

Les actions de volume sont loggées dans `action_logs` :
- **`PROVIDER_CREATE_VOLUME`** : Création d'un volume
- **`PROVIDER_DELETE_VOLUME`** : Suppression d'un volume

**Métadonnées** :
```json
{
  "zone": "fr-par-2",
  "volume_id": "scaleway-volume-id",
  "volume_type": "b_ssd",
  "size_gb": 200,
  "correlation_id": "uuid"
}
```

## Problèmes connus et solutions

### Volumes non supprimés

**Symptôme** : Volume reste présent après terminaison de l'instance

**Causes possibles** :
1. Volume non découvert lors de la terminaison
2. Erreur lors de la suppression (provider API)
3. `delete_on_terminate=false` configuré

**Solution** :
- Vérifier `action_logs` pour `PROVIDER_DELETE_VOLUME`
- Vérifier `instance_volumes.deleted_at` et `status`
- Supprimer manuellement si nécessaire

### Volumes locaux non autorisés

**Symptôme** : Erreur `The total size of local-volume(s) must be equal to 0GB`

**Cause** : Instance type nécessite boot diskless mais Scaleway a créé un volume local

**Solution** :
- Vérification préventive avant `PROVIDER_START`
- Instance marquée `provisioning_failed` avec message explicite
- Vérifier l'image boot utilisée (doit être compatible diskless)

## Code de référence

- **Volume discovery** : `inventiv-providers/src/lib.rs` → `list_attached_volumes`
- **Scaleway implementation** : `inventiv-providers/src/scaleway.rs`
- **Termination logic** : `inventiv-orchestrator/src/terminator_job.rs`
- **Creation logic** : `inventiv-orchestrator/src/services.rs` → `process_create`
- **Schema** : `sqlx-migrations/00000000000000_baseline.sql`

