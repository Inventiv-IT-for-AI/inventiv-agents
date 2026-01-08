# Scaleway Provisioning - Guide Complet

> **Date de validation** : Janvier 2025  
> **Instance type validée** : L4-1-24G  
> **Status** : ✅ Production Ready

## Vue d'ensemble

Ce document décrit la séquence complète et validée pour le provisionnement d'instances Scaleway GPU (L4-1-24G) avec Block Storage, SSH opérationnel, et installation automatique du worker.

## Solution Validée

### Séquence de Provisionnement

```
1. Créer instance avec image uniquement
   ├─ Image: 5c3d28db-33ce-4997-8572-f49506339283 (Ubuntu Noble GPU OS 13 passthrough)
   ├─ Pas de champ "volumes" dans la requête
   └─ Scaleway crée automatiquement un Block Storage de 20GB avec le snapshot de l'image (bootable)

2. Agrandir le Block Storage à 200GB via CLI
   ├─ Le volume créé automatiquement contient déjà le snapshot (bootable)
   └─ Agrandissement possible sans problème

3. Démarrer l'instance
   └─ État: stopped → starting → running

4. Attendre IP publique
   └─ Récupération de l'adresse IP publique

5. Attendre SSH accessible
   └─ SSH accessible après ~20 secondes

6. Installation worker via SSH
   ├─ Docker, NVIDIA Container Toolkit, vLLM
   └─ Agent Python inventiv-worker

7. Health checks
   └─ Vérification worker opérationnel

8. Chargement modèle LLM
   └─ Prêt pour l'inférence
```

### Configuration de la Requête API

```json
{
  "name": "instance-name",
  "commercial_type": "L4-1-24G",
  "project": "project-id",
  "image": "5c3d28db-33ce-4997-8572-f49506339283",
  "tags": ["tag1", "tag2"],
  "dynamic_ip_required": true,
  "boot_type": "local"
  // IMPORTANT: Pas de champ "volumes" !
}
```

### Points Clés

1. **Image avec `sbs_snapshot`** : L'image utilisée doit avoir un `root_volume` de type `sbs_snapshot` pour que Scaleway crée automatiquement un Block Storage bootable.

2. **Pas de volumes dans la requête** : Ne pas spécifier le champ `volumes` lors de la création. Scaleway créera automatiquement un Block Storage de 20GB avec le snapshot de l'image.

3. **Agrandissement avant démarrage** : Agrandir le Block Storage créé automatiquement à 200GB via CLI **avant** de démarrer l'instance.

4. **SSH automatique** : Scaleway applique automatiquement les clés SSH du projet, pas besoin de `user_data` ou `cloud-init`.

5. **Security Groups** : Ouvrir les ports nécessaires (22 pour SSH, 8000 et 8080 pour le worker) via les Security Groups Scaleway.

## Implémentation dans le Code

### Provider Trait Methods

```rust
/// Créer une instance avec image uniquement (Scaleway crée Block Storage automatiquement)
async fn create_instance(
    &self,
    zone: &str,
    name: &str,
    instance_type: &str,
    image_id: &str,
    // ... autres paramètres
) -> Result<String>;

/// Agrandir un Block Storage existant
async fn resize_block_storage(
    &self,
    zone: &str,
    volume_id: &str,
    new_size_gb: u64,
) -> Result<()>;

/// Démarrer l'instance
async fn start_instance(
    &self,
    zone: &str,
    server_id: &str,
) -> Result<()>;

/// Récupérer l'IP publique
async fn get_instance_ip(
    &self,
    zone: &str,
    server_id: &str,
) -> Result<String>;

/// Vérifier l'accessibilité SSH
async fn check_ssh_accessible(
    &self,
    ip: &str,
) -> Result<bool>;

/// Configurer les Security Groups (ouvrir ports SSH et worker)
async fn ensure_inbound_tcp_ports(
    &self,
    zone: &str,
    server_id: &str,
    ports: Vec<u16>,
) -> Result<bool>;
```

### Séquence dans `services.rs`

```rust
// 1. Créer instance avec image uniquement
let server_id = provider.create_instance(
    &zone,
    &instance_name,
    &instance_type,
    &image_id,
    // Pas de volumes !
).await?;

// 2. Récupérer le Block Storage créé automatiquement
let volumes = provider.list_attached_volumes(&zone, &server_id).await?;
let boot_volume_id = volumes.iter()
    .find(|v| v.volume_type == "sbs_volume")
    .map(|v| v.id.clone())
    .context("No Block Storage found")?;

// 3. Agrandir le Block Storage à 200GB via CLI
if let Some(current_size_gb) = get_volume_size_gb(&boot_volume_id) {
    if current_size_gb < 200 {
        provider.resize_block_storage(
            &zone,
            &boot_volume_id,
            200, // 200GB
        ).await?;
        // Attendre la fin de l'agrandissement
        wait_for_volume_resize(&zone, &boot_volume_id, 200).await?;
    }
}

// 4. Démarrer l'instance
provider.start_instance(&zone, &server_id).await?;

// 5. Attendre que l'instance soit running
wait_for_instance_state(&zone, &server_id, "running").await?;

// 6. Récupérer l'IP publique
let ip_address = provider.get_instance_ip(&zone, &server_id).await?;

// 7. Configurer Security Groups (SSH + worker ports)
provider.ensure_inbound_tcp_ports(
    &zone,
    &server_id,
    vec![22, 8000, 8080], // SSH, worker HTTP, worker metrics
).await?;

// 8. Attendre SSH accessible (max 3 minutes)
wait_for_ssh(&ip_address, Duration::from_secs(180)).await?;

// 9. Installation worker via SSH
install_worker_via_ssh(&ip_address).await?;

// 10. Health checks
perform_health_checks(&ip_address).await?;
```

## Progression (0-100%)

### Étapes de Progression pour Scaleway

```
0%   : REQUEST_CREATE (requête créée)
20%  : PROVIDER_CREATE (instance créée chez Scaleway)
25%  : PROVIDER_VOLUME_RESIZE (Block Storage agrandi à 200GB)
30%  : PROVIDER_START (instance démarrée)
40%  : PROVIDER_GET_IP (IP publique assignée)
45%  : PROVIDER_SECURITY_GROUP (ports ouverts)
50%  : WORKER_SSH_ACCESSIBLE (SSH accessible)
60%  : WORKER_SSH_INSTALL (Docker, dépendances, agent installé)
70%  : WORKER_VLLM_HTTP_OK (endpoint HTTP vLLM répond)
80%  : WORKER_MODEL_LOADED (modèle LLM chargé dans vLLM)
90%  : WORKER_VLLM_WARMUP (modèle préchauffé)
95%  : HEALTH_CHECK (endpoint health du worker confirme readiness)
100% : ready (VM pleinement opérationnelle)
```

## Validation

### Tests de Validation

Tous les tests suivants ont été validés avec succès :

- ✅ **Instance de type L4-1-24G** : Type correctement créé
- ✅ **SSH opérationnel** : Accessible après ~20 secondes
- ✅ **Instance accessible** : IP publique assignée et routable
- ✅ **Block Storage >= 150GB** : Volume de 200GB opérationnel

### Script de Test

Un script de test complet est disponible dans `test-scaleway/test_complete_validation.rs` :

```bash
cd test-scaleway
cargo run --bin test-complete
```

## Limitations et Contraintes

### Instance Types Supportés

- ✅ **L4-1-24G** : Validé et testé
- 🧪 **L40S** : À tester (devrait fonctionner avec la même séquence)
- 🧪 **H100** : À tester (devrait fonctionner avec la même séquence)

### Taille Minimum Block Storage

- **Minimum recommandé** : 150GB (pour Docker, vLLM, modèles LLM, logs)
- **Taille par défaut** : 200GB (configurable)

### Image Requise

- **Image ID** : `5c3d28db-33ce-4997-8572-f49506339283`
- **Nom** : Ubuntu Noble GPU OS 13 passthrough
- **Type root_volume** : `sbs_snapshot` (requis pour boot automatique)

## Dépannage

### Problèmes Courants

1. **SSH non accessible après 3 minutes**
   - Vérifier que les Security Groups ont bien les règles pour le port 22
   - Vérifier que l'instance est bien en état `running`
   - Vérifier que l'IP publique est correctement assignée

2. **Block Storage non agrandi**
   - Vérifier que le CLI Scaleway est installé et configuré
   - Vérifier les permissions (ACCESS_KEY, SECRET_KEY, ORGANIZATION_ID)
   - Vérifier que l'instance est arrêtée avant l'agrandissement

3. **Instance ne démarre pas**
   - Vérifier que le Block Storage contient bien le snapshot (bootable)
   - Vérifier qu'aucun volume local n'est attaché (contrainte L4-1-24G)

## Références

- [Scaleway Instance API](https://www.scaleway.com/en/developers/api/instances/)
- [Scaleway Block Storage API](https://www.scaleway.com/en/developers/api/block-storage/)
- [Scaleway CLI Documentation](https://www.scaleway.com/en/docs/developers/cli/)

