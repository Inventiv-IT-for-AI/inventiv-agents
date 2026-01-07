# Analyse de Fiabilité des Workers et Gestion des Instances

**Date**: 2026-01-06  
**Objectif**: Identifier les points d'amélioration pour la fiabilisation de la gestion des Instances et des workers

---

## 📋 Résumé Exécutif

Cette analyse identifie les points critiques et propose des améliorations pour :
1. **Fiabilité des workers** : gestion des heartbeats, détection de défaillances, récupération automatique
2. **Gestion des instances** : transitions d'état robustes, détection d'instances bloquées, nettoyage automatique
3. **Observabilité** : logging structuré, métriques, alertes

---

## 🏗️ Architecture Actuelle (Résumé)

### Composants Principaux

```
┌─────────────┐
│   Frontend  │ (Next.js :3000)
└──────┬──────┘
       │ HTTP (session JWT)
       ▼
┌─────────────┐      ┌──────────────┐
│  inventiv-  │──────▶│    Redis     │ (Pub/Sub: CMD:*, EVT:*)
│    api      │      │  (Events)    │
│   (:8003)   │      └──────┬───────┘
└──────┬──────┘             │
       │                    │ Subscribe
       │ PostgreSQL         ▼
       │ (State)      ┌──────────────┐
       └──────────────▶│  inventiv-   │
                       │ orchestrator │ (Control Plane :8001)
                       │  (Jobs/State)│
                       └──────┬───────┘
                              │
                              │ Provider API
                              ▼
                    ┌─────────────────┐
                    │ Scaleway / Mock  │
                    │  (Instances GPU) │
                    └─────────┬─────────┘
                              │
                              │ Worker Agent
                              ▼
                    ┌─────────────────┐
                    │ inventiv-worker │
                    │ (vLLM + Agent)   │
                    └─────────────────┘
```

### Jobs Background (Orchestrator)

1. **job-health-check** (`health_check_job.rs`)
   - Intervalle: 10s
   - Traite: `booting`, `installing`, `starting` → `ready` / `startup_failed`
   - Utilise `SKIP LOCKED` pour éviter les conflits

2. **job-provisioning** (`provisioning_job.rs`)
   - Intervalle: 10s
   - Re-queue les instances `provisioning` bloquées (Redis Pub/Sub non durable)

3. **job-terminator** (`terminator_job.rs`)
   - Intervalle: 10s
   - Traite: `terminating` → `terminated`
   - Supprime les volumes (`delete_on_terminate=true`)

4. **job-watch-dog** (`watch_dog_job.rs`)
   - Intervalle: 10s
   - Détecte les instances `ready` supprimées chez le provider (orphan detection)

5. **job-recovery** (`recovery_job.rs`)
   - Intervalle: 30s
   - Récupère les instances bloquées dans divers états

### Bus d'Événements Redis

**Channels**:
- `orchestrator_events`: `CMD:*` (PROVISION, TERMINATE, SYNC_CATALOG, RECONCILE)
- `finops_events`: `EVT:*` (coûts, tokens)

**Garanties**: Non-durable Pub/Sub → requeue si orchestrator down

---

## 🔍 Points Critiques Identifiés

### 1. Gestion des Heartbeats Workers

#### État Actuel
- ✅ Heartbeats reçus via `/internal/worker/heartbeat` (proxy via API)
- ✅ Mise à jour `worker_last_heartbeat` dans DB
- ✅ Récupération automatique si `startup_failed` avec `STARTUP_TIMEOUT`
- ⚠️ Pas de détection explicite de workers "morts" (heartbeats arrêtés)

#### Problèmes Potentiels
1. **Workers silencieux** : Si un worker cesse d'envoyer des heartbeats mais reste `ready`, l'instance reste marquée `ready` indéfiniment
2. **Timeout heartbeat** : Pas de logique explicite pour marquer une instance comme "worker_dead" si `worker_last_heartbeat` > seuil
3. **Récupération partielle** : Le heartbeat peut récupérer `startup_failed` → `booting`, mais pas détecter les workers qui meurent après `ready`

#### Recommandations
- [ ] Ajouter un job `job-worker-watchdog` qui détecte les workers sans heartbeat récent (> 5 min) en état `ready`
- [ ] Transition automatique `ready` → `worker_dead` si heartbeat > seuil (configurable)
- [ ] Option de réinstallation automatique pour les workers morts

### 2. Health Checks et Readiness

#### État Actuel
- ✅ Health checks multiples : SSH (port 22), Worker `/readyz`, vLLM `/v1/models`
- ✅ Priorité aux heartbeats récents (< 30s) sur les checks actifs
- ✅ Support des états intermédiaires : `booting` → `installing` → `starting` → `ready`
- ⚠️ Timeouts fixes (2h pour `booting`, 30min pour model loading)

#### Problèmes Potentiels
1. **Timeouts trop longs** : 2h pour `booting` peut masquer des problèmes réels
2. **Health checks coûteux** : Appels SSH répétés peuvent être lents
3. **Pas de backoff exponentiel** : Health checks à intervalle fixe (10s) même pour instances problématiques

#### Recommandations
- [ ] Implémenter un backoff exponentiel pour les health checks échoués
- [ ] Réduire les timeouts par défaut (configurables via env vars)
- [ ] Ajouter des métriques de latence des health checks
- [ ] Cache des résultats de health checks (éviter appels répétés < 30s)

### 3. Gestion des Instances Bloquées

#### État Actuel
- ✅ `job-recovery` détecte les instances `booting` bloquées > 2h
- ✅ `job-provisioning` re-queue les instances `provisioning` bloquées
- ✅ `job-terminator` gère les instances `terminating` bloquées
- ⚠️ Pas de détection pour `installing` / `starting` bloquées

#### Problèmes Potentiels
1. **États intermédiaires** : `installing` et `starting` peuvent rester bloqués sans récupération
2. **Retry limits** : `retry_count < 5` dans `provisioning_job`, mais pas de limite globale
3. **Pas de notification** : Instances bloquées > seuil ne génèrent pas d'alertes

#### Recommandations
- [ ] Étendre `job-recovery` pour détecter `installing` / `starting` bloquées
- [ ] Ajouter un système d'alertes (logs structurés + métriques) pour instances bloquées
- [ ] Implémenter un circuit breaker pour instances avec trop d'échecs

### 4. Gestion des Volumes

#### État Actuel
- ✅ Découverte automatique des volumes attachés (`list_attached_volumes`)
- ✅ Tracking dans `instance_volumes` avec `delete_on_terminate`
- ✅ Suppression automatique lors de la terminaison
- ⚠️ Pas de nettoyage périodique des volumes orphelins

#### Problèmes Potentiels
1. **Volumes orphelins** : Si une instance est supprimée manuellement chez le provider, les volumes peuvent rester
2. **Échecs de suppression** : Pas de retry automatique si suppression échoue
3. **Pas de réconciliation** : Pas de job dédié pour réconcilier les volumes DB vs provider

#### Recommandations
- [ ] Ajouter un job `job-volume-reconciliation` pour détecter les volumes orphelins
- [ ] Retry automatique avec backoff pour les suppressions échouées
- [ ] Alertes pour volumes non supprimés après terminaison

### 5. Observabilité et Logging

#### État Actuel
- ✅ Logging structuré dans `action_logs`
- ✅ Worker event logging (`/logs` endpoint)
- ✅ Métriques worker (GPU, queue depth, etc.)
- ⚠️ Pas de métriques Prometheus pour les jobs
- ⚠️ Pas de traces distribuées (correlation_id partiellement implémenté)

#### Problèmes Potentiels
1. **Métriques manquantes** : Pas de métriques pour latence des jobs, taux d'échec, etc.
2. **Corrélation limitée** : `correlation_id` présent mais pas utilisé partout
3. **Pas d'alertes** : Pas de système d'alertes pour incidents critiques

#### Recommandations
- [ ] Exposer des métriques Prometheus pour tous les jobs
- [ ] Implémenter un système d'alertes basé sur les métriques
- [ ] Étendre l'utilisation de `correlation_id` pour le tracing end-to-end

---

## 🎯 Plan d'Action Priorisé

### Phase 1 : Améliorations Critiques (1-2 semaines)

#### 1.1 Détection des Workers Morts
- [ ] Créer `job-worker-watchdog.rs` pour détecter workers sans heartbeat récent
- [ ] Ajouter transition `ready` → `worker_dead` si heartbeat > 5 min
- [ ] Tests unitaires et E2E

#### 1.2 Amélioration des Health Checks
- [ ] Implémenter backoff exponentiel pour health checks échoués
- [ ] Réduire timeouts par défaut (configurables)
- [ ] Ajouter cache des résultats (< 30s)

#### 1.3 Extension du Job Recovery
- [ ] Détecter `installing` / `starting` bloquées > seuil
- [ ] Ajouter alertes (logs structurés) pour instances bloquées

### Phase 2 : Améliorations Importantes (2-4 semaines)

#### 2.1 Réconciliation des Volumes
- [ ] Créer `job-volume-reconciliation.rs`
- [ ] Détecter volumes orphelins (DB vs provider)
- [ ] Retry automatique avec backoff pour suppressions échouées

#### 2.2 Métriques et Observabilité
- [ ] Exposer métriques Prometheus pour tous les jobs
- [ ] Ajouter métriques de latence et taux d'échec
- [ ] Dashboard Grafana (optionnel)

#### 2.3 Circuit Breaker
- [ ] Implémenter circuit breaker pour instances avec trop d'échecs
- [ ] Configurer seuils (ex: 5 échecs consécutifs → circuit ouvert)

### Phase 3 : Améliorations Optionnelles (1-2 mois)

#### 3.1 Système d'Alertes
- [ ] Intégration avec système d'alertes (ex: Alertmanager)
- [ ] Alertes pour incidents critiques (instances bloquées, workers morts)

#### 3.2 Tracing Distribué
- [ ] Étendre utilisation de `correlation_id` partout
- [ ] Intégration OpenTelemetry (optionnel)

---

## 📊 Métriques Clés à Surveiller

### Workers
- Taux de heartbeats reçus / attendus
- Latence des heartbeats (p50, p95, p99)
- Nombre de workers morts détectés
- Temps moyen de récupération après défaillance

### Instances
- Temps moyen de transition `provisioning` → `ready`
- Taux d'échec par état (`provisioning_failed`, `startup_failed`)
- Nombre d'instances bloquées par état
- Temps moyen de terminaison

### Jobs
- Latence d'exécution par job (p50, p95, p99)
- Taux d'erreur par job
- Nombre d'instances traitées par cycle

### Volumes
- Nombre de volumes orphelins
- Taux de succès de suppression
- Temps moyen de suppression

---

## 🔧 Configuration Recommandée

### Variables d'Environnement à Ajouter

```bash
# Worker Watchdog
WORKER_HEARTBEAT_TIMEOUT_SECONDS=300  # 5 minutes
WORKER_DEAD_RECOVERY_ENABLED=true

# Health Checks
HEALTH_CHECK_BACKOFF_ENABLED=true
HEALTH_CHECK_BACKOFF_MAX_INTERVAL_SECONDS=300
HEALTH_CHECK_CACHE_TTL_SECONDS=30

# Recovery
RECOVERY_INSTALLING_TIMEOUT_SECONDS=3600  # 1 hour
RECOVERY_STARTING_TIMEOUT_SECONDS=1800     # 30 minutes
RECOVERY_BOOTING_TIMEOUT_SECONDS=7200      # 2 hours

# Volume Reconciliation
VOLUME_RECONCILIATION_ENABLED=true
VOLUME_DELETE_RETRY_MAX_ATTEMPTS=5
VOLUME_DELETE_RETRY_BACKOFF_SECONDS=60

# Circuit Breaker
CIRCUIT_BREAKER_ENABLED=true
CIRCUIT_BREAKER_FAILURE_THRESHOLD=5
CIRCUIT_BREAKER_RESET_TIMEOUT_SECONDS=300
```

---

## 📝 Notes de Conception

### Principe de Récupération
- **Idempotence** : Tous les jobs doivent être idempotents (réexécution sûre)
- **SKIP LOCKED** : Utiliser `FOR UPDATE SKIP LOCKED` pour éviter les conflits entre orchestrators multiples
- **Backoff exponentiel** : Éviter les appels répétés coûteux
- **Graceful degradation** : Continuer à fonctionner même si certains composants échouent

### Gestion des Erreurs
- **Logging structuré** : Tous les événements critiques doivent être loggés dans `action_logs`
- **Métadonnées** : Inclure `correlation_id`, `retry_count`, `error_code` dans les logs
- **Retry intelligent** : Distinguer erreurs temporaires (retry) vs permanentes (fail fast)

---

## 🧪 Tests Recommandés

### Tests Unitaires
- [ ] Tests pour `job-worker-watchdog` (détection workers morts)
- [ ] Tests pour backoff exponentiel
- [ ] Tests pour circuit breaker

### Tests d'Intégration
- [ ] Test E2E : Worker meurt après `ready` → détection → récupération
- [ ] Test E2E : Instance bloquée dans `installing` → récupération
- [ ] Test E2E : Volume orphelin → réconciliation → suppression

### Tests de Charge
- [ ] Test avec 100+ instances simultanées
- [ ] Test avec orchestrator redémarrage (requeue)
- [ ] Test avec provider API lent/intermittent

---

## 📚 Références

- [Architecture](architecture.md)
- [State Machine & Progress](STATE_MACHINE_AND_PROGRESS.md)
- [Worker & Router Phase 0.2](worker_and_router_phase_0_2.md)
- [Agent Version Management](AGENT_VERSION_MANAGEMENT.md)
- [Storage Management](STORAGE_MANAGEMENT.md)

---

## ✅ Checklist de Validation

Avant de considérer les améliorations comme complètes :

- [ ] Tous les jobs ont des métriques Prometheus
- [ ] Tous les timeouts sont configurables via env vars
- [ ] Tous les jobs utilisent `SKIP LOCKED` pour éviter conflits
- [ ] Tous les événements critiques sont loggés dans `action_logs`
- [ ] Tests unitaires et d'intégration passent
- [ ] Documentation mise à jour
- [ ] Migration DB si nécessaire

