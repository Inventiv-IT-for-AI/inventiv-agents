//! Tests d'intégration end-to-end contre l'API réelle dans les containers Docker
//!
//! Ces tests nécessitent que les containers Docker soient démarrés :
//! ```bash
//! make up  # Démarre db, redis, api, orchestrator
//! ```
//!
//! Les tests se connectent à l'API réelle via HTTP sur http://localhost:8003
//!
//! Variables d'environnement :
//! - `TEST_API_URL` : URL de l'API (défaut: http://localhost:8003)
//! - `TEST_ADMIN_EMAIL` : Email de l'admin (défaut: admin@inventiv.local)
//! - `TEST_ADMIN_PASSWORD` : Mot de passe de l'admin (défaut: lit depuis /run/secrets/default_admin_password)

mod common;

use common::e2e::create_test_client;
use serde_json::json;

#[tokio::test]
async fn test_e2e_login() {
    let client = create_test_client();

    // Test login
    let response = client
        .post("/auth/login")
        .json(&json!({
            "email": client.admin_email(),
            "password": client.admin_password()
        }))
        .send()
        .await
        .expect("Failed to send login request");

    assert_eq!(response.status(), 200, "Login should succeed");
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["email"], client.admin_email());
}

#[tokio::test]
async fn test_e2e_me_endpoint() {
    let client = create_test_client();
    client.login_with_cookies().await;

    // Test /auth/me endpoint (uses cookie auth automatically)
    let response = client
        .get("/auth/me")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200, "Should get user info");
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["email"], client.admin_email());
}

#[tokio::test]
async fn test_e2e_list_instances() {
    let client = create_test_client();
    client.login_with_cookies().await;

    // Test list instances (uses cookie auth automatically)
    let response = client
        .get("/instances")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200, "Should list instances");
    let body: Vec<serde_json::Value> = response.json().await.expect("Failed to parse response");
    assert!(body.is_empty() || !body.is_empty()); // Just check it's an array
}

#[tokio::test]
async fn test_e2e_list_organizations() {
    let client = create_test_client();
    client.login_with_cookies().await;

    // Test list organizations (uses cookie auth automatically)
    let response = client
        .get("/organizations")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200, "Should list organizations");
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["rows"].is_array());
}

#[tokio::test]
async fn test_e2e_openai_models() {
    let client = create_test_client();

    // Test OpenAI-compatible /v1/models endpoint (public, no auth required)
    let response = client
        .get("/v1/models")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200, "Should list models");
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn test_e2e_health_check() {
    let client = create_test_client();

    // Test root endpoint
    let response = client
        .get("/")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200, "Root endpoint should respond");
    let text = response.text().await.expect("Failed to read response");
    assert!(text.contains("Inventiv"));
}
