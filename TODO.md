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
- [x] **Auth User (session)**:
  - [x] `POST /auth/login` (login=username/email) + cookie session
  - [x] `POST /auth/logout`
  - [x] `GET/PUT /auth/me` + `PUT /auth/me/password`
  - [x] Protection des endpoints API (401 sans session)
- [x] **Gestion des users (admin)**: `GET/POST /users`, `GET/PUT/DELETE /users/:id`

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
- [x] UI Login + protection via middleware (redirection vers `/login`).
- [x] “User chip” + profil (édition profil + changement mdp) + logout.
- [x] Page `/users` (CRUD users).
- [x] **FinOps Dashboard** : Coûts réels/forecast/cumulatifs, breakdown par provider/instance/region/type, fenêtres temporelles (minute/heure/jour/30j/365j).

### FinOps (inventiv-finops)
- [x] Service de calcul automatique des coûts (tables TimescaleDB `finops.cost_*_minute`).
- [x] Calcul coûts réels (`cost_actual_minute`) : basé sur `EVT:INSTANCE_COST_START/STOP`.
- [x] Calcul coûts prévisionnels (`cost_forecast_minute`) : basé sur burn rate et horizons (1min, 1h, 1j, 30j, 365j).
- [x] Calcul coûts cumulatifs (`cost_actual_cumulative_minute`) : depuis différentes fenêtres temporelles.
- [x] Conversion USD → EUR : toutes les colonnes FinOps utilisent EUR (migration `20251215002000_finops_use_eur.sql`).

### API FinOps (inventiv-api)
- [x] Endpoints dashboard consolidés :
  - `GET /finops/dashboard/costs/summary` : Allocation totale + breakdown par provider/instance/region/type.
  - `GET /finops/dashboard/costs/window` : Détails par fenêtre temporelle (minute/heure/jour/30j/365j).
- [x] Endpoints séries temporelles :
  - `GET /finops/cost/actual/minute` : Série coûts réels.
  - `GET /finops/cost/cumulative/minute` : Série coûts cumulatifs.

---

## 🐛 Bugs connus / incohérences (à corriger en priorité)

### DB migrations / seeds
- [x] **Single source of truth migrations**:
  - `sqlx-migrations/` = migrations exécutées au boot (API + orchestrator)
  - `migrations/` = seeds uniquement (`seeds*.sql`)
- [ ] **Seeds non exécutés automatiquement**: il faut un mécanisme clair (script, make target, doc) pour initialiser providers/regions/zones/types/associations en dev.
- [x] Users: ajout `first_name`, `last_name`, `username` + bootstrap admin via secret file.

### Tooling / Ops
- [x] Makefile: `make dev-*`/`stg-*`/`prod-*` utilisent automatiquement `env/{env}.env` et échouent avec un message clair si manquant.
- [x] Secrets sync: `default_admin_password` sync via `scripts/remote_sync_secrets.sh`.
- [x] Prompt de clôture: `/.cursor/commands/close.md`.
- [x] Makefile: commande `make ui` pour démarrer le frontend facilement (crée `.env.local` si absent).
- [x] Deploy scripts: amélioration gestion certificats LEGO (SAN, append ROOT_DOMAIN pour éviter rate limits).

### Contrats API/UI à surveiller
- [ ] `instance_type_zones` existait dans la doc mais pas en SQL au départ → maintenant ajouté; vérifier que l’UI Settings alimente correctement cette table.
- [ ] `action_logs`:
  - [ ] schéma initial incomplet (pas de `metadata`, component check trop strict) → corrigé via migration dédiée; vérifier en DB.
  - [x] endpoint de recherche paginée + stats pour UI virtualisée: `GET /action_logs/search`
  - [x] table `action_types` (catalogue UI): `GET /action_types`

### Docs / scripts obsolètes
- [x] **Router**: README mis à jour pour clarifier que le Router est prévu mais non présent actuellement (phase 0.2.2).
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

## 🧭 Phase 0.2.1 — Worker ready (priorité)

### Worker (vLLM + agent sidecar)
- [x] Finaliser un **contrat minimal** Worker:
  - `/healthz` (liveness)
  - `/readyz` (readiness: modèle chargé / vLLM prêt)
  - `/metrics` (prometheus)
- [x] Implémenter le **protocole d’enrôlement** (worker → control-plane):
  - registration: `POST /internal/worker/register` (instance_id, model_id, ports, metadata)
  - heartbeat: `POST /internal/worker/heartbeat` (status, gpu util, metadata)
- [x] Auth worker (MVP): **token par instance** + **bootstrap** (DB `worker_auth_tokens` hashé)
- [ ] Déploiement “simple” multi-machines:
  - Docker Compose par machine + réseau privé (Tailscale/WireGuard)
  - volume cache modèles local
- [x] Health-check côté Orchestrator:
  - remplacer progressivement “SSH:22” par `GET http://<worker-ip>:<port>/readyz`
  - garder un fallback SSH tant que le worker n’est pas déployé partout
- [x] Harness local no-GPU: `scripts/dev_worker_local.sh` + profile compose `worker-local`

### Hardening (ensuite)
- [ ] Rotation / révocation des tokens worker (champs déjà présents: `revoked_at`, `rotated_at`)
- [ ] Trust boundary X-Forwarded-For: n’accepter XFF que depuis la gateway / réseau interne
- [ ] Option: `WORKER_AUTH_TOKEN_FILE` monté (ex: `/run/secrets/worker_token`) sur VMs GPU
- [ ] End-to-end staging Scaleway: vrai worker (vLLM) + register/heartbeat vers API domain

## 🧭 Phase 0.2.2 — Router MVP (data plane)

### Routing / Load Balancing
- [ ] Réintroduire un **router** (OpenAI-compatible):
  - `POST /v1/chat/completions` (proxy vers workers)
  - auth API keys + rate limiting
  - load balancing (LOR / queue depth)
  - failover (retry + circuit breaker)
- [ ] Source de vérité routing:
  - Redis (pub/sub + cache) pour discovery + stats temps réel

### Observabilité / Scalabilité
- [ ] Exposer `metrics` sur API/orchestrator/worker (+ router quand présent)
- [ ] Autoscaler (Orchestrator):
  - signaux: queue depth / ttft / gpu util / erreurs
  - politiques par pool (ex: `h100_8x80`, `l40s_4x48`)
  - drain → terminate + cooldowns

### Auth / API Keys
- [x] Auth (JWT session + users management) pour `inventiv-api`.
- [ ] Gestion des API keys (backend + router/gateway).
- [ ] RBAC plus fin (au-delà de `admin`) + politiques d’accès par endpoint.

### Frontend / DX
- [ ] Corriger warning eslint existant `useFinops.ts` (deps useEffect).
- [x] RBAC minimal (admin) + stockage sécurisé (hash bcrypt via pgcrypto).

### Worker agent
- [x] `inventiv-worker/agent.py`: implémenter heartbeat/metrics + protocole d’enrôlement.
- [x] Readiness réelle (pas juste SSH:22): health endpoint du worker/vLLM.

### Router / Data plane (à trancher)
- [ ] Décision: **réintroduire un Router** (OpenAI-compatible) OU supprimer la mention du router de la doc/scripts tant qu’il n’existe pas.
- [ ] Si router: validation API keys, routing dynamique (Redis), failover, rate limiting.

### Observabilité
- [ ] `/metrics` Prometheus sur chaque service + dashboards.
- [ ] Traces distribuées (optionnel).

---

## 🎯 Next steps (priorités immédiates)

1. **FinOps Tokens** : Implémenter tracking et forecast des tokens (priorités 4-5 FinOps) :
   - Consommation par modèle/instance/type/région/provider
   - Forecast de tokens à produire
   - Fenêtres temporelles (minute/heure/jour/30j/365j)
   - Tables `finops.inference_usage` + événements `EVT:TOKENS_CONSUMED`

2. **Worker deployment réel** : Valider end-to-end staging Scaleway avec vrai worker (vLLM) + register/heartbeat vers API domain.

3. **Router MVP** : Réintroduire router OpenAI-compatible (phase 0.2.2) OU supprimer définitivement les mentions du router tant qu'il n'existe pas.

4. **Autoscaling** : Implémenter autoscaler basé sur signaux router/worker (queue depth, latence, GPU util).

5. **Rotation tokens worker** : Implémenter rotation/révocation des tokens worker (champs déjà présents en DB).

6. **Metrics Prometheus** : Exposer `/metrics` sur chaque service (API, orchestrator, worker, finops).

7. **Catalogue minimal** : Assurer qu'un catalogue minimal (zones + instance types + associations) est présent pour que l'UI propose des choix valides.

---

## ✅ Recommandations (direction / “bonne trajectoire”)

- [x] **Single source of truth DB**: `sqlx-migrations/` pour migrations, `seeds/` pour seeds (workflow clarifié).
- [x] **Stabiliser les contrats**: OpenAPI/Swagger UI disponible, contrats API/UI alignés.
- [x] **Aligner la doc**: README restructuré selon plan complet, router clarifié comme prévu mais non présent.
- [ ] **Durcir le provisioning**: gestion d'erreurs, retries, timeouts, et logs exploitables (action_logs + metadata).
