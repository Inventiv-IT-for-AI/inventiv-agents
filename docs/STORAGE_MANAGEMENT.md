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
    reconciled_at timestamptz,  -- Timestamp de réconciliation complète (après vérification provider)
    last_reconciliation timestamptz,  -- Dernière tentative de réconciliation (pour backoff)
    error_message text,
    is_boot boolean DEFAULT false NOT NULL
);

CREATE UNIQUE INDEX idx_instance_volumes_unique 
  ON instance_volumes(instance_id, provider_volume_id) 
  WHERE deleted_at IS NULL;

CREATE UNIQUE CONSTRAINT instance_volumes_unique_constraint 
  ON instance_volumes(instance_id, provider_volume_id);
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

### Principe de préservation des données

**Toutes les données de volumes sont préservées** dans la table `instance_volumes` pour :
- **Audit** : Traçabilité complète de tous les volumes alloués et libérés
- **FinOps** : Calculs et recalculs précis des coûts basés sur l'usage détaillé à la seconde près
- **Debug** : Analyse des problèmes de suppression ou de réconciliation
- **Analyse** : Statistiques sur les volumes créés/supprimés par type, zone, instance

**Champs de suivi** :
- `created_at` : Création du volume
- `attached_at` : Attachement à l'instance
- `deleted_at` : Demande de suppression (marqué par terminator)
- `reconciled_at` : Réconciliation complète (marqué par job-volume-reconciliation après vérification provider)
- `last_reconciliation` : Dernière tentative de réconciliation (pour backoff)

**Aucune donnée n'est jamais supprimée** - seulement marquée avec des timestamps pour indiquer l'état du cycle de vie.

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
5. **Marquage dans DB** : `status='deleted'`, `deleted_at=NOW()` (marque la demande de suppression)
6. **Suppression de l'instance** : `PROVIDER_DELETE`
7. **Transition d'état** : `terminating → terminated`

#### Réconciliation des volumes

Le job `job-volume-reconciliation` vérifie périodiquement (toutes les 60s) :
- **Volumes marqués `deleted_at` mais non réconciliés** : Vérifie si le volume existe encore chez le provider
  - Si existe : Retry la suppression
  - Si n'existe plus : Marque `reconciled_at=NOW()` (réconciliation complète)

**Important** : Toutes les données sont **préservées** dans la DB pour :
- **Audit** : Traçabilité complète de tous les volumes alloués et libérés
- **FinOps** : Calculs et recalculs précis des coûts basés sur l'usage détaillé à la seconde près
- **Debug** : Analyse des problèmes de suppression
- **Analyse** : Statistiques sur les volumes créés/supprimés

**Champs de réconciliation** :
- `deleted_at` : Timestamp de la demande de suppression (marqué par terminator)
- `reconciled_at` : Timestamp de la réconciliation complète (marqué par job-volume-reconciliation après vérification provider)
- `last_reconciliation` : Timestamp de la dernière tentative de réconciliation (pour backoff)

#### Logging

Chaque suppression de volume génère une action :
- **Action type** : `PROVIDER_DELETE_VOLUME`
- **Métadonnées** : `zone`, `volume_id`, `volume_type`, `size_bytes`
- **Status** : `success` ou `failed` avec message d'erreur

Chaque réconciliation génère une action :
- **Action type** : `VOLUME_RECONCILIATION_RETRY_DELETE` ou `VOLUME_RECONCILIATION_ORPHAN`
- **Métadonnées** : `zone`, `volume_id`, `instance_id`, `reason`
- **Status** : `success` (volume confirmé supprimé) ou `failed` (retry nécessaire)

### Cas spéciaux

#### Volumes locaux (L40S, L4, RENDER-S)

Certains types d'instances Scaleway nécessitent un **boot diskless** :
- **L40S-2-48G** : Pas de volumes locaux autorisés
- **L4-2-24G** : Pas de volumes locaux autorisés
- **RENDER-S** : Boot diskless avec volumes Local Storage créés automatiquement par Scaleway

**Comportement RENDER-S** :
- Scaleway crée automatiquement un volume Local Storage (`l_ssd`) de 400GB lors de la création
- Ce volume est détecté et tracké dans `instance_volumes` avec `volume_type=l_ssd`
- `delete_on_terminate=true` pour les volumes Local Storage auto-créés (suppression automatique)
- **Pas de création de Block Storage** : Le code skip la création de volumes Block Storage pour RENDER-S
- Le volume Local Storage est utilisé pour le stockage des données

**Vérification** :
- Avant `PROVIDER_START` : Vérifie qu'aucun volume local (`l_ssd`) n'est attaché pour L40S/L4
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

