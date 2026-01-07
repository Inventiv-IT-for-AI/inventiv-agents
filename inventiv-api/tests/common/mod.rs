// Common test utilities and fixtures
use axum::Router;
use inventiv_api::app::AppState;
use inventiv_api::bootstrap_admin;
use inventiv_api::config::{database::create_pool, redis::create_client};
use inventiv_api::routes::{create_router, openai, protected, public, workbench, worker};
use inventiv_api::setup::{maybe_seed_catalog, maybe_seed_provider_credentials, run_migrations};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::sync::Arc;
use tokio::sync::OnceCell;

// E2E test helpers (for testing against real Docker containers)
#[cfg(test)]
pub mod e2e;

static TEST_DB_POOL: OnceCell<Pool<Postgres>> = OnceCell::const_new();
static TEST_REDIS_CLIENT: OnceCell<redis::Client> = OnceCell::const_new();

/// Get or create a test database pool (singleton for all tests)
pub async fn get_test_db_pool() -> Pool<Postgres> {
    TEST_DB_POOL
        .get_or_init(|| async {
            let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
                "postgresql://postgres:password@localhost:5432/inventiv_test".to_string()
            });

            // Use a larger pool for tests to avoid timeouts when running tests in parallel
            let pool = PgPoolOptions::new()
                .max_connections(20)
                .connect(&database_url)
                .await
                .expect("Failed to create test database pool");

            // Run migrations
            run_migrations(&pool)
                .await
                .expect("Failed to run migrations on test database");

            // Seed catalog (includes Mock provider)
            maybe_seed_catalog(&pool).await;
            maybe_seed_provider_credentials(&pool).await;

            // Bootstrap admin and default organization
            bootstrap_admin::ensure_default_admin(&pool).await;
            bootstrap_admin::ensure_default_organization(&pool).await;

            pool
        })
        .await
        .clone()
}

/// Get or create a test Redis client (singleton for all tests)
pub async fn get_test_redis_client() -> redis::Client {
    TEST_REDIS_CLIENT
        .get_or_init(|| async {
            let redis_url = std::env::var("TEST_REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379/1".to_string());

            create_client(&redis_url).expect("Failed to create test Redis client")
        })
        .await
        .clone()
}

/// Create a test application router
pub async fn create_test_app() -> Router<Arc<AppState>> {
    let db_pool = get_test_db_pool().await;
    let redis_client = get_test_redis_client().await;
    let state = AppState::new(redis_client, db_pool);
    let app = create_router(state);
    // Apply CORS like in main.rs
    let cors = inventiv_api::app::create_cors();
    app.layer(cors)
}

/// Create a test application service for TestServer
/// Returns a Router without state that implements IntoTransportLayer
/// In axum-test 18, TestServer::new() accepts Router (without state) directly
/// We create the router without state, then add state via with_state()
pub async fn create_test_app_service() -> Router {
    let db_pool = get_test_db_pool().await;
    let redis_client = get_test_redis_client().await;
    let state = AppState::new(redis_client, db_pool);

    // Create router without state first
    let app = Router::new()
        .merge(public::create_public_routes())
        .merge(worker::create_worker_routes())
        .merge(openai::create_openai_routes(state.clone()))
        .merge(workbench::create_workbench_routes(state.clone()))
        .merge(protected::create_protected_routes(state.clone()));

    // Apply CORS
    let cors = inventiv_api::app::create_cors();
    app.layer(cors).with_state(state)
}

/// Clean up test data (optional, can be called between tests)
pub async fn cleanup_test_data(pool: &Pool<Postgres>) {
    // Clean up instances (terminate any active ones)
    sqlx::query("UPDATE instances SET status = 'terminated', terminated_at = NOW() WHERE status != 'terminated'")
        .execute(pool)
        .await
        .ok();

    // Clean up test users (except admin)
    sqlx::query("DELETE FROM users WHERE email LIKE 'test_%@test.com'")
        .execute(pool)
        .await
        .ok();

    // Clean up test organizations (except default)
    sqlx::query("DELETE FROM organizations WHERE slug LIKE 'test-%' AND slug != 'inventiv-it'")
        .execute(pool)
        .await
        .ok();

    // Clean up test API keys
    sqlx::query("DELETE FROM api_keys WHERE name LIKE 'test-%'")
        .execute(pool)
        .await
        .ok();
}

/// Ensure Mock provider exists and is active
pub async fn ensure_mock_provider(pool: &Pool<Postgres>) -> uuid::Uuid {
    let provider_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO providers (id, name, code, description, is_active)
         VALUES (gen_random_uuid(), 'Mock', 'mock', 'Mock provider for testing', true)
         ON CONFLICT (code) DO UPDATE SET is_active = true
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("Failed to ensure Mock provider");

    provider_id
}

/// Get Mock provider ID
pub async fn get_mock_provider_id(pool: &Pool<Postgres>) -> Option<uuid::Uuid> {
    sqlx::query_scalar("SELECT id FROM providers WHERE code = 'mock' AND is_active = true")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// Get Mock zone ID (local zone)
pub async fn get_mock_zone_id(pool: &Pool<Postgres>) -> Option<uuid::Uuid> {
    sqlx::query_scalar(
        "SELECT z.id FROM zones z
         JOIN regions r ON r.id = z.region_id
         JOIN providers p ON p.id = r.provider_id
         WHERE p.code = 'mock' AND z.code = 'local' AND z.is_active = true
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// Get Mock instance type ID
pub async fn get_mock_instance_type_id(pool: &Pool<Postgres>) -> Option<uuid::Uuid> {
    sqlx::query_scalar(
        "SELECT it.id FROM instance_types it
         JOIN providers p ON p.id = it.provider_id
         WHERE p.code = 'mock' AND it.code = 'mock-local-instance' AND it.is_active = true
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// Create a test user and return user ID and password hash
/// Automatically cleans up any existing user with the same email first
pub async fn create_test_user(pool: &Pool<Postgres>, email: &str, password: &str) -> uuid::Uuid {
    use bcrypt::{hash, DEFAULT_COST};

    // Clean up any existing user with this email first
    let _ = sqlx::query(
        "DELETE FROM user_sessions WHERE user_id IN (SELECT id FROM users WHERE email = $1)",
    )
    .bind(email)
    .execute(pool)
    .await;
    let _ = sqlx::query("DELETE FROM organization_memberships WHERE user_id IN (SELECT id FROM users WHERE email = $1)")
        .bind(email)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await;

    let password_hash = hash(password, DEFAULT_COST).expect("Failed to hash test password");

    // Extract username from email (before @)
    let username = email.split('@').next().unwrap_or(email);

    let user_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (id, email, password_hash, username, created_at)
         VALUES (gen_random_uuid(), $1, $2, $3, NOW())
         RETURNING id",
    )
    .bind(email)
    .bind(password_hash)
    .bind(username)
    .fetch_one(pool)
    .await
    .expect("Failed to create test user");

    user_id
}

/// Create a test session token for a user
/// NOTE: This creates a simple token string, NOT a JWT. For JWT-based sessions,
/// use inventiv_api::auth::{sign_session_jwt, hash_session_token, create_session} directly.
pub async fn create_test_session(
    pool: &Pool<Postgres>,
    user_id: uuid::Uuid,
    organization_id: Option<uuid::Uuid>,
    role: Option<&str>,
) -> String {
    use sha2::{Digest, Sha256};

    let session_token = format!("test_session_{}", uuid::Uuid::new_v4());
    // Use hex format to match hash_session_token() behavior
    let mut hasher = Sha256::new();
    hasher.update(session_token.as_bytes());
    let token_hash = format!("{:x}", hasher.finalize());

    sqlx::query(
        "INSERT INTO user_sessions (id, user_id, current_organization_id, organization_role, session_token_hash, ip_address, user_agent, created_at, last_used_at, expires_at)
         VALUES (gen_random_uuid(), $1, $2, $3, $4, '127.0.0.1', 'test', NOW(), NOW(), NOW() + INTERVAL '24 hours')
         RETURNING id"
    )
    .bind(user_id)
    .bind(organization_id)
    .bind(role)
    .bind(token_hash)
    .fetch_one(pool)
    .await
    .expect("Failed to create test session");

    session_token
}

/// Create a test organization
pub async fn create_test_organization(
    pool: &Pool<Postgres>,
    name: &str,
    slug: &str,
    owner_id: uuid::Uuid,
) -> uuid::Uuid {
    let org_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO organizations (id, name, slug, created_at)
         VALUES (gen_random_uuid(), $1, $2, NOW())
         RETURNING id",
    )
    .bind(name)
    .bind(slug)
    .fetch_one(pool)
    .await
    .expect("Failed to create test organization");

    // Add owner membership
    sqlx::query(
        "INSERT INTO organization_memberships (id, organization_id, user_id, role, created_at)
         VALUES (gen_random_uuid(), $1, $2, 'owner', NOW())
         ON CONFLICT (organization_id, user_id) DO UPDATE SET role = 'owner'",
    )
    .bind(org_id)
    .bind(owner_id)
    .execute(pool)
    .await
    .expect("Failed to add owner to organization");

    org_id
}

/// Ensure only Mock provider is active for testing (deactivate others)
/// This prevents accidental provisioning of real cloud resources
pub async fn enforce_mock_only_provider(pool: &Pool<Postgres>) {
    // Deactivate all providers except Mock
    sqlx::query("UPDATE providers SET is_active = false WHERE code != 'mock'")
        .execute(pool)
        .await
        .expect("Failed to deactivate non-Mock providers");

    // Ensure Mock provider is active
    ensure_mock_provider(pool).await;
}

/// Verify that a deployment request uses Mock provider only
pub fn assert_mock_provider_only(request: &serde_json::Value) {
    if let Some(provider_code) = request.get("provider_code").and_then(|v| v.as_str()) {
        assert_eq!(
            provider_code, "mock",
            "Tests MUST use Mock provider only. Found: {}",
            provider_code
        );
    }

    // If provider_id is specified, verify it's Mock
    if let Some(provider_id) = request.get("provider_id").and_then(|v| v.as_str()) {
        // We can't verify UUID without DB lookup, but we can log a warning
        // In practice, tests should use provider_code = "mock"
        eprintln!("WARNING: Test uses provider_id instead of provider_code. Ensure it's Mock provider UUID.");
    }
}
