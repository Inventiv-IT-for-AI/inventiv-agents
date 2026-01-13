# Instance Export Enhanced - Status & Progress Tracking

## Vue d'ensemble

L'endpoint `/instances/{id}/export` a été créé pour fournir une vue complète et enrichie de la séquence de provisioning/terminaison d'une instance, incluant :

- **Status et progression** pour chaque action
- **Validation des transitions** de state machine
- **Tracking des retries** et des durées par phase
- **Analyse de cohérence** avec la state machine

## Endpoint

```
GET /instances/{id}/export
```

### Réponse

```json
{
  "instance": { ... },
  "storages": [ ... ],
  "actions": [
    {
      "id": "uuid",
      "action_type": "PROVIDER_CREATE",
      "component": "orchestrator",
      "status": "success",
      "progress_percent": 20,
      "phase": "provisioning",
      "sub_phase": null,
      "retry_count": 0,
      "is_state_transition": true,
      "is_valid_transition": true,
      "elapsed_seconds_since_start": 5,
      "elapsed_seconds_since_start_completed": 7,
      ...
    }
  ],
  "state_transitions": [
    {
      "id": "uuid",
      "from_status": "provisioning",
      "to_status": "booting",
      "reason": "Instance created",
      "elapsed_seconds_since_start": 7
    }
  ],
  "summary": {
    "total_actions": 51,
    "total_state_transitions": 5,
    "phases": [
      {
        "phase": "provisioning",
        "start_time": "2026-01-10T11:03:39Z",
        "end_time": "2026-01-10T11:04:56Z",
        "duration_seconds": 77,
        "action_count": 8,
        "success_count": 7,
        "failed_count": 0,
        "progress_start": 5,
        "progress_end": 25
      }
    ],
    "total_duration_seconds": 7200,
    "invalid_transitions": [],
    "retry_count": 2,
    "exported_at": "2026-01-10T12:56:46Z"
  }
}
```

## Champs enrichis des actions

### `progress_percent` (0-100)

Pourcentage de progression calculé pour chaque action basé sur son type et sa position dans le cycle de vie :

- **Provisioning (0-25%)** :
  - `REQUEST_CREATE`: 5%
  - `PROVIDER_CREATE`: 20%
  - `PROVIDER_VOLUME_RESIZE`: 25%

- **Booting (25-50%)** :
  - `PROVIDER_START`: 30%
  - `PROVIDER_GET_IP`: 40%
  - `PROVIDER_SECURITY_GROUP`: 45%
  - `WORKER_SSH_ACCESSIBLE`: 50%

- **Installing (50-60%)** :
  - `WORKER_SSH_INSTALL`: 60% (success) / 55% (in progress)

- **Starting (60-95%)** :
  - `WORKER_VLLM_HTTP_OK`: 70%
  - `WORKER_MODEL_LOADED`: 80%
  - `WORKER_VLLM_WARMUP`: 90%
  - `HEALTH_CHECK`: 95% (success) / 90% (waiting)

- **Ready (100%)** :
  - `INSTANCE_READY`: 100%

- **Termination** : 0% (instance en cours de destruction)

### `phase`

Phase principale à laquelle appartient l'action :
- `provisioning` : Création de l'instance chez le provider (0-25%)
- `booting` : Démarrage de l'instance, SSH devient accessible (25-50%)
- `installing` : Installation de Docker et du modèle via SSH (50-60%)
- `starting` : Démarrage des conteneurs, chargement du modèle, warmup, health checks (60-95%)
- `ready` : Instance pleinement opérationnelle (100%)
- `draining` : Instance en cours de vidage
- `terminating` : Instance en cours de suppression
- `terminated` : Instance supprimée

### `sub_phase`

Sous-phase extraite des métadonnées (ex: `docker_install`, `vllm_start`, `done`).

### `retry_count`

Nombre de tentatives précédentes pour le même type d'action (0 = première tentative).

### `is_state_transition`

Indique si cette action a provoqué une transition d'état de l'instance.

### `is_valid_transition`

Valide si la transition respecte la state machine :
- `true` : transition valide
- `false` : transition invalide (erreur de cohérence)
- `null` : pas de transition d'état

### `elapsed_seconds_since_start`

Temps écoulé depuis la création de l'instance au moment où l'action a commencé.

### `elapsed_seconds_since_start_completed`

Temps écoulé depuis la création de l'instance au moment où l'action s'est terminée.

## State Machine Validation

### Transitions valides

```
provisioning → booting, provisioning_failed
booting → installing, starting, ready, startup_failed, unavailable
installing → starting, ready, startup_failed
starting → ready, startup_failed, unavailable
ready → draining, terminating, unavailable, terminated
draining → terminating
terminating → terminated
terminated → archived
unavailable → ready, terminating, terminated
startup_failed → booting, terminating, terminated
```

### Détection des transitions invalides

Le champ `summary.invalid_transitions` liste toutes les transitions invalides détectées :

```json
{
  "invalid_transitions": [
    "PROVIDER_TERMINATE: ready -> booting"
  ]
}
```

## Analyse des phases

Le `summary.phases` fournit un résumé pour chaque phase :

- **Durée** : temps total passé dans la phase
- **Nombre d'actions** : total, succès, échecs
- **Progression** : progression au début et à la fin de la phase

## Cas d'usage

### 1. Analyse de performance

Identifier les phases les plus longues :

```javascript
const longestPhase = export.summary.phases
  .sort((a, b) => (b.duration_seconds || 0) - (a.duration_seconds || 0))[0];
```

### 2. Détection de problèmes

Identifier les retries et transitions invalides :

```javascript
const retries = export.actions.filter(a => a.retry_count > 0);
const invalidTransitions = export.summary.invalid_transitions;
```

### 3. Visualisation de progression

Utiliser `progress_percent` pour créer une timeline visuelle :

```javascript
export.actions.forEach(action => {
  console.log(`${action.action_type}: ${action.progress_percent}%`);
});
```

### 4. Analyse de cohérence

Vérifier la cohérence avec la state machine :

```javascript
const invalid = export.actions.filter(a => a.is_valid_transition === false);
if (invalid.length > 0) {
  console.error('Transitions invalides détectées:', invalid);
}
```

## Améliorations futures

1. **Calcul de progression historique** : Calculer la progression exacte au moment où chaque action s'est terminée (nécessite un historique des états)
2. **Détection de patterns** : Identifier les patterns d'erreurs récurrents
3. **Prédiction de durée** : Estimer la durée restante basée sur les phases précédentes
4. **Alertes automatiques** : Alerter sur les transitions invalides ou retries excessifs

## Exemple d'analyse de la séquence fournie

### Observations

1. **Provisioning réussi** : 77 secondes (11:03:39 → 11:04:56)
2. **Booting réussi** : ~4 minutes 30 secondes (11:04:56 → 11:09:28)
3. **Termination avec retry** :
   - Première tentative `PROVIDER_TERMINATE` échoue (instance en état "stopping")
   - Recovery déclenché automatiquement
   - Deuxième tentative réussit après 32 secondes

### Points d'amélioration détectés

1. **Retry automatique** : Le système gère correctement les échecs de terminaison
2. **Volume cleanup** : Le volume est nettoyé même si la terminaison échoue initialement
3. **Cohérence** : Toutes les transitions sont valides selon la state machine

## Code de référence

- **Handler** : `inventiv-api/src/handlers/instance_export.rs`
- **Progress calculation** : `inventiv-api/src/progress.rs`
- **State machine** : `inventiv-orchestrator/src/state_machine.rs`
