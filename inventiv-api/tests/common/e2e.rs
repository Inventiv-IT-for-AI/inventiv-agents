//! Helpers pour les tests d'intégration end-to-end contre l'API réelle

use reqwest::Client;
use serde_json::json;
use std::sync::OnceLock;
use std::time::Duration;

static TEST_CLIENT: OnceLock<TestClient> = OnceLock::new();

/// Client HTTP pour les tests E2E
pub struct TestClient {
    http_client: Client,
    api_url: String,
    admin_email: String,
    admin_password: String,
}

impl TestClient {
    pub fn new() -> Self {
        let api_url = std::env::var("TEST_API_URL")
            .unwrap_or_else(|_| "http://localhost:8003".to_string());
        
        let admin_email = std::env::var("TEST_ADMIN_EMAIL")
            .unwrap_or_else(|_| {
                // Try to read from env file or use default
                std::fs::read_to_string("env/dev.env")
                    .ok()
                    .and_then(|content| {
                        content.lines()
                            .find(|line| line.starts_with("DEFAULT_ADMIN_EMAIL="))
                            .and_then(|line| {
                                let parts: Vec<&str> = line.splitn(2, '=').collect();
                                if parts.len() == 2 {
                                    Some(parts[1].trim().trim_matches('"').to_string())
                                } else {
                                    None
                                }
                            })
                    })
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "admin@inventiv.local".to_string())
            });
        
        // Try to read admin password from file (like the API does)
        let admin_password = std::env::var("TEST_ADMIN_PASSWORD")
            .unwrap_or_else(|_| {
                // Try reading from the same path as the API container
                std::fs::read_to_string("/run/secrets/default_admin_password")
                    .or_else(|_| std::fs::read_to_string("./deploy/secrets-dev/default_admin_password"))
                    .unwrap_or_else(|_| "admin".to_string())
                    .trim()
                    .to_string()
            });
        
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .cookie_store(true)
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            http_client,
            api_url,
            admin_email,
            admin_password,
        }
    }
    
    pub fn http_client(&self) -> &Client {
        &self.http_client
    }
    
    pub fn api_url(&self) -> &str {
        &self.api_url
    }
    
    pub fn admin_email(&self) -> &str {
        &self.admin_email
    }
    
    pub fn admin_password(&self) -> &str {
        &self.admin_password
    }
    
    /// Créer une requête GET
    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.http_client.get(format!("{}{}", self.api_url, path))
    }
    
    /// Créer une requête POST
    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.http_client.post(format!("{}{}", self.api_url, path))
    }
    
    /// Créer une requête PUT
    pub fn put(&self, path: &str) -> reqwest::RequestBuilder {
        self.http_client.put(format!("{}{}", self.api_url, path))
    }
    
    /// Créer une requête DELETE
    pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        self.http_client.delete(format!("{}{}", self.api_url, path))
    }
    
    /// Login et retourner le token de session (depuis le cookie)
    pub async fn login(&self) -> String {
        let response = self
            .post("/auth/login")
            .json(&json!({
                "email": self.admin_email,
                "password": self.admin_password
            }))
            .send()
            .await
            .expect("Failed to login");
        
        assert_eq!(response.status(), 200, "Login should succeed");
        
        // Extract session token from cookie
        let cookies = response.cookies();
        for cookie in cookies {
            if cookie.name() == "inventiv_session" {
                return cookie.value().to_string();
            }
        }
        
        // Fallback: try to get from response body
        let body: serde_json::Value = response.json().await.expect("Failed to parse login response");
        body.get("session_token")
            .or_else(|| body.get("token"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                panic!("No session token found in login response. Response: {:?}", body)
            })
    }
    
    /// Ajouter l'authentification Bearer à une requête
    pub fn bearer_auth(&self, request: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
        request.bearer_auth(token)
    }
    
    /// Ajouter l'authentification via cookie (pour les endpoints qui utilisent HttpOnly cookies)
    pub fn cookie_auth(&self, request: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
        request.header("Cookie", format!("inventiv_session={}", token))
    }
    
    /// Login et utiliser les cookies automatiquement pour les requêtes suivantes
    /// Le cookie HttpOnly sera automatiquement géré par reqwest avec cookie_store=true
    pub async fn login_with_cookies(&self) {
        let _ = self.login().await;
        // Cookie is now stored in http_client automatically via cookie_store=true
    }
}

impl Default for TestClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Créer ou récupérer le client de test singleton
pub fn create_test_client() -> &'static TestClient {
    TEST_CLIENT.get_or_init(|| TestClient::new())
}

