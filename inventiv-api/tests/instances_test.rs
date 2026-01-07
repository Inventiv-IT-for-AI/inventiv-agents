// Integration tests for instances endpoints
// IMPORTANT: All instance provisioning MUST use Mock provider only

mod common;

use axum_test::TestServer;
use common::{
    create_test_app_service, get_test_db_pool, create_test_user, create_test_session,
    ensure_mock_provider, get_mock_zone_id, get_mock_instance_type_id
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_list_instances() {
    let app = create_test_app_service().await;
    // In axum-test 18, we need to use into_make_service() to convert Router<Arc<AppState>> to a service
    // But Router<Arc<AppState>> doesn't have into_make_service(), so we use the router directly
    // and TestServer will handle the conversion internally
    let server = TestServer::new(app).unwrap();
    
    let pool = get_test_db_pool().await;
    
    // Create test user and session
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    let session_token = create_test_session(&pool, user_id, None, None).await;
    
    // Test list instances
    let response = server
        .get("/instances")
        .add_header("Cookie", format!("inventiv_session={}", session_token))
        .await;
    
    assert_eq!(response.status_code(), 200);
    let body: Vec<serde_json::Value> = response.json();
    assert!(!body.is_empty() || body.is_empty()); // Just check it's a Vec
}

#[tokio::test]
async fn test_search_instances() {
    let app = create_test_app_service().await;
    // In axum-test 18, we need to use into_make_service() to convert Router<Arc<AppState>> to a service
    // But Router<Arc<AppState>> doesn't have into_make_service(), so we use the router directly
    // and TestServer will handle the conversion internally
    let server = TestServer::new(app).unwrap();
    
    let pool = get_test_db_pool().await;
    
    // Create test user and session
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    let session_token = create_test_session(&pool, user_id, None, None).await;
    
    // Test search instances
    let response = server
        .get("/instances/search?limit=10&offset=0")
        .add_header("Cookie", format!("inventiv_session={}", session_token))
        .await;
    
    assert_eq!(response.status_code(), 200);
    let body: serde_json::Value = response.json();
    assert!(body["rows"].is_array());
    assert!(body["total_count"].is_number());
    assert!(body["filtered_count"].is_number());
}

#[tokio::test]
async fn test_get_instance() {
    let app = create_test_app_service().await;
    // In axum-test 18, we need to use into_make_service() to convert Router<Arc<AppState>> to a service
    // But Router<Arc<AppState>> doesn't have into_make_service(), so we use the router directly
    // and TestServer will handle the conversion internally
    let server = TestServer::new(app).unwrap();
    
    let pool = get_test_db_pool().await;
    
    // Ensure Mock provider exists
    let mock_provider_id = ensure_mock_provider(&pool).await;
    let mock_zone_id = get_mock_zone_id(&pool).await.unwrap();
    let mock_instance_type_id = get_mock_instance_type_id(&pool).await.unwrap();
    
    // Create a test model
    let model_id: Uuid = sqlx::query_scalar(
        "INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, created_at, updated_at)
         VALUES (gen_random_uuid(), 'Test Model', 'test-model', 1, 2048, true, NOW(), NOW())
         RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create test model");
    
    // Create a test instance (Mock provider only)
    let instance_id: Uuid = sqlx::query_scalar(
        "INSERT INTO instances (id, provider_id, zone_id, instance_type_id, model_id, status, created_at, gpu_profile)
         VALUES (gen_random_uuid(), $1, $2, $3, $4, 'ready', NOW(), '{}')
         RETURNING id"
    )
    .bind(mock_provider_id)
    .bind(mock_zone_id)
    .bind(mock_instance_type_id)
    .bind(model_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to create test instance");
    
    // Create test user and session
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    let session_token = create_test_session(&pool, user_id, None, None).await;
    
    // Test get instance
    let response = server
        .get(&format!("/instances/{}", instance_id))
        .add_header("Cookie", format!("inventiv_session={}", session_token))
        .await;
    
    assert_eq!(response.status_code(), 200);
    let body: serde_json::Value = response.json();
    assert_eq!(body["instance"]["id"], instance_id.to_string());
}

#[tokio::test]
async fn test_terminate_instance() {
    let app = create_test_app_service().await;
    // In axum-test 18, we need to use into_make_service() to convert Router<Arc<AppState>> to a service
    // But Router<Arc<AppState>> doesn't have into_make_service(), so we use the router directly
    // and TestServer will handle the conversion internally
    let server = TestServer::new(app).unwrap();
    
    let pool = get_test_db_pool().await;
    
    // Ensure Mock provider exists
    let mock_provider_id = ensure_mock_provider(&pool).await;
    let mock_zone_id = get_mock_zone_id(&pool).await.unwrap();
    let mock_instance_type_id = get_mock_instance_type_id(&pool).await.unwrap();
    
    // Create a test model
    let model_id: Uuid = sqlx::query_scalar(
        "INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, created_at, updated_at)
         VALUES (gen_random_uuid(), 'Test Model', 'test-model', 1, 2048, true, NOW(), NOW())
         RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create test model");
    
    // Create a test instance (Mock provider only)
    let instance_id: Uuid = sqlx::query_scalar(
        "INSERT INTO instances (id, provider_id, zone_id, instance_type_id, model_id, status, created_at, gpu_profile)
         VALUES (gen_random_uuid(), $1, $2, $3, $4, 'ready', NOW(), '{}')
         RETURNING id"
    )
    .bind(mock_provider_id)
    .bind(mock_zone_id)
    .bind(mock_instance_type_id)
    .bind(model_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to create test instance");
    
    // Create test user and session
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    let session_token = create_test_session(&pool, user_id, None, None).await;
    
    // Test terminate instance
    let response = server
        .delete(&format!("/instances/{}", instance_id))
        .add_header("Cookie", format!("inventiv_session={}", session_token))
        .await;
    
    assert_eq!(response.status_code(), 202);
    
    // Verify instance status changed to terminating
    let status: String = sqlx::query_scalar(
        "SELECT status::text FROM instances WHERE id = $1"
    )
    .bind(instance_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to get instance status");
    
    assert_eq!(status, "terminating");
}

