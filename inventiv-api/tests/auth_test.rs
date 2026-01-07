// Integration tests for authentication endpoints
mod common;

use axum_test::TestServer;
use common::{create_test_app_service, create_test_session, create_test_user, get_test_db_pool};
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
        .add_header(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("inventiv_session={}", session_token)).unwrap(),
        )
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
        .add_header(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("inventiv_session={}", session_token)).unwrap(),
        )
        .await;

    assert_eq!(response.status_code(), 200);
}

#[tokio::test]
async fn test_list_sessions() {
    let app = create_test_app_service().await;
    let server = TestServer::new(app).unwrap();

    let pool = get_test_db_pool().await;

    // Create a test user
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;

    // Create an organization
    let org_id = uuid::Uuid::new_v4();
    let _ = sqlx::query(
        r#"
        INSERT INTO organizations (id, name, slug, created_by_user_id)
        VALUES ($1, 'Test Org', 'test-org', $2)
        ON CONFLICT (slug) DO UPDATE SET updated_at = NOW()
        "#,
    )
    .bind(org_id)
    .bind(user_id)
    .execute(&pool)
    .await;

    // Create membership
    let _ = sqlx::query(
        r#"
        INSERT INTO organization_memberships (organization_id, user_id, role)
        VALUES ($1, $2, 'owner')
        ON CONFLICT (organization_id, user_id) DO UPDATE SET role = 'owner'
        "#,
    )
    .bind(org_id)
    .bind(user_id)
    .execute(&pool)
    .await;

    // Create first session (current) with organization
    let session_id_1 = uuid::Uuid::new_v4();
    use inventiv_api::auth::{create_session, hash_session_token, sign_session_jwt, AuthUser};
    let auth_user_1 = AuthUser {
        user_id,
        email: "test_user@test.com".to_string(),
        role: "user".to_string(),
        session_id: session_id_1.to_string(),
        current_organization_id: Some(org_id),
        current_organization_role: Some("owner".to_string()),
    };
    let session_token = sign_session_jwt(&auth_user_1).unwrap();
    let token_hash_1 = hash_session_token(&session_token);
    let _ = create_session(
        &pool,
        session_id_1,
        user_id,
        Some(org_id),
        Some("owner".to_string()),
        Some("127.0.0.1".to_string()),
        Some("test".to_string()),
        token_hash_1,
    )
    .await;

    // Create second session for the same user (without org)
    let session_id_2 = uuid::Uuid::new_v4();
    let auth_user_2 = AuthUser {
        user_id,
        email: "test_user@test.com".to_string(),
        role: "user".to_string(),
        session_id: session_id_2.to_string(),
        current_organization_id: None,
        current_organization_role: None,
    };
    let session_token_2 = sign_session_jwt(&auth_user_2).unwrap();
    let token_hash_2 = hash_session_token(&session_token_2);
    let _ = create_session(
        &pool,
        session_id_2,
        user_id,
        None,
        None,
        Some("192.168.1.1".to_string()),
        Some("test2".to_string()),
        token_hash_2,
    )
    .await;

    // Test GET /auth/sessions
    use axum::http::HeaderValue;
    let response = server
        .get("/auth/sessions")
        .add_header(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("inventiv_session={}", session_token)).unwrap(),
        )
        .await;

    assert_eq!(response.status_code(), 200);
    let body: serde_json::Value = response.json();
    assert!(body.is_array());
    let sessions = body.as_array().unwrap();
    assert!(sessions.len() >= 2); // At least 2 sessions (current + second)

    // Verify that one session is marked as current
    let has_current = sessions.iter().any(|s| s["is_current"] == true);
    assert!(has_current);
}

#[tokio::test]
async fn test_revoke_session() {
    let app = create_test_app_service().await;
    let server = TestServer::new(app).unwrap();

    let pool = get_test_db_pool().await;

    // Create a test user
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;

    // Create first session (current)
    let session_id_1 = uuid::Uuid::new_v4();
    use inventiv_api::auth::{create_session, hash_session_token, sign_session_jwt, AuthUser};
    let auth_user_1 = AuthUser {
        user_id,
        email: "test_user@test.com".to_string(),
        role: "user".to_string(),
        session_id: session_id_1.to_string(),
        current_organization_id: None,
        current_organization_role: None,
    };
    let session_token = sign_session_jwt(&auth_user_1).unwrap();
    let token_hash_1 = hash_session_token(&session_token);
    let _ = create_session(
        &pool,
        session_id_1,
        user_id,
        None,
        None,
        Some("127.0.0.1".to_string()),
        Some("test".to_string()),
        token_hash_1,
    )
    .await;

    // Create a second session for the same user
    let session_id_2 = uuid::Uuid::new_v4();
    let auth_user_2 = AuthUser {
        user_id,
        email: "test_user@test.com".to_string(),
        role: "user".to_string(),
        session_id: session_id_2.to_string(),
        current_organization_id: None,
        current_organization_role: None,
    };
    let session_token_2 = sign_session_jwt(&auth_user_2).unwrap();
    let token_hash_2 = hash_session_token(&session_token_2);
    let _ = create_session(
        &pool,
        session_id_2,
        user_id,
        None,
        None,
        Some("192.168.1.1".to_string()),
        Some("test2".to_string()),
        token_hash_2,
    )
    .await;

    // Test POST /auth/sessions/:id/revoke
    use axum::http::HeaderValue;
    let response = server
        .post(&format!("/auth/sessions/{}/revoke", session_id_2))
        .add_header(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("inventiv_session={}", session_token)).unwrap(),
        )
        .await;

    assert_eq!(response.status_code(), 200);
    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "ok");

    // Verify that the session is revoked
    let revoked: Option<bool> =
        sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM user_sessions WHERE id = $1")
            .bind(session_id_2)
            .fetch_optional(&pool)
            .await
            .unwrap()
            .map(|r: bool| r);

    assert_eq!(revoked, Some(true));
}

#[tokio::test]
async fn test_revoke_current_session_forbidden() {
    let app = create_test_app_service().await;
    let server = TestServer::new(app).unwrap();

    let pool = get_test_db_pool().await;

    // Create a test user
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;

    // Create session (current)
    let session_id_1 = uuid::Uuid::new_v4();
    use inventiv_api::auth::{create_session, hash_session_token, sign_session_jwt, AuthUser};
    let auth_user_1 = AuthUser {
        user_id,
        email: "test_user@test.com".to_string(),
        role: "user".to_string(),
        session_id: session_id_1.to_string(),
        current_organization_id: None,
        current_organization_role: None,
    };
    let session_token = sign_session_jwt(&auth_user_1).unwrap();
    let token_hash_1 = hash_session_token(&session_token);
    let _ = create_session(
        &pool,
        session_id_1,
        user_id,
        None,
        None,
        Some("127.0.0.1".to_string()),
        Some("test".to_string()),
        token_hash_1,
    )
    .await;

    let current_session_id = session_id_1;

    // Try to revoke the current session (should fail)
    use axum::http::HeaderValue;
    let response = server
        .post(&format!("/auth/sessions/{}/revoke", current_session_id))
        .add_header(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("inventiv_session={}", session_token)).unwrap(),
        )
        .await;

    assert_eq!(response.status_code(), 400);
    let body: serde_json::Value = response.json();
    assert_eq!(body["error"], "cannot_revoke_current_session");
}

#[tokio::test]
async fn test_revoke_other_user_session_forbidden() {
    let app = create_test_app_service().await;
    let server = TestServer::new(app).unwrap();

    let pool = get_test_db_pool().await;

    // Create two test users
    let user_id_1 = create_test_user(&pool, "user1@test.com", "password123").await;
    let user_id_2 = create_test_user(&pool, "user2@test.com", "password123").await;

    // Create session for user 1
    let session_id_1 = uuid::Uuid::new_v4();
    use inventiv_api::auth::{create_session, hash_session_token, sign_session_jwt, AuthUser};
    let auth_user_1 = AuthUser {
        user_id: user_id_1,
        email: "user1@test.com".to_string(),
        role: "user".to_string(),
        session_id: session_id_1.to_string(),
        current_organization_id: None,
        current_organization_role: None,
    };
    let session_token_1 = sign_session_jwt(&auth_user_1).unwrap();
    let token_hash_1 = hash_session_token(&session_token_1);
    let _ = create_session(
        &pool,
        session_id_1,
        user_id_1,
        None,
        None,
        Some("127.0.0.1".to_string()),
        Some("test".to_string()),
        token_hash_1,
    )
    .await;

    // Create session for user 2
    let session_id_2 = uuid::Uuid::new_v4();
    let auth_user_2 = AuthUser {
        user_id: user_id_2,
        email: "user2@test.com".to_string(),
        role: "user".to_string(),
        session_id: session_id_2.to_string(),
        current_organization_id: None,
        current_organization_role: None,
    };
    let session_token_2 = sign_session_jwt(&auth_user_2).unwrap();
    let token_hash_2 = hash_session_token(&session_token_2);
    let _ = create_session(
        &pool,
        session_id_2,
        user_id_2,
        None,
        None,
        Some("192.168.1.1".to_string()),
        Some("test2".to_string()),
        token_hash_2,
    )
    .await;

    // Try to revoke user 2's session as user 1 (should fail)
    use axum::http::HeaderValue;
    let response = server
        .post(&format!("/auth/sessions/{}/revoke", session_id_2))
        .add_header(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("inventiv_session={}", session_token_1)).unwrap(),
        )
        .await;

    assert_eq!(response.status_code(), 403);
    let body: serde_json::Value = response.json();
    assert_eq!(body["error"], "forbidden");
}

#[tokio::test]
async fn test_me_endpoint_includes_organization_role() {
    let app = create_test_app_service().await;
    let server = TestServer::new(app).unwrap();

    let pool = get_test_db_pool().await;

    // Create a test user
    let user_id = create_test_user(&pool, "test_user@test.com", "password123").await;

    // Create an organization
    let org_id = uuid::Uuid::new_v4();
    let _ = sqlx::query(
        r#"
        INSERT INTO organizations (id, name, slug, created_by_user_id)
        VALUES ($1, 'Test Org', 'test-org', $2)
        ON CONFLICT (slug) DO UPDATE SET updated_at = NOW()
        "#,
    )
    .bind(org_id)
    .bind(user_id)
    .execute(&pool)
    .await;

    // Create membership with role 'admin'
    let _ = sqlx::query(
        r#"
        INSERT INTO organization_memberships (organization_id, user_id, role)
        VALUES ($1, $2, 'admin')
        ON CONFLICT (organization_id, user_id) DO UPDATE SET role = 'admin'
        "#,
    )
    .bind(org_id)
    .bind(user_id)
    .execute(&pool)
    .await;

    // Create session with organization
    let session_id = uuid::Uuid::new_v4();
    let token_hash = "test_hash";
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(12);
    let _ = sqlx::query(
        r#"
        INSERT INTO user_sessions (id, user_id, current_organization_id, organization_role, session_token_hash, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(org_id)
    .bind("admin")
    .bind(token_hash)
    .bind(expires_at)
    .execute(&pool)
    .await;

    // Generate JWT token for this session
    use inventiv_api::auth::{create_session, hash_session_token, sign_session_jwt, AuthUser};
    let auth_user = AuthUser {
        user_id,
        email: "test_user@test.com".to_string(),
        role: "user".to_string(),
        session_id: session_id.to_string(),
        current_organization_id: Some(org_id),
        current_organization_role: Some("admin".to_string()),
    };
    let session_token = sign_session_jwt(&auth_user).unwrap();
    let token_hash = hash_session_token(&session_token);

    // Create session in DB
    let _ = create_session(
        &pool,
        session_id,
        user_id,
        Some(org_id),
        Some("admin".to_string()),
        Some("127.0.0.1".to_string()),
        Some("test".to_string()),
        token_hash,
    )
    .await;

    // Test /auth/me endpoint
    use axum::http::HeaderValue;
    let response = server
        .get("/auth/me")
        .add_header(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("inventiv_session={}", session_token)).unwrap(),
        )
        .await;

    assert_eq!(response.status_code(), 200);
    let body: serde_json::Value = response.json();
    assert_eq!(body["current_organization_role"], "admin");
    assert_eq!(body["current_organization_id"], org_id.to_string());
}
