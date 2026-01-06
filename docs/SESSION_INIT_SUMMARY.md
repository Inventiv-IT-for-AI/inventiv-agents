# Résumé de Session d'Initialisation - Workers Debug & Optimisation

**Date**: 2026-01-06  
**Objectif**: Comprendre l'infrastructure LLM (control-plane/data-plane) et identifier les points d'amélioration

---

## ✅ Travail Effectué

### 1. Lecture de la Documentation
- ✅ README.md, TODO.md
- ✅ docs/architecture.md, domain_design.md, specification_generale.md
- ✅ docs/API_URL_CONFIGURATION.md, worker_and_router_phase_0_2.md

### 2. Exploration du Code
- ✅ inventiv-api/src/main.rs (API endpoints, auth, OpenAI proxy)
- ✅ inventiv-orchestrator/src/main.rs (jobs, event listener, worker endpoints)
- ✅ inventiv-worker/agent.py (worker agent, heartbeats, health checks)
- ✅ inventiv-common/ (types partagés, bus d'événements)

### 3. Analyse de la Base de Données
- ✅ sqlx-migrations/ (structure des tables)
- ✅ seeds/catalog_seeds.sql (catalogue providers/zones/types)

### 4. Compréhension des Jobs Background
- ✅ job-health-check (booting/installing/starting → ready)
- ✅ job-provisioning (requeue stuck provisioning)
- ✅ job-terminator (terminating → terminated)
- ✅ job-watch-dog (orphan detection)
- ✅ job-recovery (recover stuck instances)

### 5. Analyse du Bus d'Événements Redis
- ✅ Channels: `orchestrator_events` (CMD:*), `finops_events` (EVT:*)
- ✅ Commands: PROVISION, TERMINATE, SYNC_CATALOG, RECONCILE
- ✅ Pattern: Non-durable Pub/Sub → requeue via jobs

### 6. Documentation Créée
- ✅ `docs/WORKER_RELIABILITY_ANALYSIS.md` - Analyse détaillée des points critiques
- ✅ `docs/FLUX_ARCHITECTURE_MAP.md` - Carte mentale des flux
- ✅ `docs/SESSION_INIT_SUMMARY.md` - Ce document

---

## 🔍 Points Critiques Identifiés

### 1. Gestion des Workers Morts
**Problème**: Pas de détection explicite des workers qui cessent d'envoyer des heartbeats après être passés en `ready`.

**Impact**: Instances marquées `ready` mais workers morts → trafic routé vers instances non fonctionnelles.

**Solution Recommandée**: 
- Créer `job-worker-watchdog.rs` pour détecter workers sans heartbeat récent (> 5 min)
- Transition automatique `ready` → `worker_dead` si heartbeat > seuil

### 2. Health Checks et Timeouts
**Problème**: 
- Timeouts fixes (2h pour `booting`, 30min pour model loading)
- Pas de backoff exponentiel pour health checks échoués
- Health checks répétés même pour instances problématiques

**Impact**: Détection tardive des problèmes, surcharge inutile du système.

**Solution Recommandée**:
- Implémenter backoff exponentiel pour health checks échoués
- Réduire timeouts par défaut (configurables via env vars)
- Cache des résultats de health checks (< 30s)

### 3. Instances Bloquées dans États Intermédiaires
**Problème**: `job-recovery` détecte seulement `booting` bloquées, pas `installing` / `starting`.

**Impact**: Instances peuvent rester bloquées indéfiniment dans ces états.

**Solution Recommandée**:
- Étendre `job-recovery` pour détecter `installing` / `starting` bloquées
- Ajouter alertes (logs structurés) pour instances bloquées

### 4. Réconciliation des Volumes
**Problème**: Pas de job dédié pour réconcilier les volumes DB vs provider.

**Impact**: Volumes orphelins peuvent rester non supprimés.

**Solution Recommandée**:
- Créer `job-volume-reconciliation.rs`
- Détecter volumes orphelins (DB vs provider)
- Retry automatique avec backoff pour suppressions échouées

### 5. Observabilité et Métriques
**Problème**: 
- Pas de métriques Prometheus pour les jobs
- `correlation_id` partiellement implémenté
- Pas de système d'alertes

**Impact**: Difficulté à diagnostiquer les problèmes, pas de visibilité sur les performances.

**Solution Recommandée**:
- Exposer métriques Prometheus pour tous les jobs
- Étendre utilisation de `correlation_id` partout
- Implémenter système d'alertes basé sur métriques

---

## 📊 Architecture Comprise

### Flux Principal
1. **UI → API**: Requêtes HTTP avec session JWT
2. **API → Redis**: Publication de commandes `CMD:*` dans `orchestrator_events`
3. **Redis → Orchestrator**: Event listener subscribe et spawn handlers
4. **Orchestrator → Provider**: Appels API pour provisioning/termination
5. **Provider → VM**: Création/suppression d'instances
6. **VM → Worker**: Agent Python déployé via SSH bootstrap
7. **Worker → Orchestrator**: Heartbeats via `/internal/worker/heartbeat` (proxy API)

### Jobs Background
- **job-health-check**: Transition `booting/installing/starting` → `ready`
- **job-provisioning**: Re-queue instances `provisioning` bloquées
- **job-terminator**: Traitement instances `terminating` → `terminated`
- **job-watch-dog**: Détection instances `ready` supprimées chez provider
- **job-recovery**: Récupération instances bloquées

### State Machine
```
provisioning → booting → installing → starting → ready
                                    ↓
                            startup_failed
                                    ↓
                            terminating → terminated → archived
```

---

## 🎯 Plan d'Action Priorisé

### Phase 1 : Améliorations Critiques (1-2 semaines)

#### 1.1 Détection des Workers Morts
- [ ] Créer `job-worker-watchdog.rs`
- [ ] Détecter workers sans heartbeat récent (> 5 min)
- [ ] Transition `ready` → `worker_dead`
- [ ] Tests unitaires et E2E

#### 1.2 Amélioration des Health Checks
- [ ] Implémenter backoff exponentiel
- [ ] Réduire timeouts par défaut (configurables)
- [ ] Ajouter cache des résultats (< 30s)

#### 1.3 Extension du Job Recovery
- [ ] Détecter `installing` / `starting` bloquées
- [ ] Ajouter alertes (logs structurés)

### Phase 2 : Améliorations Importantes (2-4 semaines)

#### 2.1 Réconciliation des Volumes
- [ ] Créer `job-volume-reconciliation.rs`
- [ ] Détecter volumes orphelins
- [ ] Retry automatique avec backoff

#### 2.2 Métriques et Observabilité
- [ ] Exposer métriques Prometheus pour tous les jobs
- [ ] Dashboard Grafana (optionnel)

#### 2.3 Circuit Breaker
- [ ] Implémenter circuit breaker pour instances avec trop d'échecs

### Phase 3 : Améliorations Optionnelles (1-2 mois)

#### 3.1 Système d'Alertes
- [ ] Intégration avec système d'alertes (ex: Alertmanager)

#### 3.2 Tracing Distribué
- [ ] Étendre utilisation de `correlation_id` partout
- [ ] Intégration OpenTelemetry (optionnel)

---

## 🔧 Incohérences et Divergences Identifiées

### 1. Documentation vs Code
- ✅ **État**: La documentation est globalement à jour
- ⚠️ **Note**: Certains documents mentionnent des fonctionnalités "à venir" qui sont déjà implémentées (ex: progress tracking, agent version management)

### 2. Timeouts et Configuration
- ⚠️ **Problème**: Timeouts hardcodés dans le code (2h, 30min) non configurables
- ✅ **Recommandation**: Ajouter variables d'environnement pour tous les timeouts

### 3. SSE Implementation
- ⚠️ **Note**: SSE basé sur polling DB (pas event-sourced) - mentionné dans TODO.md comme dette technique
- ✅ **Recommandation**: Améliorer via NOTIFY/LISTEN PostgreSQL ou Redis streams

### 4. Mock Provider Routing
- ⚠️ **Note**: Test E2E override `instances.ip_address` vers `mock-vllm` (hack local)
- ✅ **Recommandation**: Remplacer par mécanisme propre (voir backlog)

---

## 📚 Points d'Extension Identifiés

### 1. Nouveaux Providers
**Fichier**: `inventiv-providers/src/{provider}.rs`  
**Trait**: `CloudProvider`  
**Registration**: `provider_manager.rs` → `ProviderManager::get_provider()`

### 2. Nouveaux Jobs Background
**Pattern**:
1. Créer `{job_name}_job.rs` dans `inventiv-orchestrator/src/`
2. Fonction `pub async fn run(pool, redis_client)`
3. Loop avec `tokio::time::interval()`
4. Utiliser `FOR UPDATE SKIP LOCKED`
5. Spawn dans `main.rs`

### 3. Nouveaux Événements Redis
**Channel**: `orchestrator_events` ou `finops_events`  
**Format**: `{"type": "CMD:NEW_COMMAND", ...}`  
**Handler**: Ajouter dans `main.rs` → Event Listener

### 4. Nouveaux Endpoints API
**Fichier**: `inventiv-api/src/main.rs` ou module dédié  
**Pattern**: Route → Handler → Auth middleware → Swagger docs

### 5. Nouveaux États de State Machine
**Fichier**: `inventiv-orchestrator/src/state_machine.rs`  
**Pattern**: Fonction `{from}_to_{to}()` → UPDATE → INSERT history → Log

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

---

## 📖 Documents de Référence

### Créés lors de cette session
- `docs/WORKER_RELIABILITY_ANALYSIS.md` - Analyse détaillée des points critiques
- `docs/FLUX_ARCHITECTURE_MAP.md` - Carte mentale des flux
- `docs/SESSION_INIT_SUMMARY.md` - Ce document

### Documents existants pertinents
- `docs/architecture.md` - Architecture générale
- `docs/domain_design.md` - Design du domaine
- `docs/specification_generale.md` - Spécifications générales
- `docs/STATE_MACHINE_AND_PROGRESS.md` - State machine et progress tracking
- `docs/AGENT_VERSION_MANAGEMENT.md` - Gestion de version de l'agent
- `docs/STORAGE_MANAGEMENT.md` - Gestion du stockage
- `docs/worker_and_router_phase_0_2.md` - Worker et router phase 0.2

---

## 🎓 Apprentissages Clés

### Architecture
- **Séparation CQRS**: API (Product Plane) vs Orchestrator (Control Plane)
- **Event-Driven**: Redis Pub/Sub pour communication asynchrone
- **Jobs Background**: Pattern `SKIP LOCKED` pour éviter conflits

### Fiabilité
- **Idempotence**: Tous les jobs doivent être idempotents
- **Requeue**: Redis Pub/Sub non durable → requeue via jobs
- **Health Checks**: Multiples méthodes (SSH, Worker `/readyz`, vLLM `/v1/models`)

### Observabilité
- **Logging structuré**: `action_logs` pour tous les événements critiques
- **Worker events**: Logs structurés sur worker (`/logs` endpoint)
- **Métriques**: Worker expose Prometheus metrics (`/metrics`)

### Extensibilité
- **Providers modulaires**: Trait `CloudProvider` pour nouveaux providers
- **Jobs extensibles**: Pattern clair pour ajouter nouveaux jobs
- **State machine**: Transitions explicites et historisées

---

## 🚀 Prochaines Étapes Recommandées

1. **Implémenter Phase 1** (détection workers morts, amélioration health checks)
2. **Tests E2E** pour valider les améliorations
3. **Métriques Prometheus** pour monitoring
4. **Documentation** mise à jour avec nouvelles fonctionnalités
5. **Review** du code avec l'équipe

---

## 📝 Notes Finales

Le système est bien architecturé avec une séparation claire des responsabilités. Les principaux points d'amélioration concernent :
1. La détection proactive des défaillances (workers morts, instances bloquées)
2. L'observabilité (métriques, alertes)
3. La configuration (timeouts configurables)

Les améliorations proposées sont alignées avec les principes existants (idempotence, SKIP LOCKED, logging structuré) et peuvent être implémentées progressivement sans casser l'existant.

