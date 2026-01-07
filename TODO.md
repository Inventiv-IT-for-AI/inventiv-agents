# Roadmap & TODO (état repo + backlog)

Ce fichier reflète l’état **réel** du repo (code + migrations + UI) et la suite (priorisée).

---

## ✅ Réalisé (livré dans le code)

### Control-plane & provisioning
- ✅ **Provisioning Scaleway** (orchestrator): création VM avec image uniquement, Block Storage automatique (20GB), agrandissement à 200GB via CLI, poweron, récupération IP, Security Groups, SSH accessible (~20s), transitions d'état. **Validé pour L4-1-24G**.
- ✅ **Provisioning Mock** (inventiv-providers): gestion automatique des runtimes Docker Compose, récupération IP, transitions d'état.
- ✅ **Architecture providers modulaire**: package `inventiv-providers` avec trait `CloudProvider`, séparation orchestrator/providers.
- ✅ **State machine + jobs**: provisioning/health-check/terminator/watch-dog + requeue.
- ✅ **Auto-install worker**: bootstrap via SSH avec phases `::phase::…`, logs enrichis dans `action_logs.metadata`.
- ✅ **Sizing stockage par modèle**: taille recommandée depuis la table `models` (fallbacks contrôlés).
- ✅ **HF token**: support `WORKER_HF_TOKEN_FILE` (secret file) + alias `HUGGINGFACE_TOKEN`.
- ✅ **Scaleway Block Storage**: Séquence validée - création automatique avec image (20GB bootable), agrandissement à 200GB avant démarrage, SSH opérationnel après ~20 secondes.

### Modèles & readiness
- **Catalogue `models`**: champs `is_active`, `data_volume_gb`, metadata (seed enrichi).
- **Sélecteur de modèle obligatoire** côté UI + **enforcement API** (`model_id` requis pour créer une instance).
- **Readiness industrialisée**: actions `WORKER_VLLM_HTTP_OK`, `WORKER_MODEL_LOADED`, `WORKER_VLLM_WARMUP`.
- **Modes vLLM**: `mono` (1 vLLM) / `multi` (1 vLLM par GPU derrière HAProxy sticky).

### OpenAI-compatible API + API keys
- **OpenAI proxy** (inventiv-api): `/v1/models`, `/v1/chat/completions` (streaming), `/v1/completions`, `/v1/embeddings`.
- **API keys (client)**: CRUD + auth `Authorization: Bearer <key>` (séparé des tokens workers).
- **Live capacity**: `/v1/models` reflète les modèles réellement servis par des workers "fresh" (avec tolérance staleness).
- ✅ **Résolution modèles HuggingFace**: Correction de la logique pour éviter les faux positifs avec les offering ids (`org_slug/model_code`)

### Runtime models dashboard + Workbench
- **Runtime models**: endpoint + page UI `/models` (instances, GPUs, VRAM, requests, failed).
- **Workbench**: page UI `/workbench` (base URL, snippets, test chat via API key).

### Temps réel (UI)
- **SSE**: `GET /events/stream` (topics instances/actions) + hook frontend `useRealtimeEvents` (refresh instances + action logs).
- **IADataTable persistence**: préférences colonnes persistées (tri/largeur/ordre/visibilité) pour les tables IA (dont la pop-in “Actions de l’instance”).

### UI / Design system (monorepo)
- **Packages internes**:
  - `inventiv-ui/ia-designsys` (primitives UI centralisées)
  - `inventiv-ui/ia-widgets` (widgets de plus haut niveau, préfixe `IA*`)
- **Tailwind v4 (CSS-first)**: ajout des `@source` vers les packages workspaces (`ia-widgets`, `ia-designsys`) pour éviter toute purge de classes.
- **IADataTable**: table virtualisée réutilisable (dans `ia-widgets`) + **resize via séparateurs dédiés** (5px) entre colonnes.
- **Ergonomie dev**: `make ui-down` et `make ui-local-down` (stop UI Docker / kill UI host).
- ✅ **Affichage Version**: Badge discret sous le titre de l'application avec popover au hover/click affichant version FE, BE et timestamp du build.

### Dev ergonomics
- **PORT_OFFSET** (worktrees) + UI-only exposée.
- **`make api-expose`**: proxy loopback pour tunnels (cloudflared) sans modifier `docker-compose.yml`.
- **DB/Redis stateful**: `make down` garde volumes, `make nuke` wipe.

### Multi-tenant (MVP)
- **Organisations**: création + membership + sélection “organisation courante” (switcher UX).
- **Pré-câblage DB “model sharing + chargeback tokens”** (non-breaking): tables `organization_models` + `organization_model_shares` + extension `finops.inference_usage`.

---

## 🐛 Bugs connus / dettes techniques (à suivre)

- **SSE**: implémentation actuelle basée sur polling DB (efficace mais pas "event-sourced" → à améliorer via NOTIFY/LISTEN ou Redis streams).
- **Observabilité**: pas encore de stack métriques/traces end-to-end (Prometheus/Grafana/OTel) + alerting.
- ✅ **FinOps**: coûts OK + **comptage tokens in/out** implémenté (voir section "FinOps full features").
- **Docs**: certains documents restent "vision" (router, bare-metal) vs "implémenté".
- **Mock provider routing**: le test E2E OpenAI proxy override `instances.ip_address` vers `mock-vllm` (hack local). À remplacer par un mécanisme propre (voir backlog).
- **Docker CLI version**: orchestrator utilise Docker CLI 27.4.0 (compatible API 1.44+). À documenter les prérequis Docker dans la doc.
- ✅ **Progression "starting"**: Corrigé - les instances "starting" affichent maintenant la progression correcte
- ✅ **Health checks "starting"**: Corrigé - les instances "starting" sont maintenant vérifiées par le health check job
- ✅ **Résolution modèles publics**: Corrigé - les modèles HuggingFace publics fonctionnent sans organisation
- ⚠️ **Volumes non libérés**: Certaines terminaisons d'instances ne libèrent pas correctement les block storage associés (voir section "Fiabilité Workers & Instances").

---

## 🚧 À faire (backlog)

### Fiabilité Workers & Instances (Priorité)

#### 1. Détection des Workers Morts
- [ ] Créer `job-worker-watchdog.rs` pour détecter workers sans heartbeat récent (> 5 min)
- [ ] Transition automatique `ready` → `worker_dead` si heartbeat > seuil configurable
- [ ] Option de réinstallation automatique pour les workers morts
- [ ] Tests unitaires et E2E

#### 2. Amélioration des Health Checks
- [ ] Implémenter backoff exponentiel pour health checks échoués
- [ ] Réduire timeouts par défaut (configurables via env vars)
- [ ] Ajouter cache des résultats de health checks (< 30s)
- [ ] Métriques de latence des health checks

#### 3. Extension du Job Recovery
- [ ] Détecter `installing` / `starting` bloquées > seuil configurable
- [ ] Ajouter alertes (logs structurés) pour instances bloquées
- [ ] Circuit breaker pour instances avec trop d'échecs consécutifs

#### 4. Réconciliation des Volumes (EN COURS)
- [ ] Créer `job-volume-reconciliation.rs` pour détecter volumes orphelins
- [ ] Détecter volumes dans DB mais pas chez provider (nettoyer DB)
- [ ] Détecter volumes chez provider mais pas dans DB (tracker et supprimer)
- [ ] Retry automatique avec backoff pour suppressions échouées
- [ ] Vérifier volumes marqués `deleted_at` mais qui existent encore chez provider
- [ ] Tests E2E pour valider la réconciliation

#### 5. Métriques et Observabilité
- [ ] Exposer métriques Prometheus pour tous les jobs (latence, taux d'échec, instances traitées)
- [ ] Dashboard Grafana (optionnel)
- [ ] Système d'alertes basé sur métriques (instances bloquées, workers morts, volumes orphelins)
- [ ] Étendre utilisation de `correlation_id` pour tracing end-to-end

### Scaleway Provider - Implémentation de la séquence validée
- [ ] **Adapter le code Scaleway Provider** pour utiliser la séquence validée :
  - Créer instance avec image uniquement (pas de volumes)
  - Détecter et agrandir le Block Storage créé automatiquement (20GB → 200GB) via CLI
  - Configurer Security Groups (ports 22, 8000, 8080)
  - Vérifier SSH accessible avant installation worker
- [ ] **Mettre à jour la state machine générique** pour supporter les nouvelles étapes :
  - `PROVIDER_VOLUME_RESIZE` (25%)
  - `PROVIDER_SECURITY_GROUP` (45%)
  - `WORKER_SSH_ACCESSIBLE` (50%)
- [ ] **Tester avec autres types d'instances** : L40S, H100 (séquence devrait être identique)
- [ ] **Documentation** : Mettre à jour les guides utilisateur avec la nouvelle séquence

## 🚧 À faire (backlog)

### Déploiement & DNS
- **Staging**: déploiement sur `studio-stg.inventiv-agents.fr` (routing API + edge + certs).
- **Production**: déploiement sur `studio-prd.inventiv-agents.fr`.

### UX / API
- **System Prompt configurable** (Inventiv-Agents): UI + API + persistence (par modèle / par tenant / par key).
- **Streaming**: améliorer streaming E2E (Workbench + proxy + UI) + UX (annulation, TTFT, tokens/sec).

### Observability / Monitoring
- ✅ **Metrics**: `/metrics` sur API/orchestrator/worker + dashboards (CPU/Mem/Disk/Net + GPU per-index) + SLOs.
  - Implémenté: métriques système (CPU/Mem/Disk/Net) et GPU dans dashboard Observability
  - Implémenté: métriques requêtes et tokens par instance (`GET /instances/:instance_id/metrics`)
- ✅ **Progress Tracking**: Système de progression 0-100% basé sur les actions complétées
  - Implémenté: calcul automatique dans `inventiv-api/src/progress.rs`
  - Implémenté: affichage dans UI avec colonne dédiée
  - Implémenté: étapes granulaires (SSH install, vLLM HTTP, model loaded, warmup, health check)
  - ✅ **Séquence Scaleway validée**: Étapes spécifiques ajoutées (PROVIDER_VOLUME_RESIZE 25%, PROVIDER_SECURITY_GROUP 45%, WORKER_SSH_ACCESSIBLE 50%)
  - ✅ **Statuts "installing" et "starting"**: Ajout des statuts intermédiaires pour tracking granulaire
  - ✅ **Gestion progression multi-statuts**: Calcul de progression corrigé pour "installing" et "starting"
  - ✅ **Health checks multi-statuts**: Health check job vérifie maintenant "booting", "installing", et "starting"
- ✅ **Agent Version Management**: Versioning et checksum SHA256 pour `agent.py`
  - Implémenté: constantes `AGENT_VERSION` et `AGENT_BUILD_DATE` dans agent.py
  - Implémenté: endpoint `/info` pour exposer version/checksum
  - Implémenté: vérification checksum dans script SSH bootstrap
  - Implémenté: tooling Makefile (`agent-checksum`, `agent-version-bump`, etc.)
  - Implémenté: CI/CD integration (vérification automatique, workflow de bump)
  - Implémenté: monitoring dans health checks et heartbeats
- ✅ **Storage Management**: Gestion automatique du cycle de vie des volumes
  - Implémenté: découverte automatique des volumes attachés (`list_attached_volumes`)
  - Implémenté: tracking dans `instance_volumes` avec `delete_on_terminate`
  - Implémenté: suppression automatique lors de la terminaison
  - Implémenté: détection des volumes de boot créés automatiquement
- ✅ **State Machine**: Transitions explicites et historisation
  - Implémenté: fonctions explicites dans `state_machine.rs`
  - Implémenté: historique dans `instance_state_history`
  - Implémenté: logging structuré avec métadonnées
  - ✅ **Statuts intermédiaires**: Ajout de "installing" et "starting" pour tracking granulaire
  - ✅ **Transitions multi-statuts**: Support des transitions depuis "booting" ou "installing" vers "starting"
- ✅ **Worker Event Logging**: Système de logging structuré sur le worker pour diagnostics
  - Implémenté: fonction `_log_event()` dans `agent.py` avec rotation automatique (10MB, 10k lignes)
  - Implémenté: endpoint `/logs` pour récupérer les logs via HTTP (`?tail=N&since=ISO8601`)
  - Implémenté: événements loggés (agent_started, register_start/success/failed, heartbeat_success/failed/exception, vllm_ready/not_ready, etc.)
  - Implémenté: intégration dans orchestrator (`fetch_worker_logs()`) pour analyser les logs avant de relancer l'install SSH
  - Implémenté: vérification de l'état des conteneurs via SSH (`check_containers_via_ssh()`) avant retry
  - Implémenté: logs de diagnostic (`WORKER_CONTAINER_CHECK`, `WORKER_LOG_ERRORS`, `WORKER_LOG_FETCH`) dans l'orchestrator
- **Tracing**: OTel (optionnel au début) + corrélation `correlation_id` (API ↔ orchestrator ↔ worker ↔ upstream).
  - Partiellement: `correlation_id` ajouté dans logs API, à étendre aux autres services
- **Monitoring infra**: GPU util, queue depth, vLLM health, erreurs, saturation, qualité du load-balancing.
- **E2E test chain (mock)**: étendre le test pour valider aussi le routing OpenAI sans hack DB (voir item "mock provider routing").

### Mock provider / tests
- ✅ **Gestion automatique des runtimes Mock**: création/suppression via Docker Compose dans `inventiv-providers/src/mock.rs`.
- ✅ **Scripts de synchronisation**: `mock_runtime_sync.sh` pour synchroniser les runtimes avec les instances actives.
- ✅ **Tests E2E multi-instances**: `test_worker_observability_mock_multi.sh` pour valider le provisionnement en série et parallèle.
- ✅ **Docker CLI/Compose dans orchestrator**: Docker CLI 27.4.0 + Docker Compose plugin v2.27.1 installés dans `Dockerfile.rust`.
- ✅ **Réseau Docker explicite**: `CONTROLPLANE_NETWORK_NAME` configuré dans `docker-compose.yml` pour éviter les erreurs de réseau.
- **Routage OpenAI proxy en mock**: rendre l'upstream joignable sans muter `instances.ip_address` (options: IP routable mock, ou param "upstream_base_url" par instance en DB, ou résolution "service name" côté API quand provider=mock).
- **Tests contractuels**: ajouter des tests (Rust) des payloads `register/heartbeat` (schema/validation) + compat rétro (old heartbeat payload sans `system_samples`).
- **Documentation Mock provider**: créer `docs/providers.md` avec architecture et guide d'utilisation.

### FinOps "full features"
- ✅ **Comptage tokens in/out** par Worker / API_KEY / User / Tenant / Model.
  - Implémenté: extraction tokens depuis réponses streaming/non-streaming, stockage dans `instance_request_metrics` et `finops.inference_usage`
  - Endpoint: `GET /instances/:instance_id/metrics`
  - Dashboard: métriques affichées dans Observability (`/observability`)
- **Validation**: consolidation dashboards + exports + séries temporelles.

### Secrets & credentials
- **AUTO_SEED_PROVIDER_CREDENTIALS**: documenter clairement le modèle “secrets in /run/secrets → provider_settings chiffré pgcrypto” + rotation/rollback + conventions de clés (`SCALEWAY_PROJECT_ID`, `SCALEWAY_SECRET_KEY_ENC`) + menace (logs/backup).

### Multi-tenant & sécurité
- ✅ **Organisations (MVP)**: création + membership + sélection "organisation courante" (switcher UX).
- ✅ **Pré-câblage DB "model sharing + chargeback"** (non-breaking):
  - `organizations` + `organization_memberships` + `users.current_organization_id`
  - `organization_models` (offering publié par org)
  - `organization_model_shares` (contrats provider→consumer, `pricing` JSONB)
  - extension `finops.inference_usage` pour attribuer `provider_organization_id` / `consumer_organization_id` + `unit_price_eur_per_1k_tokens` + `charged_amount_eur`
- ✅ **RBAC Foundation**: Module RBAC avec rôles Owner/Admin/Manager/User, règles de délégation, double activation (tech/eco).
- ✅ **Gestion Membres**: Endpoints pour lister/changer rôle/retirer membres avec invariant "dernier owner".
- ✅ **Bootstrap Default Org**: Création automatique org "Inventiv IT" avec admin comme owner.
- ✅ **Password Reset Flow**: Intégration SMTP Scaleway TEM, génération de tokens sécurisés, emails de réinitialisation, endpoints API complets.
- ✅ **Code Reorganization**: Refactoring majeur de `main.rs` (~3500 lignes → ~86 lignes), extraction en modules `config/`, `setup/`, `routes/`, `handlers/` pour meilleure maintenabilité.
- ✅ **Integration Tests**: Infrastructure de tests d'intégration avec `axum-test`, tests pour auth, deployments, instances (Mock provider uniquement pour éviter coûts cloud).
- ✅ **Axum 0.8 Upgrade**: Migration vers `axum 0.8` et `axum-test 18.0`, corrections pour `async_trait`, `SwaggerUi`, `FromRequestParts`, compatibilité OpenAPI avec `utoipa 5.4`.
- ⏳ **Architecture Sessions Multi-Org**: Table `user_sessions` pour plusieurs sessions simultanées avec orgs différentes (voir `docs/SESSION_ARCHITECTURE_PROPOSAL.md`).
- ⏳ **Scoping Instances**: Isoler instances par `organization_id` + RBAC.
- ⏳ **Scoping Models**: Isoler modèles par `organization_id` + visibilité publique/privée.
- ⏳ **Invitations**: Inviter users par email dans une organisation.
- ⏳ **Scoping API Keys**: Isoler clés API par `organization_id`.
- ⏳ **Scoping Users**: Filtrer liste users selon workspace.
- ⏳ **Scoping FinOps**: Filtrer dashboards financiers selon workspace.
- ⏳ **Migration Frontend Modules**: Masquer/afficher modules selon workspace + rôle.
- ⏳ **Double Activation**: Activation technique (Admin) + économique (Manager) par ressource.
- ⏳ **Model Sharing & Billing**: Partage modèles entre orgs avec facturation au token.

📄 Doc: `docs/MULTI_TENANT_MODEL_SHARING_BILLING.md` (pricing v1 = **€/1k tokens**)
- **Tenants v1 (Org isolation)**:
  - Isoler les ressources “métier” par `organization_id` (au minimum: instances, workbench_runs, action_logs, api_keys).
  - Introduire une notion d’**org courante obligatoire** pour les endpoints métier (401/409 si non sélectionnée).
  - Clarifier RBAC org: `owner|admin|manager|user` + policy par endpoint.
  - Règles RBAC:
    - Invitations: Owner/Admin/Manager
    - Dernier Owner non révocable
    - Audit logs immuables (pas de delete)
  - “Double activation”:
    - Admin active techniquement (providers/regions/zones/types/models/api_keys/users/plan)
    - Manager active économiquement (providers/regions/zones/types/models/api_keys/users/plan)
    - Opérationnel uniquement si les 2 activations sont OK (par ressource)
    - UX: afficher un état “non opérationnel” + alerte indiquant le flag manquant (tech/eco)
  - (Plus tard) **RLS PostgreSQL** une fois le modèle stabilisé.
  - UX anti-erreur: **couleur de sidebar configurable par organisation** (visuel “scope changed”).

📄 Roadmap cible: `docs/MULTI_TENANT_ROADMAP.md` (users first-class + org workspaces + community offerings + entitlements + billing tokens)

- **API keys org-owned (prévu)**:
  - Activer `api_keys.organization_id` (actuellement nullable) + migration data (si besoin).
  - Résolution “consumer org” via API key (prioritaire) ou session (org courante).

- **Partage de modèles inter-org (provider→consumer)**:
  - CRUD `organization_models` (publish/unpublish).
  - CRUD `organization_model_shares` (grant/pause/revoke + pricing JSONB).
  - Convention d’identifiant “virtual model”: `org_slug/model_code` (côté OpenAI proxy).
  - Clarifier `visibility`: `public | unlisted | private` (private = org-only; unlisted = non listé mais accessible si autorisé).
  - Ajouter “consumer org discovery prefs” (autoriser/masquer public/payant/payant-with-contract).

- **Chargeback tokens (v1)**:
  - Ingestion/persistence des events `finops.inference_usage` avec:
    - `consumer_organization_id`, `provider_organization_id`, `organization_model_id`
    - pricing v1: `eur_per_1k_tokens`, calcul `charged_amount_eur`
  - Exposer dashboards/exports “consommation par org / provider / consumer”.

### Data plane / perf
- **Optimisation load-balancing** (sticky, health scoring, failover, retry policy).
- **Auto scale-up / auto scale-down**.
- **Support other Cloud Providers** (AWS/GCP/etc).
- **Support on-prem / private / shared bare metal servers**.

---

## 🎯 Next steps Multi-Tenant (priorités)

**Phase Immédiate (Sprint 1)** :
1) **Architecture Sessions Multi-Org** : Table `user_sessions`, migration `current_organization_id`, enrichir JWT avec `session_id` + `organization_role`  
2) **Migration PK/FK** : Appliquer migration `20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql`

**Phase Court Terme (Sprint 2-3)** :
3) **Scoping Instances** : Migration SQL + API + UI + Tests pour isoler instances par `organization_id`  
4) **Scoping Models** : Migration SQL + API + UI + Tests pour isoler modèles par `organization_id`  
5) **Invitations** : Migration SQL + API + UI + Tests pour inviter users par email

**Phase Moyen Terme (Sprint 4-6)** :
6) **Scoping API Keys** : API + UI + Tests  
7) **Scoping Users** : API + UI + Tests  
8) **Scoping FinOps** : API + UI + Tests  
9) **Migration Frontend Modules** : Masquer/afficher selon workspace + rôle

**Phase Long Terme (Sprint 7+)** :
10) **Double Activation** : Tech (Admin) + Eco (Manager) par ressource  
11) **Model Sharing & Billing** : Partage modèles entre orgs avec facturation au token

**Autres priorités** :
- **Deploy Staging + DNS** (`studio-stg.inventiv-agents.fr`) avec routing propre UI/API + certs  
- **Observability** (metrics + dashboards minimum viable)  
- **LB hardening** + signaux worker (queue depth / TTFT)  
- **Autoscaling MVP** (politiques + cooldowns)

---

## 🧪 Tests & Validation (nouvelles fonctionnalités)

### Progress Tracking
- ✅ **Test E2E Scaleway** : Validé avec script `test-scaleway/test_complete_validation.rs` - toutes les étapes fonctionnent
- [ ] **Test unitaire** : Vérifier le calcul de progression pour chaque étape
- [ ] **Test E2E Mock** : Valider la progression simulée pour instances Mock
- [ ] **Test UI** : Vérifier l'affichage de la colonne progress dans la table
- [ ] **Test SSE** : Vérifier la mise à jour en temps réel du progress

### Agent Version Management
- [ ] **Test checksum** : Vérifier que le checksum est calculé correctement
- [ ] **Test vérification** : Valider que le script bootstrap détecte les checksums invalides
- [ ] **Test endpoint /info** : Vérifier que `/info` retourne les bonnes informations
- [ ] **Test heartbeat** : Valider que `agent_info` est inclus dans les heartbeats
- [ ] **Test health check** : Vérifier que le health check récupère et log les infos agent
- [ ] **Test CI/CD** : Valider que `make agent-version-check` échoue si version non mise à jour
- [ ] **Test workflow GitHub** : Valider que le workflow `agent-version-bump` fonctionne
- [ ] **Test version mismatch** : Simuler une version incorrecte et vérifier la détection
- [ ] **Test checksum mismatch** : Simuler un checksum invalide et vérifier l'échec du bootstrap

### Storage Management
- [ ] **Test découverte volumes** : Valider que `list_attached_volumes` découvre tous les volumes
- [ ] **Test création** : Vérifier que les volumes sont trackés immédiatement après création
- [ ] **Test terminaison** : Valider que tous les volumes sont supprimés lors de la terminaison
- [ ] **Test volumes boot** : Vérifier que les volumes de boot créés automatiquement sont trackés
- [ ] **Test volumes persistants** : Valider que `delete_on_terminate=false` préserve les volumes
- [ ] **Test erreur suppression** : Simuler une erreur de suppression et vérifier le logging
- [ ] **Test volumes locaux** : Valider la détection et le rejet des volumes locaux pour L40S/L4
- [ ] **Test récupération** : Vérifier que les volumes non supprimés peuvent être nettoyés manuellement

### State Machine
- [ ] **Test transitions** : Valider chaque transition d'état (booting→ready, booting→startup_failed, etc.)
- [ ] **Test idempotence** : Vérifier que les transitions sont idempotentes
- [ ] **Test historique** : Valider que `instance_state_history` enregistre toutes les transitions
- [ ] **Test récupération** : Vérifier la récupération automatique (STARTUP_TIMEOUT → booting)
- [ ] **Test erreurs spécifiques** : Valider les transitions vers `startup_failed` avec codes d'erreur spécifiques

### Monitoring & Observabilité
- [ ] **Test health check agent_info** : Vérifier que le health check récupère `/info`
- [ ] **Test métadonnées** : Valider que `agent_info` est stocké dans `worker_metadata`
- [ ] **Test logs** : Vérifier que les métadonnées agent sont incluses dans les logs de health check
- [ ] **Test détection problèmes** : Simuler des problèmes (version incorrecte, checksum invalide) et vérifier la détection
- [ ] **Test rate limiting** : Valider le rate limiting des logs de health check (5min succès, 1min échec)

### Intégration
- [ ] **Test complet cycle** : Provisionner une instance Scaleway et valider :
  - Découverte des volumes
  - Vérification checksum agent
  - Progression 0-100%
  - Health checks avec agent_info
  - Terminaison et suppression des volumes
- [ ] **Test Mock provider** : Valider que toutes les fonctionnalités fonctionnent avec Mock
- [ ] **Test multi-instances** : Valider avec plusieurs instances en parallèle
- [ ] **Test récupération** : Valider la récupération après erreurs (timeout, checksum mismatch, etc.)

### Documentation
- [ ] **Mise à jour README** : Ajouter références aux nouveaux documents
- [ ] **Validation docs** : Vérifier que tous les exemples de code fonctionnent
- [ ] **Guide utilisateur** : Créer un guide pour utiliser les nouvelles fonctionnalités

---

## 🚀 Plan d’implémentation (step-by-step, testable) — RBAC + scoping org

### Phase 1 — RBAC foundation (backend + tests) → commit
- **DB (migrations)**:
  - Normaliser `organization_memberships.role` sur: `owner|admin|manager|user`
  - Backfill: `member` → `user` (si présent)
  - Contrainte `CHECK` + `DEFAULT 'user'`
- **Backend (Rust)**:
  - Module RBAC (enum + helpers): rôle org, règles d’assignation (Owner/Admin/Manager), double activation (tech/eco)
  - Tests unitaires sur la matrice RBAC (sans DB)
- **Tests**:
  - `cargo check -p inventiv-api`
  - `cargo test -p inventiv-api`

### Phase 2 — Roles associés aux users (membership lifecycle) + tests → commit
- **API (org-scopé)**:
  - `GET /organizations/members`
  - `PUT /organizations/members/:user_id/role` (règles: Owner tout; Manager ↔ User; Admin ↔ User)
  - `DELETE /organizations/members/:user_id` + invariant “dernier Owner non révocable”
- **Audit logs**: loguer role changes et removals (immutables)
- **Tests**: dernier owner, escalations interdites, etc.

### Phase 3 — Invitations + Users management + tests → commit
- **DB**: `organization_invitations` (email, token, expiry, role, invited_by, accepted_at)
- **API**:
  - `POST /organizations/invitations`
  - `GET /organizations/invitations`
  - `POST /organizations/invitations/:token/accept` (user existant ou création)
- **UI**: inviter, voir pending, accepter (flow)

### Phase 4 — Settings org-scopés + double activation + tests → commit(s)
- Providers/regions/zones/types/models/settings scoppés org
- Double activation **par ressource**:
  - Admin = tech only, Manager = eco only, Owner = both
  - UI: état “non opérationnel” + alerte flag manquant

### Phase 5 — Instances org-scopées + RBAC + tests → commit(s)
- Admin/Owner: ops (provision/terminate/reinstall/scheduling/scaling)
- Manager: finance gating + dashboards
- User: usage / lecture selon politique

### Phase 6 — Models/Offerings + RBAC + tests → commit(s)
- Admin: config technique + publication
- Manager: pricing + activation économique + partage
- Owner: tout
