// Integration tests for authentication endpoints
mod common;

use axum_test::TestServer;
use common::{create_test_app_service, get_test_db_pool, create_test_user, create_test_session};
use serde_json::json;

#[tokio::test]
async fn test_login_success() {
    let app = create_test_app_service().await;
    let server = TestServer::new(app).unwrap();
    
    let pool = get_test_db_pool().await;
    
    // Create a test user
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    
    // Test login
    let response = server
        .post("/auth/login")
        .json(&json!({
            "email": "test_user@test.com",
            "password": "password123"
        }))
        .await;
    
    assert_eq!(response.status_code(), 200);
    let body: serde_json::Value = response.json();
    assert_eq!(body["email"], "test_user@test.com");
}

#[tokio::test]
async fn test_login_invalid_credentials() {
    let app = create_test_app_service().await;
    let server = TestServer::new(app).unwrap();
    
    let pool = get_test_db_pool().await;
    
    // Create a test user
    create_test_user(&pool, "test_user@test.com", "password123").await;
    
    // Test login with wrong password
    let response = server
        .post("/auth/login")
        .json(&json!({
            "email": "test_user@test.com",
            "password": "wrong_password"
        }))
        .await;
    
    assert_eq!(response.status_code(), 401);
}

#[tokio::test]
async fn test_me_endpoint() {
    let app = create_test_app_service().await;
    let server = TestServer::new(app).unwrap();
    
    let pool = get_test_db_pool().await;
    
    // Create a test user and session
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    let session_token = create_test_session(&pool, user_id, None, None).await;
    
    // Test /auth/me endpoint
    use axum::http::HeaderValue;
    let response = server
        .get("/auth/me")
        .add_header(axum::http::header::COOKIE, HeaderValue::from_str(&format!("inventiv_session={}", session_token)).unwrap())
        .await;
    
    assert_eq!(response.status_code(), 200);
    let body: serde_json::Value = response.json();
    assert_eq!(body["email"], "test_user@test.com");
}

#[tokio::test]
async fn test_logout() {
    let app = create_test_app_service().await;
    let server = TestServer::new(app).unwrap();
    
    let pool = get_test_db_pool().await;
    
    // Create a test user and session
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;
    let session_token = create_test_session(&pool, user_id, None, None).await;
    
    // Test logout
    use axum::http::HeaderValue;
    let response = server
        .post("/auth/logout")
        .add_header(axum::http::header::COOKIE, HeaderValue::from_str(&format!("inventiv_session={}", session_token)).unwrap())
        .await;
    
    assert_eq!(response.status_code(), 200);
}

