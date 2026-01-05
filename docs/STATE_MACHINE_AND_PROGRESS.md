# State Machine & Progress Tracking

## Vue d'ensemble

Le système utilise une **state machine explicite** pour gérer le cycle de vie des instances, avec un **système de progression granulaire** (0-100%) pour suivre l'avancement du provisionnement et du démarrage.

## State Machine

### États des instances

Les instances passent par les états suivants :

```
provisioning → booting → ready → draining → terminating → terminated → archived
```

**États d'erreur** :
- `provisioning_failed` : Échec lors de la création de l'instance chez le provider
- `startup_failed` : Échec lors du démarrage ou de la configuration du worker
- `failed` : État générique d'échec

### Transitions d'état

Toutes les transitions sont gérées par des fonctions explicites dans `inventiv-orchestrator/src/state_machine.rs` :

#### `booting_to_ready`
- **Condition** : Health check réussi (worker `/readyz` OK + modèle chargé)
- **Action** : Met à jour `status='ready'`, `ready_at=NOW()`, `last_health_check=NOW()`
- **Logging** : Crée une action `INSTANCE_READY` dans `action_logs`
- **Historique** : Enregistre la transition dans `instance_state_history`

#### `booting_to_startup_failed`
- **Condition** : Timeout de démarrage ou erreur critique détectée
- **Paramètres** : `error_code` (ex: `STARTUP_TIMEOUT`, `AGENT_CHECKSUM_FAILED`), `error_message`
- **Action** : Met à jour `status='startup_failed'`, `error_code`, `error_message`, `failed_at=NOW()`
- **Logging** : Crée une action `INSTANCE_STARTUP_FAILED` avec métadonnées d'erreur

#### `terminating_to_terminated`
- **Condition** : Confirmation de suppression chez le provider
- **Action** : Met à jour `status='terminated'`, `terminated_at=NOW()`
- **Logging** : Crée une action `INSTANCE_TERMINATED`

#### `mark_provider_deleted`
- **Condition** : Instance supprimée directement par le provider (orphan detection)
- **Action** : Transition `ready → terminated` avec `deleted_by_provider=TRUE`
- **Logging** : Crée une action `PROVIDER_DELETED_DETECTED`

### Historique des transitions

Toutes les transitions sont enregistrées dans `instance_state_history` :
- `instance_id` : UUID de l'instance
- `from_status` : État source
- `to_status` : État cible
- `reason` : Raison de la transition (ex: "Health check passed", "Startup timeout")
- `created_at` : Timestamp de la transition

## Progress Tracking (0-100%)

### Calcul de progression

Le système calcule automatiquement un **pourcentage de progression** (`progress_percent`) basé sur les actions complétées dans `action_logs`.

**Module** : `inventiv-api/src/progress.rs`

### Étapes de progression

#### Phase `provisioning` (0-25%)
- **5%** : `REQUEST_CREATE` complété (requête créée dans la DB)
- **20%** : `PROVIDER_CREATE` complété (instance créée chez le provider)
- **25%** : `PROVIDER_VOLUME_RESIZE` complété (Block Storage agrandi, si applicable - Scaleway uniquement)

#### Phase `booting` (25-100%)
- **25%** : `PROVIDER_CREATE` complété (début du booting)
- **30%** : `PROVIDER_START` complété (instance démarrée/powered on)
- **40%** : `PROVIDER_GET_IP` complété (adresse IP assignée)
- **45%** : `PROVIDER_SECURITY_GROUP` complété (ports ouverts, si applicable - Scaleway uniquement)
- **50%** : `WORKER_SSH_ACCESSIBLE` complété (SSH accessible sur port 22)
- **60%** : `WORKER_SSH_INSTALL` complété (Docker, dépendances, agent installé)
- **70%** : `WORKER_VLLM_HTTP_OK` complété (endpoint HTTP vLLM répond)
- **80%** : `WORKER_MODEL_LOADED` complété (modèle LLM chargé dans vLLM)
- **90%** : `WORKER_VLLM_WARMUP` complété (modèle préchauffé, prêt pour l'inférence)
- **95%** : `HEALTH_CHECK` success (endpoint health du worker confirme la readiness)
- **100%** : `ready` (VM pleinement opérationnelle)

### Séquence spécifique Scaleway

Pour les instances Scaleway GPU (L4-1-24G, L40S, H100), la séquence inclut des étapes supplémentaires :

1. **Création avec image uniquement** : Scaleway crée automatiquement un Block Storage de 20GB avec le snapshot de l'image (bootable)
2. **Agrandissement Block Storage** : Le volume est agrandi à 200GB via CLI avant le démarrage
3. **Configuration Security Groups** : Ouverture des ports SSH (22) et worker (8000, 8080)
4. **Vérification SSH** : Attente de l'accessibilité SSH (max 3 minutes, généralement ~20 secondes)

Voir [Scaleway Provisioning](SCALEWAY_PROVISIONING.md) pour les détails complets.

#### États terminaux
- **100%** : `ready`
- **0%** : `terminated`, `terminating`, `archived`
- **0%** : États d'échec (`provisioning_failed`, `startup_failed`, `failed`)

### Simulation pour Mock Provider

Pour les instances Mock, la progression est **simulée** basée sur le temps écoulé depuis la création :
- Progression linéaire dans le temps
- Permet de tester l'UI sans attendre les vraies actions

### Utilisation

#### API
```rust
// Calcul automatique lors de la récupération des instances
GET /instances
// Réponse inclut : "progress_percent": 50
```

#### Frontend
- Affichage dans une colonne dédiée dans la table des instances
- Barre de progression visuelle (optionnel)
- Mise à jour en temps réel via SSE

## Actions et Logging

### Action Types

Chaque étape du cycle de vie génère une action dans `action_logs` :

#### Provisioning
- `REQUEST_CREATE` : Requête de création reçue
- `EXECUTE_CREATE` : Début de l'exécution de la création
- `PROVIDER_CREATE` : Instance créée chez le provider
- `PERSIST_PROVIDER_ID` : ID provider persisté en DB
- `PROVIDER_CREATE_VOLUME` : Volume de données créé (si applicable)
- `PROVIDER_VOLUME_RESIZE` : Volume agrandi (Scaleway uniquement - Block Storage 20GB → 200GB)
- `PROVIDER_START` : Instance démarrée
- `PROVIDER_GET_IP` : Adresse IP récupérée
- `PROVIDER_SECURITY_GROUP` : Security Groups configurés (Scaleway uniquement - ports SSH et worker)
- `WORKER_SSH_ACCESSIBLE` : SSH accessible sur port 22
- `INSTANCE_CREATED` : Instance créée (transition vers `booting`)

#### Booting
- `WORKER_SSH_INSTALL` : Installation du worker via SSH
- `WORKER_VLLM_HTTP_OK` : vLLM HTTP endpoint répond
- `WORKER_MODEL_LOADED` : Modèle chargé dans vLLM
- `WORKER_VLLM_WARMUP` : Modèle préchauffé
- `HEALTH_CHECK` : Vérification de santé (succès/échec)
- `INSTANCE_READY` : Instance prête (transition vers `ready`)

#### Termination
- `REQUEST_TERMINATE` : Requête de terminaison reçue
- `PROVIDER_STOP` : Instance arrêtée chez le provider
- `PROVIDER_DELETE_VOLUME` : Volume supprimé
- `PROVIDER_DELETE` : Instance supprimée chez le provider
- `INSTANCE_TERMINATED` : Instance terminée

### Métadonnées des actions

Chaque action inclut des **métadonnées JSONB** (`action_logs.metadata`) :

```json
{
  "correlation_id": "uuid",
  "ip_address": "51.159.177.94",
  "server_id": "scaleway-instance-id",
  "zone": "fr-par-2",
  "phases": ["start", "docker_install", "vllm_start", "done"],
  "last_phase": "done",
  "ssh_exit_status": "exit status: 0",
  "agent_info": {
    "version": "1.0.0",
    "build_date": "2026-01-03",
    "checksum": "4f9441dc..."
  }
}
```

### Logging structuré

- **Component** : `api` ou `orchestrator`
- **Status** : `in_progress`, `success`, `failed`
- **Duration** : Durée en millisecondes
- **Error message** : Message d'erreur si échec
- **Metadata** : JSONB avec détails contextuels

## Health Check Flow

### Vérifications effectuées

Pour les instances `booting`, le système vérifie :

1. **SSH (port 22)** : Accessibilité de base
2. **Worker `/readyz`** : Endpoint de readiness du worker
3. **vLLM `/v1/models`** : Modèle chargé et visible
4. **Agent `/info`** : Version et checksum de l'agent (nouveau)
5. **Heartbeat** : Dernier heartbeat du worker (prioritaire)

### Priorisation

- **Heartbeat récent** (< 30s) : Fait confiance au heartbeat, skip les checks actifs
- **Pas de heartbeat** : Effectue les checks actifs (SSH, `/readyz`, `/v1/models`)

### Rate Limiting

- **Succès** : Log toutes les 5 minutes
- **Échec** : Log toutes les 1 minute
- **Premier check** : Toujours loggé

## Récupération automatique

Le système peut **récupérer automatiquement** certaines erreurs :

- **`STARTUP_TIMEOUT`** : Si un heartbeat arrive après le timeout, transition `startup_failed → booting`
- **`WAITING_FOR_WORKER_HEARTBEAT`** : Effacé si heartbeat reçu

## Code de référence

- **State machine** : `inventiv-orchestrator/src/state_machine.rs`
- **Progress calculation** : `inventiv-api/src/progress.rs`
- **Health check** : `inventiv-orchestrator/src/health_check_flow.rs`
- **Action logging** : `inventiv-orchestrator/src/logger.rs`

