# Roadmap & TODOs (État Réel + Prochaines Étapes)

Ce fichier reflète l’état **réel** du repo (code + migrations + UI) et les chantiers prioritaires.

---

## ✅ Réalisé (fonctionnel / implémenté)

### Event-driven backbone
- [x] **Redis Pub/Sub**: `inventiv-api` publie `CMD:*` sur `orchestrator_events`.
- [x] **Orchestrator subscriber**: consomme `CMD:PROVISION`, `CMD:TERMINATE`, `CMD:SYNC_CATALOG`, `CMD:RECONCILE`.

### API (inventiv-api :8003)
- [x] `POST /deployments` → publie `CMD:PROVISION`.
- [x] `GET /instances` (+ filtre `archived`), `DELETE /instances/:id` (status `terminating` + event), `PUT /instances/:id/archive`.
- [x] **Settings API**: `GET/PUT` providers/regions/zones/instance_types.
- [x] **Zone ↔ InstanceType**:
  - [x] `GET /instance_types/:id/zones`
  - [x] `PUT /instance_types/:id/zones` (remplacement complet)
  - [x] `GET /zones/:zone_id/instance_types` (filtrage pour l’UI)
- [x] **Action logs**: `GET /action_logs` (filtrage, limit).
- [x] Swagger UI: `/swagger-ui` + spec `/api-docs/openapi.json`.

### Orchestrator (inventiv-orchestrator :8001)
- [x] **Provisioning Scaleway** (réel): `create_instance` + `poweron` + récupération IP → DB `booting`.
- [x] **Health check loop**: transition `booting` → `ready` (check SSH:22).
- [x] **Termination** (réel): appel provider + DB `terminated`.
- [x] **Reconciliation watchdog**: détection “deleted by provider”, retry termination.
- [x] **Catalog sync** (Scaleway): fetch API products → upsert `instance_types`.

### Frontend (inventiv-frontend)
- [x] UI Dashboard/Instances/Settings/Monitoring/Traces.
- [x] API base URL via `NEXT_PUBLIC_API_URL` + `apiUrl()` (centralisé).
- [x] Filtrage: zones par région + types par zone dans le flow de création.

---

## 🐛 Bugs connus / incohérences (à corriger en priorité)

### DB migrations / seeds
- [x] **Single source of truth migrations**:
  - `sqlx-migrations/` = migrations exécutées au boot (API + orchestrator)
  - `migrations/` = seeds uniquement (`seeds*.sql`)
- [ ] **Seeds non exécutés automatiquement**: il faut un mécanisme clair (script, make target, doc) pour initialiser providers/regions/zones/types/associations en dev.

### Contrats API/UI à surveiller
- [ ] `instance_type_zones` existait dans la doc mais pas en SQL au départ → maintenant ajouté; vérifier que l’UI Settings alimente correctement cette table.
- [ ] `action_logs`:
  - [ ] schéma initial incomplet (pas de `metadata`, component check trop strict) → corrigé via migration dédiée; vérifier en DB.
  - [x] endpoint de recherche paginée + stats pour UI virtualisée: `GET /action_logs/search`
  - [x] table `action_types` (catalogue UI): `GET /action_types`

### Docs / scripts obsolètes
- [ ] **Router**: le crate `inventiv-router` a été supprimé mais la doc/README/scripts en parlent encore (port 8002, `/v1/chat/completions`).
- [ ] `scripts/test_architecture.sh` attend `/health` backend/router (à aligner avec la réalité ou ré-implémenter).

---

## 🎯 Objectif court terme (priorité produit): Provisioning Scaleway réel via UI (E2E)

### Pré-requis Scaleway
- [x] Documenter clairement les variables requises:
  - `SCALEWAY_PROJECT_ID`
  - `SCALEWAY_SECRET_KEY`
  - (optionnel/à trancher) `SCALEWAY_ACCESS_KEY`
- [ ] Assurer qu’un **catalogue minimal** est présent (zones + instance types + associations zone↔type) pour que l’UI propose des choix valides.

### E2E flow à valider
- [x] UI → `POST /deployments`
- [x] API → Redis `CMD:PROVISION`
- [x] Orchestrator → Scaleway `create_instance` + DB `booting` + IP
- [x] Health check → DB `ready`
- [x] UI: rafraîchissement/polling → instance visible et statuts corrects

---

## 🚧 Ce qui manque encore (produit & plateforme)

## 🧭 Phase 0.2.0 — Workers + Data plane (objectif)

### Worker (vLLM + agent sidecar)
- [ ] Définir un **contrat minimal** Worker:
  - `/healthz` (liveness)
  - `/readyz` (readiness: modèle chargé / vLLM prêt)
  - `/metrics` (prometheus)
- [ ] Clarifier le **protocole d’enrôlement** (worker → API/orchestrator):
  - registration (instance_id, ip, model, gpu specs)
  - heartbeat (status, queue depth, gpu utilization)
- [ ] Décider comment le worker est démarré:
  - cloud-init + systemd
  - docker run via SSH (provisoire)
  - k3s / nomad (plus tard)

### Routing / Load Balancing (data plane)
- [ ] Réintroduire un **router** (OpenAI-compatible):
  - `POST /v1/chat/completions` (proxy vers workers)
  - auth API keys + rate limiting
  - load balancing (LOR / queue depth)
  - failover (retry + circuit breaker)
- [ ] Source of truth routing:
  - Redis (pub/sub + cache)
  - ou DB + watcher

### Observabilité / Scalabilité
- [ ] Exposer `metrics` sur API/orchestrator/worker/router
- [ ] Ajout d’un scaler loop basé sur:
  - `queue_depth`
  - `ttft / p95 latency`
  - `gpu_util`

### Auth / API Keys
- [ ] Auth (JWT) + gestion des API keys (backend + router/gateway).
- [ ] RBAC minimal (admin) + stockage sécurisé (hash/rotation).

### Worker agent
- [ ] `inventiv-worker/agent.py`: implémenter heartbeat/metrics + protocole d’enrôlement.
- [ ] Readiness réelle (pas juste SSH:22): health endpoint du worker/vLLM.

### Router / Data plane (à trancher)
- [ ] Décision: **réintroduire un Router** (OpenAI-compatible) OU supprimer la mention du router de la doc/scripts tant qu’il n’existe pas.
- [ ] Si router: validation API keys, routing dynamique (Redis), failover, rate limiting.

### Observabilité
- [ ] `/metrics` Prometheus sur chaque service + dashboards.
- [ ] Traces distribuées (optionnel).

---

## ✅ Recommandations (direction / “bonne trajectoire”)

- [ ] **Single source of truth DB**: choisir un workflow unique migrations + seeds (idéalement `sqlx-migrations/` pour les migrations, et un script explicite pour les seeds).
- [ ] **Stabiliser les contrats**: documenter (OpenAPI) et faire matcher l’UI strictement.
- [ ] **Aligner la doc**: README + `docs/architecture.md` + scripts, notamment sur le router.
- [ ] **Durcir le provisioning**: gestion d’erreurs, retries, timeouts, et logs exploitables (action_logs + metadata).
