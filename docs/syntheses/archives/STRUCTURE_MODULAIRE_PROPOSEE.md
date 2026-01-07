# Structure Modulaire Proposée - inventiv-api

**Date**: 2024  
**Objectif**: Vue d'ensemble visuelle de l'organisation modulaire proposée.

---

## 🏗️ Architecture Globale

```
inventiv-api/
├── src/
│   ├── main.rs                    # Orchestration (~200 lignes)
│   ├── lib.rs                     # Exports publics
│   │
│   ├── domains/                   # Domaines métier (DDD)
│   │   ├── models/
│   │   ├── instances/
│   │   ├── deployments/
│   │   ├── observability/
│   │   ├── action_logs/
│   │   ├── commands/
│   │   ├── realtime/
│   │   └── worker/
│   │
│   ├── infrastructure/            # Infrastructure & Setup
│   │   ├── database.rs
│   │   ├── redis.rs
│   │   ├── state.rs
│   │   └── config.rs
│   │
│   ├── middleware/                # Middleware Axum
│   │   ├── auth.rs
│   │   └── cors.rs
│   │
│   ├── utils/                     # Helpers réutilisables
│   │   ├── hashing.rs
│   │   ├── config.rs
│   │   └── orchestrator.rs
│   │
│   └── [modules existants]/       # Garder tels quels
│       ├── auth.rs
│       ├── auth_endpoints.rs
│       ├── api_keys.rs
│       ├── organizations.rs
│       ├── finops.rs
│       ├── workbench.rs
│       ├── chat.rs
│       ├── openai_proxy.rs
│       ├── settings.rs
│       ├── provider_settings.rs
│       ├── instance_type_zones.rs
│       ├── metrics.rs
│       ├── users_endpoint.rs
│       ├── worker_routing.rs
│       ├── bootstrap_admin.rs
│       ├── api_docs.rs
│       └── simple_logger.rs
```

---

## 📦 Détail des Domaines

### 1. `domains/models/` - Catalogue de Modèles LLM

```
domains/models/
├── mod.rs              # Exports publics
├── handlers.rs         # Endpoints handlers
├── service.rs          # Logique métier
└── dto.rs              # Request/Response DTOs
```

**Endpoints**:
- `GET /models` → `handlers::list_models`
- `POST /models` → `handlers::create_model`
- `GET /models/:id` → `handlers::get_model`
- `PUT /models/:id` → `handlers::update_model`
- `DELETE /models/:id` → `handlers::delete_model`
- `GET /instance_types/:id/models` → `handlers::list_compatible_models`

**Dépendances**:
- `AppState` (DB)
- `inventiv-common::LlmModel`

---

### 2. `domains/instances/` - Gestion des Instances GPU

```
domains/instances/
├── mod.rs              # Exports publics
├── handlers.rs         # Endpoints handlers
├── service.rs          # Logique métier (queries complexes)
└── dto.rs              # Request/Response DTOs
```

**Endpoints**:
- `GET /instances` → `handlers::list_instances`
- `GET /instances/search` → `handlers::search_instances`
- `GET /instances/:id` → `handlers::get_instance`
- `DELETE /instances/:id` → `handlers::terminate_instance`
- `PUT /instances/:id/archive` → `handlers::archive_instance`
- `POST /instances/:id/reinstall` → `handlers::reinstall_instance`

**Dépendances**:
- `AppState` (DB + Redis)
- `domains::metrics` (pour `/instances/:id/metrics`)

---

### 3. `domains/deployments/` - Déploiement de Modèles

```
domains/deployments/
├── mod.rs              # Exports publics
├── handlers.rs         # Endpoints handlers
├── service.rs          # Logique métier (validation, orchestration)
└── dto.rs              # Request/Response DTOs
```

**Endpoints**:
- `POST /deployments` → `handlers::create_deployment`

**Service Functions**:
- `validate_deployment_request()`
- `resolve_provider()`
- `create_instance_record()`
- `publish_provision_event()`

**Dépendances**:
- `AppState` (DB + Redis)
- `utils::orchestrator` (pour URL orchestrator)

---

### 4. `domains/observability/` - Métriques Temps-Réel

```
domains/observability/
├── mod.rs              # Exports publics
├── runtime_models.rs   # list_runtime_models handler
├── gpu_activity.rs     # list_gpu_activity handler
├── system_activity.rs  # list_system_activity handler
└── dto.rs              # Response DTOs
```

**Endpoints**:
- `GET /runtime/models` → `runtime_models::list_runtime_models`
- `GET /gpu/activity` → `gpu_activity::list_gpu_activity`
- `GET /system/activity` → `system_activity::list_system_activity`

**Dépendances**:
- `AppState` (DB)
- Tables: `instances`, `instance_volumes`

---

### 5. `domains/action_logs/` - Audit Trail

```
domains/action_logs/
├── mod.rs              # Exports publics
├── handlers.rs         # list_action_logs, list_action_types
├── search.rs           # search_action_logs (existant)
└── dto.rs              # Request/Response DTOs
```

**Endpoints**:
- `GET /action_logs` → `handlers::list_action_logs`
- `GET /action_logs/search` → `search::search_action_logs`
- `GET /action_types` → `handlers::list_action_types`

**Dépendances**:
- `AppState` (DB)

---

### 6. `domains/commands/` - Commandes Orchestrator

```
domains/commands/
├── mod.rs              # Exports publics
├── handlers.rs         # reconcile, catalog_sync
└── service.rs          # Redis event publishing
```

**Endpoints**:
- `POST /reconcile` → `handlers::manual_reconcile_trigger`
- `POST /catalog/sync` → `handlers::manual_catalog_sync_trigger`

**Service Functions**:
- `publish_orchestrator_command(command_type: &str)`

**Dépendances**:
- `AppState` (Redis)

---

### 7. `domains/realtime/` - Server-Sent Events

```
domains/realtime/
├── mod.rs              # Exports publics
├── handlers.rs         # events_stream handler
└── service.rs          # SSE logic, signature tracking
```

**Endpoints**:
- `GET /events/stream` → `handlers::events_stream`

**Service Functions**:
- `track_instance_changes()`
- `track_action_log_changes()`
- `compute_instance_signature()`

**Dépendances**:
- `AppState` (DB)
- Tokio streams, channels

---

### 8. `domains/worker/` - Worker Internal Routes

```
domains/worker/
├── mod.rs              # Exports publics
├── handlers.rs         # register, heartbeat (proxy)
└── service.rs          # Auth verification, proxy logic
```

**Endpoints**:
- `POST /internal/worker/register` → `handlers::proxy_worker_register`
- `POST /internal/worker/heartbeat` → `handlers::proxy_worker_heartbeat`

**Service Functions**:
- `verify_worker_auth()`
- `proxy_to_orchestrator()`

**Dépendances**:
- `AppState` (DB + Redis)
- `utils::orchestrator` (pour URL orchestrator)

---

## 🔧 Infrastructure

### `infrastructure/database.rs`
- Pool setup
- Migrations
- Seeds (`maybe_seed_catalog`, `maybe_seed_provider_credentials`)

### `infrastructure/redis.rs`
- Redis client setup
- Connection helpers

### `infrastructure/state.rs`
- `AppState` struct definition

### `infrastructure/config.rs`
- Environment variables
- Configuration structs

---

## 🛡️ Middleware

### `middleware/auth.rs`
- `require_user()` → `auth::require_user`
- `require_user_or_api_key()` → `auth::require_user_or_api_key`

### `middleware/cors.rs`
- CORS configuration

---

## 🧰 Utils

### `utils/hashing.rs`
- `stable_hash_u64()`

### `utils/config.rs`
- `openai_worker_stale_seconds_env()`
- `openai_worker_stale_seconds_db()`

### `utils/orchestrator.rs`
- `orchestrator_internal_url()`

---

## 📊 Graphique des Dépendances

```
main.rs
  ├── infrastructure/state.rs (AppState)
  ├── middleware/auth.rs
  ├── middleware/cors.rs
  │
  ├── domains/models/
  │   └── infrastructure/state.rs
  │
  ├── domains/instances/
  │   ├── infrastructure/state.rs
  │   └── domains/metrics/ (via metrics.rs existant)
  │
  ├── domains/deployments/
  │   ├── infrastructure/state.rs
  │   └── utils/orchestrator.rs
  │
  ├── domains/observability/
  │   └── infrastructure/state.rs
  │
  ├── domains/action_logs/
  │   └── infrastructure/state.rs
  │
  ├── domains/commands/
  │   └── infrastructure/state.rs
  │
  ├── domains/realtime/
  │   └── infrastructure/state.rs
  │
  ├── domains/worker/
  │   ├── infrastructure/state.rs
  │   └── utils/orchestrator.rs
  │
  └── [modules existants]/
      └── infrastructure/state.rs
```

**Règle**: Pas de dépendances circulaires entre domaines. Communication via `AppState` et `inventiv-common`.

---

## 🔄 Flux de Données Typique

### Exemple: Création d'un Deployment

```
1. Request → main.rs (router)
   ↓
2. Middleware auth → vérifie session
   ↓
3. domains/deployments/handlers.rs → create_deployment()
   ↓
4. domains/deployments/service.rs → validate_deployment_request()
   ↓
5. domains/deployments/service.rs → create_instance_record() (DB)
   ↓
6. domains/deployments/service.rs → publish_provision_event() (Redis)
   ↓
7. Response → JSON avec instance_id
```

---

## 📝 Exemple de Code: Avant/Après

### Avant (main.rs)

```rust
// main.rs - 3907 lignes
async fn create_deployment(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeploymentRequest>,
) -> impl IntoResponse {
    // 600 lignes de logique métier...
}
```

### Après (modulaire)

```rust
// main.rs - ~200 lignes
let deployments = Router::new()
    .route("/deployments", post(domains::deployments::handlers::create_deployment))
    .route_layer(middleware::from_fn(auth::require_user));

// domains/deployments/handlers.rs
pub async fn create_deployment(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeploymentRequest>,
) -> impl IntoResponse {
    let service = DeploymentsService::new(state.db.clone(), state.redis_client.clone());
    match service.create_deployment(payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// domains/deployments/service.rs
pub struct DeploymentsService {
    db: Pool<Postgres>,
    redis: redis::Client,
}

impl DeploymentsService {
    pub async fn create_deployment(&self, req: DeploymentRequest) -> Result<DeploymentResponse> {
        // Validation
        self.validate_deployment_request(&req).await?;
        
        // Création instance
        let instance_id = self.create_instance_record(&req).await?;
        
        // Publication événement
        self.publish_provision_event(&instance_id, &req).await?;
        
        Ok(DeploymentResponse { instance_id, status: "accepted" })
    }
    
    async fn validate_deployment_request(&self, req: &DeploymentRequest) -> Result<()> {
        // Logique de validation...
    }
    
    async fn create_instance_record(&self, req: &DeploymentRequest) -> Result<Uuid> {
        // Logique DB...
    }
    
    async fn publish_provision_event(&self, instance_id: &Uuid, req: &DeploymentRequest) -> Result<()> {
        // Logique Redis...
    }
}
```

---

## ✅ Avantages de cette Structure

1. **Séparation des responsabilités**: Chaque domaine a sa mission claire
2. **Testabilité**: Services isolés, tests unitaires facilités
3. **Maintenabilité**: Code organisé par domaine métier
4. **Évolutivité**: Ajout de nouveaux endpoints simplifié
5. **Réutilisabilité**: Services réutilisables entre endpoints
6. **Lisibilité**: `main.rs` devient un orchestrateur clair

---

## 🚀 Prochaines Étapes

1. ✅ Valider cette structure avec l'équipe
2. ✅ Commencer par Phase 1 (domaines simples)
3. ✅ Tester après chaque extraction
4. ✅ Documenter les services extraits
5. ✅ Ajouter tests unitaires

---

**Note**: Cette structure respecte les principes DDD et les bonnes pratiques Rust/Axum.


