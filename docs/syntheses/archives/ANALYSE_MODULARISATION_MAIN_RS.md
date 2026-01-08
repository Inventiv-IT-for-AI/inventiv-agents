# Analyse et Proposition de Modularisation du `main.rs` (inventiv-api)

**Date**: 2024  
**Objectif**: Analyser le fichier `main.rs` (3907 lignes) et proposer une organisation modulaire basée sur DDD (Domain-Driven Design) avant refactoring.

---

## 📊 État Actuel

### Statistiques
- **Taille**: 3907 lignes
- **Modules externes déjà extraits**: 18 fichiers modulaires existants
- **Endpoints définis**: ~70+ routes
- **Fonctions métier dans main.rs**: ~30+ fonctions

### Structure Actuelle

```
inventiv-api/src/
├── main.rs (3907 lignes - TOO LARGE)
├── action_logs_endpoint.rs
├── action_logs_search.rs
├── api_docs.rs
├── api_keys.rs
├── audit_log.rs
├── auth_endpoints.rs
├── auth.rs
├── bootstrap_admin.rs
├── chat.rs
├── finops.rs
├── instance_type_zones.rs
├── metrics.rs
├── openai_proxy.rs
├── organizations.rs
├── provider_settings.rs
├── rbac.rs
├── settings.rs
├── simple_logger.rs
├── users_endpoint.rs
├── workbench.rs
└── worker_routing.rs
```

---

## 🎯 Domaines Métier Identifiés

### 1. **Authentication & Authorization** (Déjà modulaire ✅)
- **Fichiers**: `auth.rs`, `auth_endpoints.rs`, `rbac.rs`
- **Endpoints**:
  - `POST /auth/login`
  - `POST /auth/logout`
  - `GET /auth/me`
  - `PUT /auth/me`
  - `PUT /auth/me/password`
- **État**: Bien organisé, pas de changement nécessaire

### 2. **Models (Catalogue de Modèles LLM)** ❌ À extraire
- **Endpoints dans main.rs**:
  - `GET /models` → `list_models()`
  - `POST /models` → `create_model()`
  - `GET /models/:id` → `get_model()`
  - `PUT /models/:id` → `update_model()`
  - `DELETE /models/:id` → `delete_model()`
  - `GET /instance_types/:id/models` → `list_compatible_models()`
- **Logique métier**: CRUD complet, validation de compatibilité instance_type
- **Proposé**: `src/domains/models/` ou `src/models/`

### 3. **Instances (Gestion des Instances GPU)** ❌ À extraire
- **Endpoints dans main.rs**:
  - `GET /instances` → `list_instances()`
  - `GET /instances/search` → `search_instances()`
  - `GET /instances/:id` → `get_instance()`
  - `DELETE /instances/:id` → `terminate_instance()`
  - `PUT /instances/:id/archive` → `archive_instance()`
  - `POST /instances/:id/reinstall` → `reinstall_instance()`
  - `GET /instances/:id/metrics` → `metrics::get_instance_metrics()` (déjà extrait ✅)
- **Logique métier**: CRUD, recherche paginée, cycle de vie (terminate, archive, reinstall)
- **Proposé**: `src/domains/instances/` ou `src/instances/`

### 4. **Deployments (Déploiement de Modèles)** ❌ À extraire
- **Endpoints dans main.rs**:
  - `POST /deployments` → `create_deployment()`
- **Logique métier**: Création d'instance + modèle, validation provider, publication Redis event
- **Complexité**: ~600 lignes dans `create_deployment()`
- **Proposé**: `src/domains/deployments/` ou `src/deployments/`

### 5. **Runtime & Observability** ❌ Partiellement à extraire
- **Endpoints dans main.rs**:
  - `GET /runtime/models` → `list_runtime_models()` (~100 lignes)
  - `GET /gpu/activity` → `list_gpu_activity()` (~300 lignes)
  - `GET /system/activity` → `list_system_activity()` (~200 lignes)
- **Logique métier**: Agrégation de métriques temps-réel depuis instances
- **Proposé**: `src/domains/observability/` ou `src/observability/`

### 6. **Action Logs (Audit Trail)** ⚠️ Partiellement extrait
- **Endpoints**:
  - `GET /action_logs` → `list_action_logs()` (dans main.rs)
  - `GET /action_logs/search` → `action_logs_search::search_action_logs()` (déjà extrait ✅)
  - `GET /action_types` → `list_action_types()` (dans main.rs)
- **Proposé**: Consolider dans `src/domains/action_logs/`

### 7. **Commands (Commandes Orchestrator)** ❌ À extraire
- **Endpoints dans main.rs**:
  - `POST /reconcile` → `manual_reconcile_trigger()`
  - `POST /catalog/sync` → `manual_catalog_sync_trigger()`
- **Logique métier**: Publication d'événements Redis vers orchestrator
- **Proposé**: `src/domains/commands/` ou `src/commands/`

### 8. **Realtime (SSE Events)** ❌ À extraire
- **Endpoints dans main.rs**:
  - `GET /events/stream` → `events_stream()` (~200 lignes)
- **Logique métier**: Server-Sent Events pour instances/actions updates
- **Proposé**: `src/domains/realtime/` ou `src/realtime/`

### 9. **Settings (Configuration Infrastructure)** ⚠️ Partiellement extrait
- **Endpoints**:
  - Providers: `settings::*` (déjà extrait ✅)
  - Regions: `settings::*` (déjà extrait ✅)
  - Zones: `settings::*` (déjà extrait ✅)
  - Instance Types: `settings::*` (déjà extrait ✅)
  - Provider Settings: `provider_settings::*` (déjà extrait ✅)
- **État**: Bien organisé, pas de changement nécessaire

### 10. **Organizations (Multi-tenant)** ✅ Déjà modulaire
- **Fichier**: `organizations.rs`
- **État**: Bien organisé

### 11. **API Keys** ✅ Déjà modulaire
- **Fichier**: `api_keys.rs`
- **État**: Bien organisé

### 12. **Users** ✅ Déjà modulaire
- **Fichier**: `users_endpoint.rs`
- **État**: Bien organisé

### 13. **Finops** ✅ Déjà modulaire
- **Fichier**: `finops.rs`
- **État**: Bien organisé

### 14. **Workbench** ✅ Déjà modulaire
- **Fichier**: `workbench.rs`
- **État**: Bien organisé

### 15. **Chat** ✅ Déjà modulaire
- **Fichier**: `chat.rs`
- **État**: Bien organisé

### 16. **OpenAI Proxy** ✅ Déjà modulaire
- **Fichier**: `openai_proxy.rs`
- **Endpoints dans main.rs**: Routes définies mais handlers dans module
- **État**: Bien organisé

### 17. **Worker Internal Routes** ❌ À extraire
- **Endpoints dans main.rs**:
  - `POST /internal/worker/register` → `proxy_worker_register()`
  - `POST /internal/worker/heartbeat` → `proxy_worker_heartbeat()`
- **Logique métier**: Proxy vers orchestrator avec auth worker
- **Proposé**: `src/domains/worker/` ou `src/worker/` (ou intégrer dans `worker_routing.rs`)

---

## 📁 Proposition d'Organisation Modulaire (DDD)

### Structure Proposée

```
inventiv-api/src/
├── main.rs                          # Orchestration uniquement (~200 lignes)
├── lib.rs                           # Exports publics
│
├── domains/                         # Domaines métier (DDD)
│   ├── models/
│   │   ├── mod.rs                  # Module exports
│   │   ├── handlers.rs             # Endpoints handlers
│   │   ├── service.rs              # Logique métier
│   │   └── dto.rs                   # Request/Response DTOs
│   │
│   ├── instances/
│   │   ├── mod.rs
│   │   ├── handlers.rs             # list, get, search, terminate, archive, reinstall
│   │   ├── service.rs              # Logique métier instances
│   │   └── dto.rs
│   │
│   ├── deployments/
│   │   ├── mod.rs
│   │   ├── handlers.rs             # create_deployment
│   │   ├── service.rs              # Validation, orchestration
│   │   └── dto.rs
│   │
│   ├── observability/
│   │   ├── mod.rs
│   │   ├── runtime_models.rs       # list_runtime_models
│   │   ├── gpu_activity.rs         # list_gpu_activity
│   │   ├── system_activity.rs      # list_system_activity
│   │   └── dto.rs
│   │
│   ├── action_logs/
│   │   ├── mod.rs
│   │   ├── handlers.rs             # list_action_logs, list_action_types
│   │   ├── search.rs               # search_action_logs (déjà existant)
│   │   └── dto.rs
│   │
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── handlers.rs             # reconcile, catalog_sync
│   │   └── service.rs              # Redis event publishing
│   │
│   ├── realtime/
│   │   ├── mod.rs
│   │   ├── handlers.rs             # events_stream
│   │   └── service.rs              # SSE logic, signature tracking
│   │
│   └── worker/
│       ├── mod.rs
│       ├── handlers.rs             # register, heartbeat (proxy)
│       └── service.rs               # Auth verification, proxy logic
│
├── infrastructure/                  # Infrastructure & Cross-cutting
│   ├── database.rs                  # Pool, migrations, seeds
│   ├── redis.rs                     # Redis client setup
│   ├── state.rs                     # AppState definition
│   └── config.rs                    # Configuration (env vars, URLs)
│
├── middleware/                      # Middleware Axum
│   ├── mod.rs
│   ├── auth.rs                      # require_user, require_user_or_api_key
│   └── cors.rs                      # CORS configuration
│
├── utils/                           # Helpers réutilisables
│   ├── mod.rs
│   ├── hashing.rs                  # stable_hash_u64
│   ├── config.rs                   # openai_worker_stale_seconds_*
│   └── orchestrator.rs             # orchestrator_internal_url
│
└── [modules existants]              # Garder tels quels
    ├── auth.rs
    ├── auth_endpoints.rs
    ├── api_keys.rs
    ├── organizations.rs
    ├── finops.rs
    ├── workbench.rs
    ├── chat.rs
    ├── openai_proxy.rs
    ├── settings.rs
    ├── provider_settings.rs
    ├── instance_type_zones.rs
    ├── metrics.rs
    ├── users_endpoint.rs
    ├── worker_routing.rs
    ├── bootstrap_admin.rs
    ├── api_docs.rs
    └── simple_logger.rs
```

---

## 🔍 Analyse Détaillée par Domaine

### 1. Models Domain

**Fonctions à extraire**:
- `list_models()` (lignes ~1720-1762)
- `get_model()` (lignes ~1803-1822)
- `create_model()` (lignes ~1830-1860)
- `update_model()` (lignes ~1868-1911)
- `delete_model()` (lignes ~1918-1945)
- `list_compatible_models()` (lignes ~1774-1796)

**DTOs à créer**:
- `ListModelsParams`
- `CreateModelRequest`
- `UpdateModelRequest`

**Dépendances**:
- `AppState` (DB pool)
- `inventiv_common::LlmModel`

**Complexité**: Moyenne (CRUD standard)

---

### 2. Instances Domain

**Fonctions à extraire**:
- `list_instances()` (lignes ~2697-2774) - Query complexe avec JOINs
- `search_instances()` (lignes ~2782-2900) - Pagination, sorting
- `get_instance()` (lignes ~2918-3059) - Query détaillée
- `terminate_instance()` (lignes ~3122-3401) - Logique complexe avec Redis
- `archive_instance()` (lignes ~3060-3121)
- `reinstall_instance()` (lignes ~3402-3635) - Logique complexe

**DTOs à créer**:
- `ListInstanceParams`
- `SearchInstancesParams`
- `SearchInstancesResponse`
- `InstanceResponse` (déjà défini dans main.rs)

**Dépendances**:
- `AppState` (DB + Redis)
- `metrics::get_instance_metrics()` (déjà extrait)

**Complexité**: Élevée (queries complexes, logique métier importante)

---

### 3. Deployments Domain

**Fonctions à extraire**:
- `create_deployment()` (lignes ~1956-2601) - **~600 lignes !**

**Logique métier**:
1. Validation provider (code ou UUID)
2. Validation modèle
3. Validation instance_type
4. Création instance en DB (status: `provisioning`)
5. Publication événement Redis `CMD:PROVISION`
6. Calcul coût estimé
7. Retour réponse avec instance_id

**DTOs**:
- `DeploymentRequest` (déjà défini)
- `DeploymentResponse` (déjà défini)

**Dépendances**:
- `AppState` (DB + Redis)
- `orchestrator_internal_url()`

**Complexité**: Très élevée (logique métier critique, nombreuses validations)

**Recommandation**: Diviser en sous-fonctions dans `service.rs`:
- `validate_deployment_request()`
- `resolve_provider()`
- `create_instance_record()`
- `publish_provision_event()`

---

### 4. Observability Domain

**Fonctions à extraire**:
- `list_runtime_models()` (lignes ~450-561) - Agrégation SQL complexe
- `list_gpu_activity()` (lignes ~562-849) - Agrégation multi-instances
- `list_system_activity()` (lignes ~850-1059) - Agrégation CPU/Mem/Disk/Network

**DTOs**:
- `RuntimeModelRow` (déjà défini)
- `GpuActivityRow` (à identifier)
- `SystemActivityRow` (à identifier)

**Dépendances**:
- `AppState` (DB)
- Tables: `instances`, `instance_volumes`, etc.

**Complexité**: Moyenne-Élevée (queries d'agrégation complexes)

---

### 5. Action Logs Domain

**Fonctions à extraire**:
- `list_action_logs()` (lignes ~3636-3665)
- `list_action_types()` (lignes ~3688-3700)

**État actuel**:
- `action_logs_search.rs` existe déjà ✅
- `action_logs_endpoint.rs` existe mais semble inutilisé ?

**Recommandation**: Consolider dans `domains/action_logs/`

**DTOs**:
- `ActionLogQuery` (déjà défini)
- `ActionLogResponse` (déjà défini)
- `ActionTypeResponse` (déjà défini)

**Complexité**: Faible (CRUD simple)

---

### 6. Commands Domain

**Fonctions à extraire**:
- `manual_reconcile_trigger()` (lignes ~2612-2643)
- `manual_catalog_sync_trigger()` (lignes ~2654-2687)

**Logique métier**: Publication événements Redis vers `orchestrator_events`

**DTOs**: Réponses JSON simples

**Dépendances**:
- `AppState` (Redis)

**Complexité**: Faible (wrappers Redis pub)

**Recommandation**: Créer service générique `publish_orchestrator_command()` dans `service.rs`

---

### 7. Realtime Domain

**Fonctions à extraire**:
- `events_stream()` (lignes ~3727-3906) - **~180 lignes**

**Logique métier**:
1. SSE (Server-Sent Events) setup
2. Polling DB périodique (2s)
3. Signature tracking pour instances (éviter bruit heartbeats)
4. Tracking action_logs par timestamp
5. Émission événements: `instance.updated`, `action_log.created`

**DTOs**:
- `EventsStreamParams`
- `InstancesChangedPayload`
- `ActionLogsChangedPayload`

**Dépendances**:
- `AppState` (DB)
- Tokio streams, channels

**Complexité**: Élevée (asynchrone, stateful, optimisation signatures)

**Recommandation**: Extraire logique polling dans `service.rs`, garder handler léger

---

### 8. Worker Domain

**Fonctions à extraire**:
- `proxy_worker_register()` (lignes ~1285-1315)
- `proxy_worker_heartbeat()` (lignes ~1316-1343)
- `verify_worker_auth_api()` (lignes ~1210-1230)
- `verify_worker_token_db()` (lignes ~1179-1209)
- `proxy_post_to_orchestrator()` (lignes ~1231-1284)

**Logique métier**: Proxy avec auth worker vers orchestrator

**Dépendances**:
- `AppState` (DB + Redis)
- `orchestrator_internal_url()`

**Complexité**: Moyenne (auth + proxy)

**Recommandation**: Intégrer dans `worker_routing.rs` existant ou créer `domains/worker/`

---

## 🛠️ Fonctions Utilitaires à Extraire

### Dans `utils/`

1. **`utils/hashing.rs`**:
   - `stable_hash_u64()` (lignes ~390-395)

2. **`utils/config.rs`**:
   - `openai_worker_stale_seconds_env()` (lignes ~397-403)
   - `openai_worker_stale_seconds_db()` (lignes ~405-431)

3. **`utils/orchestrator.rs`**:
   - `orchestrator_internal_url()` (lignes ~371-378)

### Dans `infrastructure/`

1. **`infrastructure/database.rs`**:
   - `maybe_seed_catalog()` (lignes ~1344-1419)
   - `maybe_seed_provider_credentials()` (lignes ~1420-1655)

2. **`infrastructure/state.rs`**:
   - `AppState` struct (lignes ~52-56)

---

## 📋 Plan de Refactoring (Ordre Recommandé)

### Phase 1: Extraction Utilitaires (Low Risk)
1. ✅ Créer `utils/` et extraire fonctions utilitaires
2. ✅ Créer `infrastructure/` et extraire setup DB/Redis
3. ✅ Créer `middleware/` et extraire auth middleware

### Phase 2: Domaines Simples (Medium Risk)
1. ✅ Extraire **Commands** domain (~50 lignes)
2. ✅ Extraire **Action Logs** domain (consolider existants)
3. ✅ Extraire **Models** domain (CRUD standard)

### Phase 3: Domaines Complexes (Higher Risk)
1. ✅ Extraire **Observability** domain (queries complexes)
2. ✅ Extraire **Instances** domain (queries + logique métier)
3. ✅ Extraire **Realtime** domain (SSE, stateful)

### Phase 4: Domaines Critiques (Highest Risk)
1. ✅ Extraire **Deployments** domain (~600 lignes, logique critique)
2. ✅ Extraire **Worker** domain (proxy + auth)

### Phase 5: Nettoyage
1. ✅ Réduire `main.rs` à ~200 lignes (orchestration uniquement)
2. ✅ Créer `lib.rs` pour exports publics
3. ✅ Tests unitaires pour services extraits

---

## ⚠️ Points d'Attention

### 1. Dépendances Circulaires
- Éviter imports circulaires entre domains
- Utiliser `inventiv-common` pour types partagés
- `AppState` partagé via `State<Arc<AppState>>`

### 2. Tests
- Créer tests unitaires pour services extraits
- Tests d'intégration pour endpoints (via axum TestClient)

### 3. Documentation
- Documenter chaque domaine avec `//!` doc comments
- Maintenir Swagger/OpenAPI annotations (`#[utoipa::path]`)

### 4. Migration Progressive
- Extraire domaine par domaine
- Tester après chaque extraction
- Garder `main.rs` fonctionnel à chaque étape

### 5. Types Partagés
- DTOs dans `domains/{domain}/dto.rs`
- Types métier dans `inventiv-common` si partagés entre services

---

## 📊 Métriques Cibles

### Avant Refactoring
- `main.rs`: **3907 lignes**
- Fonctions métier dans main.rs: **~30+**
- Endpoints définis dans main.rs: **~20+**

### Après Refactoring
- `main.rs`: **~200 lignes** (orchestration uniquement)
- Domaines extraits: **8 domaines**
- Modules utilitaires: **3 modules**
- Maintenabilité: **+++**

---

## 🎯 Bénéfices Attendus

1. **Maintenabilité**: Code organisé par domaine métier
2. **Testabilité**: Services isolés, tests unitaires facilités
3. **Lisibilité**: `main.rs` devient un orchestrateur clair
4. **Évolutivité**: Ajout de nouveaux endpoints par domaine simplifié
5. **Réutilisabilité**: Services réutilisables entre endpoints
6. **Séparation des responsabilités**: Chaque module a une mission claire

---

## 📝 Notes de Migration

### Exemple: Extraction Models Domain

**Avant** (`main.rs`):
```rust
async fn list_models(...) -> impl IntoResponse {
    // 40 lignes de logique
}
```

**Après** (`domains/models/handlers.rs`):
```rust
use crate::domains::models::service::ModelsService;

pub async fn list_models(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListModelsParams>,
) -> impl IntoResponse {
    let service = ModelsService::new(state.db.clone());
    let models = service.list_models(params).await?;
    (StatusCode::OK, Json(models)).into_response()
}
```

**Après** (`domains/models/service.rs`):
```rust
pub struct ModelsService {
    db: Pool<Postgres>,
}

impl ModelsService {
    pub async fn list_models(&self, params: ListModelsParams) -> Result<Vec<LlmModel>> {
        // Logique métier extraite
    }
}
```

---

## ✅ Checklist de Validation

- [ ] Tous les endpoints extraits dans domaines appropriés
- [ ] `main.rs` réduit à ~200 lignes (orchestration)
- [ ] Pas de dépendances circulaires
- [ ] Tests unitaires pour services critiques
- [ ] Documentation Swagger maintenue
- [ ] Pas de régression fonctionnelle
- [ ] Code review par équipe
- [ ] Migration progressive validée

---

**Prochaine étape**: Valider cette analyse avec l'équipe avant de commencer le refactoring.


