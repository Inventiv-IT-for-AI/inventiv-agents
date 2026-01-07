// Integration tests for deployment endpoints
// IMPORTANT: All tests MUST use Mock provider only to avoid cloud costs

mod common;

use axum_test::TestServer;
use common::{
    create_test_app_service, create_test_organization, create_test_session, create_test_user,
    ensure_mock_provider, get_mock_instance_type_id, get_mock_zone_id, get_test_db_pool,
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_create_deployment_mock_only() {
    let app = create_test_app_service().await;
    // In axum-test 18, we need to use into_make_service() to convert Router<Arc<AppState>> to a service
    // But Router<Arc<AppState>> doesn't have into_make_service(), so we use the router directly
    // and TestServer will handle the conversion internally
    let server = TestServer::new(app).unwrap();

    let pool = get_test_db_pool().await;

    // Ensure Mock provider exists
    let mock_provider_id = ensure_mock_provider(&pool).await;

    // Get Mock zone and instance type
    let mock_zone_id = get_mock_zone_id(&pool)
        .await
        .expect("Mock zone should exist");
    let mock_instance_type_id = get_mock_instance_type_id(&pool)
        .await
        .expect("Mock instance type should exist");

    // Create a test model compatible with Mock instance type
    let model_id: Uuid = sqlx::query_scalar(
        "INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, created_at, updated_at)
         VALUES (gen_random_uuid(), 'Test Model', 'test-model', 1, 2048, true, NOW(), NOW())
         RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create test model");

    // Create test user and session
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    let session_token = create_test_session(&pool, user_id, None, None).await;

    // Get zone code
    let zone_code: String = sqlx::query_scalar("SELECT code FROM zones WHERE id = $1")
        .bind(mock_zone_id)
        .fetch_one(&pool)
        .await
        .expect("Failed to get zone code");

    // Get instance type code
    let instance_type_code: String =
        sqlx::query_scalar("SELECT code FROM instance_types WHERE id = $1")
            .bind(mock_instance_type_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to get instance type code");

    // Test deployment creation with Mock provider
    let response = server
        .post("/deployments")
        .add_header("Cookie", format!("inventiv_session={}", session_token))
        .json(&json!({
            "provider_code": "mock",  // MUST be Mock
            "zone": zone_code,
            "instance_type": instance_type_code,
            "model_id": model_id
        }))
        .await;

    assert_eq!(response.status_code(), 200);
    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "accepted");
    assert!(body["instance_id"].is_string());
}

#[tokio::test]
async fn test_create_deployment_rejects_non_mock_provider() {
    let app = create_test_app_service().await;
    // In axum-test 18, we need to use into_make_service() to convert Router<Arc<AppState>> to a service
    // But Router<Arc<AppState>> doesn't have into_make_service(), so we use the router directly
    // and TestServer will handle the conversion internally
    let server = TestServer::new(app).unwrap();

    let pool = get_test_db_pool().await;

    // Create test user and session
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    let session_token = create_test_session(&pool, user_id, None, None).await;

    // Try to create deployment with Scaleway provider (should fail in tests)
    let response = server
        .post("/deployments")
        .add_header("Cookie", format!("inventiv_session={}", session_token))
        .json(&json!({
            "provider_code": "scaleway",  // NOT Mock - should be rejected
            "zone": "fr-par-1",
            "instance_type": "H100-SXM-2-80G",
            "model_id": uuid::Uuid::new_v4()
        }))
        .await;

    // Should fail because Scaleway provider should not be used in tests
    // This ensures we don't accidentally provision real cloud resources
    assert!(
        response.status_code() != 200,
        "Deployment with non-Mock provider should be rejected in tests"
    );
}

#[tokio::test]
async fn test_create_deployment_missing_model() {
    let app = create_test_app_service().await;
    // In axum-test 18, we need to use into_make_service() to convert Router<Arc<AppState>> to a service
    // But Router<Arc<AppState>> doesn't have into_make_service(), so we use the router directly
    // and TestServer will handle the conversion internally
    let server = TestServer::new(app).unwrap();

    let pool = get_test_db_pool().await;

    // Ensure Mock provider exists
    ensure_mock_provider(&pool).await;

    // Get Mock zone and instance type
    let zone_code: String = sqlx::query_scalar(
        "SELECT code FROM zones z
         JOIN regions r ON r.id = z.region_id
         JOIN providers p ON p.id = r.provider_id
         WHERE p.code = 'mock' AND z.code = 'local' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to get Mock zone code");

    let instance_type_code: String = sqlx::query_scalar(
        "SELECT code FROM instance_types it
         JOIN providers p ON p.id = it.provider_id
         WHERE p.code = 'mock' AND it.code = 'mock-local-instance' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to get Mock instance type code");

    // Create test user and session
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    let session_token = create_test_session(&pool, user_id, None, None).await;

    // Test deployment creation without model_id
    let response = server
        .post("/deployments")
        .add_header("Cookie", format!("inventiv_session={}", session_token))
        .json(&json!({
            "provider_code": "mock",
            "zone": zone_code,
            "instance_type": instance_type_code
            // Missing model_id
        }))
        .await;

    assert_eq!(response.status_code(), 400);
    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "failed");
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("Missing model_id"));
}
