use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Pool, Postgres};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub role: String,
    pub session_id: String, // UUID of the session in user_sessions table
    pub current_organization_id: Option<uuid::Uuid>,
    pub current_organization_role: Option<String>, // owner|admin|manager|user
}

#[derive(Clone, Debug)]
pub struct ApiKeyPrincipal {
    pub api_key_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub key_prefix: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
    sub: String, // user_id
    email: String,
    role: String,
    // Session context
    session_id: String, // UUID of the session in user_sessions table
    current_organization_id: Option<String>,
    current_organization_role: Option<String>, // owner|admin|manager|user
    jti: String,                               // JWT ID (for revocation/rotation)
    iat: usize,
    exp: usize,
}

pub fn jwt_secret() -> String {
    std::env::var("JWT_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "dev_insecure_change_me".to_string())
}

pub fn jwt_issuer() -> String {
    std::env::var("JWT_ISSUER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "inventiv-api".to_string())
}

pub fn jwt_ttl_seconds() -> u64 {
    std::env::var("JWT_TTL_SECONDS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(60 * 60 * 12) // 12h
}

pub fn session_cookie_name() -> String {
    std::env::var("SESSION_COOKIE_NAME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "inventiv_session".to_string())
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn sign_session_jwt(user: &AuthUser) -> anyhow::Result<String> {
    let now = now_ts() as usize;
    let exp = (now_ts() + jwt_ttl_seconds()) as usize;
    // jti is a hash of session_id + secret for secure revocation
    let jti = sha256_hash(&format!("{}:{}", user.session_id, jwt_secret()));
    let claims = Claims {
        iss: jwt_issuer(),
        sub: user.user_id.to_string(),
        email: user.email.clone(),
        role: user.role.clone(),
        session_id: user.session_id.clone(),
        current_organization_id: user.current_organization_id.map(|id| id.to_string()),
        current_organization_role: user.current_organization_role.clone(),
        jti,
        iat: now,
        exp,
    };
    let token = jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret().as_bytes()),
    )?;
    Ok(token)
}

fn sha256_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Hash a JWT token for storage in DB (for revocation)
pub fn hash_session_token(token: &str) -> String {
    sha256_hash(token)
}

fn decode_session_jwt(token: &str) -> anyhow::Result<AuthUser> {
    let mut validation = Validation::default();
    validation.set_issuer(&[jwt_issuer()]);

    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret().as_bytes()),
        &validation,
    )?;
    let user_id = uuid::Uuid::parse_str(&data.claims.sub)?;
    let current_organization_id = match data.claims.current_organization_id.as_deref() {
        Some(s) if !s.trim().is_empty() => uuid::Uuid::parse_str(s).ok(),
        _ => None,
    };
    Ok(AuthUser {
        user_id,
        email: data.claims.email,
        role: data.claims.role,
        session_id: data.claims.session_id,
        current_organization_id,
        current_organization_role: data.claims.current_organization_role,
    })
}

fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    let Some(auth) = headers.get(header::AUTHORIZATION) else {
        return None;
    };
    let Ok(auth) = auth.to_str() else {
        return None;
    };
    let auth = auth.trim();
    let prefix = "Bearer ";
    if auth.len() <= prefix.len() || !auth.starts_with(prefix) {
        return None;
    }
    Some(auth[prefix.len()..].trim().to_string())
}

fn extract_api_key_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let Some(raw) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) else {
        return None;
    };
    for part in raw.split(';') {
        let mut it = part.trim().splitn(2, '=');
        let k = it.next()?.trim();
        let v = it.next().unwrap_or("").trim();
        if k == name && !v.is_empty() {
            return Some(v.to_string());
        }
    }
    None
}

pub fn session_cookie_value(token: &str) -> HeaderValue {
    // SameSite=Lax works well for dashboard-like apps; HttpOnly protects against XSS token theft.
    // Secure should be enabled in prod behind HTTPS; in dev it would block cookies on http://.
    let secure = std::env::var("COOKIE_SECURE")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    let mut s = format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        session_cookie_name(),
        token,
        jwt_ttl_seconds()
    );
    if secure {
        s.push_str("; Secure");
    }
    HeaderValue::from_str(&s)
        .unwrap_or_else(|_| HeaderValue::from_static("inventiv_session=; Path=/"))
}

pub fn clear_session_cookie_value() -> HeaderValue {
    let secure = std::env::var("COOKIE_SECURE")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    let mut s = format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        session_cookie_name()
    );
    if secure {
        s.push_str("; Secure");
    }
    HeaderValue::from_str(&s)
        .unwrap_or_else(|_| HeaderValue::from_static("inventiv_session=; Path=/; Max-Age=0"))
}

pub fn current_user_from_headers(headers: &HeaderMap) -> anyhow::Result<AuthUser> {
    // Prefer cookie (browser sessions), fallback to Authorization Bearer (API clients).
    let token = extract_cookie(headers, &session_cookie_name())
        .or_else(|| extract_bearer(headers))
        .ok_or_else(|| anyhow::anyhow!("missing_token"))?;
    decode_session_jwt(&token)
}

pub async fn require_user(
    State(db): State<Pool<Postgres>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    // Extract token from headers
    let token = extract_cookie(req.headers(), &session_cookie_name())
        .or_else(|| extract_bearer(req.headers()));

    let token = match token {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":"unauthorized","message":"login_required"})),
            )
                .into_response();
        }
    };

    // Decode JWT
    let user = match decode_session_jwt(&token) {
        Ok(u) => u,
        Err(e) => {
            tracing::debug!("JWT decode failed: {}", e);
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":"unauthorized","message":"invalid_token"})),
            )
                .into_response();
        }
    };

    // Verify session in DB
    let session_id = match uuid::Uuid::parse_str(&user.session_id) {
        Ok(id) => id,
        Err(e) => {
            tracing::debug!("Invalid session_id format: {}", e);
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":"unauthorized","message":"invalid_session_id"})),
            )
                .into_response();
        }
    };

    let token_hash = hash_session_token(&token);
    match verify_session_db(&db, session_id, &token_hash).await {
        Ok(true) => {
            // Session is valid, update last_used_at
            update_session_last_used(&db, session_id).await.ok();
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        Ok(false) => {
            tracing::debug!(
                "Session verification failed: session_id={}, token_hash={}",
                session_id,
                token_hash
            );
            // Session invalid, expired, or revoked
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":"unauthorized","message":"session_invalid_or_expired"})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::debug!("Session verification error: {}", e);
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":"unauthorized","message":"session_invalid_or_expired"})),
            )
                .into_response()
        }
    }
}

async fn verify_api_key_db(db: &Pool<Postgres>, token: &str) -> Option<ApiKeyPrincipal> {
    let row: Option<(uuid::Uuid, uuid::Uuid, String, String)> = sqlx::query_as(
        r#"
        SELECT id, user_id, key_prefix, name
        FROM api_keys
        WHERE revoked_at IS NULL
          AND key_hash = encode(digest($1::text, 'sha256'), 'hex')
        LIMIT 1
        "#,
    )
    .bind(token)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let Some((api_key_id, user_id, key_prefix, name)) = row else {
        return None;
    };

    let _ = sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
        .bind(api_key_id)
        .execute(db)
        .await;

    Some(ApiKeyPrincipal {
        api_key_id,
        user_id,
        key_prefix,
        name,
    })
}

/// Middleware: allow either browser session (cookie/JWT) OR OpenAI API key (Bearer or X-API-Key).
pub async fn require_user_or_api_key(
    State(db): State<Pool<Postgres>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    // 1) Cookie-based session (preferred for browsers)
    let cookie_token = extract_cookie(req.headers(), &session_cookie_name());
    if let Some(tok) = cookie_token {
        if let Ok(user) = decode_session_jwt(&tok) {
            // Verify session in DB
            if let Ok(session_id) = uuid::Uuid::parse_str(&user.session_id) {
                let token_hash = hash_session_token(&tok);
                if verify_session_db(&db, session_id, &token_hash)
                    .await
                    .unwrap_or(false)
                {
                    update_session_last_used(&db, session_id).await.ok();
                    req.extensions_mut().insert(user);
                    return next.run(req).await;
                }
            }
        }
    }

    // 2) X-API-Key header (common in some clients)
    if let Some(key) = extract_api_key_header(req.headers()) {
        if let Some(p) = verify_api_key_db(&db, &key).await {
            req.extensions_mut().insert(p);
            return next.run(req).await;
        }
    }

    // 3) Authorization: Bearer ... (could be JWT or API key)
    if let Some(tok) = extract_bearer(req.headers()) {
        if let Ok(user) = decode_session_jwt(&tok) {
            // Verify session in DB
            if let Ok(session_id) = uuid::Uuid::parse_str(&user.session_id) {
                let token_hash = hash_session_token(&tok);
                if verify_session_db(&db, session_id, &token_hash)
                    .await
                    .unwrap_or(false)
                {
                    update_session_last_used(&db, session_id).await.ok();
                    req.extensions_mut().insert(user);
                    return next.run(req).await;
                }
            }
        }
        if let Some(p) = verify_api_key_db(&db, &tok).await {
            req.extensions_mut().insert(p);
            return next.run(req).await;
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error":"unauthorized","message":"api_key_or_login_required"})),
    )
        .into_response()
}

pub fn require_admin(user: &AuthUser) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if user.role.to_ascii_lowercase() == "admin" {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error":"forbidden","message":"admin_required"})),
        ))
    }
}

// ============================================================================
// Session Management Helpers (DB operations)
// ============================================================================

#[derive(Debug, sqlx::FromRow)]
struct SessionRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    current_organization_id: Option<uuid::Uuid>,
    organization_role: Option<String>,
    session_token_hash: String,
    ip_address: Option<String>, // IP as string from DB
    user_agent: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
    revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Create a new session in the database
pub async fn create_session(
    db: &Pool<Postgres>,
    session_id: uuid::Uuid,
    user_id: uuid::Uuid,
    current_organization_id: Option<uuid::Uuid>,
    organization_role: Option<String>,
    ip_address: Option<String>, // IP as string (e.g., "192.168.1.1")
    user_agent: Option<String>,
    token_hash: String,
) -> anyhow::Result<()> {
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(jwt_ttl_seconds() as i64);

    sqlx::query(
        r#"
        INSERT INTO user_sessions (
            id, user_id, current_organization_id, organization_role,
            session_token_hash, ip_address, user_agent,
            created_at, last_used_at, expires_at
        )
        VALUES ($1, $2, $3, $4, $5, $6::inet, $7, NOW(), NOW(), $8)
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(current_organization_id)
    .bind(organization_role)
    .bind(token_hash)
    .bind(ip_address)
    .bind(user_agent)
    .bind(expires_at)
    .execute(db)
    .await?;

    Ok(())
}

/// Verify a session exists and is valid (not revoked, not expired)
pub async fn verify_session_db(
    db: &Pool<Postgres>,
    session_id: uuid::Uuid,
    token_hash: &str,
) -> anyhow::Result<bool> {
    // First check if session exists at all (for early return)
    let session_exists: Option<(String, bool, bool)> = sqlx::query_as(
        "SELECT session_token_hash, expires_at > NOW() as not_expired, revoked_at IS NULL as not_revoked 
         FROM user_sessions WHERE id = $1"
    )
    .bind(session_id)
    .fetch_optional(db)
    .await?;

    if session_exists.is_none() {
        return Ok(false);
    }

    // Fetch the row and compare hash in Rust (avoids SQL string comparison issues)
    // Use a small buffer for expires_at comparison to avoid race conditions
    let row: Option<SessionRow> = sqlx::query_as(
        r#"
        SELECT id, user_id, current_organization_id, organization_role,
               session_token_hash, ip_address::text as ip_address, user_agent,
               created_at, last_used_at, expires_at, revoked_at
        FROM user_sessions
        WHERE id = $1
          AND revoked_at IS NULL
          AND expires_at > (NOW() - INTERVAL '1 second')
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(db)
    .await?;

    let result = match row {
        Some(row) => row.session_token_hash == token_hash,
        None => false,
    };

    Ok(result)
}

/// Update session's last_used_at timestamp
pub async fn update_session_last_used(
    db: &Pool<Postgres>,
    session_id: uuid::Uuid,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE user_sessions SET last_used_at = NOW() WHERE id = $1")
        .bind(session_id)
        .execute(db)
        .await?;

    Ok(())
}

/// Update session's organization context
pub async fn update_session_org(
    db: &Pool<Postgres>,
    session_id: uuid::Uuid,
    current_organization_id: Option<uuid::Uuid>,
    organization_role: Option<String>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE user_sessions
        SET current_organization_id = $2,
            organization_role = $3,
            last_used_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(session_id)
    .bind(current_organization_id)
    .bind(organization_role)
    .execute(db)
    .await?;

    Ok(())
}

/// Update session's token hash (for token rotation)
pub async fn update_session_token_hash(
    db: &Pool<Postgres>,
    session_id: uuid::Uuid,
    token_hash: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE user_sessions SET session_token_hash = $2, last_used_at = NOW() WHERE id = $1",
    )
    .bind(session_id)
    .bind(token_hash)
    .execute(db)
    .await?;

    Ok(())
}

/// Revoke a session (soft delete)
pub async fn revoke_session(db: &Pool<Postgres>, session_id: uuid::Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE user_sessions SET revoked_at = NOW() WHERE id = $1")
        .bind(session_id)
        .execute(db)
        .await?;

    Ok(())
}

/// Get user's last used organization (for default org selection on login)
pub async fn get_user_last_org(
    db: &Pool<Postgres>,
    user_id: uuid::Uuid,
) -> anyhow::Result<Option<uuid::Uuid>> {
    let row: Option<(Option<uuid::Uuid>,)> = sqlx::query_as(
        r#"
        SELECT current_organization_id
        FROM user_sessions
        WHERE user_id = $1
          AND revoked_at IS NULL
          AND current_organization_id IS NOT NULL
        ORDER BY last_used_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(row.and_then(|r| r.0))
}

/// Extract IP address from request headers
pub fn extract_ip_address(headers: &HeaderMap) -> Option<String> {
    // Try X-Forwarded-For first (for proxies/load balancers)
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // X-Forwarded-For can contain multiple IPs, take the first one
            if let Some(first_ip) = forwarded_str.split(',').next() {
                return Some(first_ip.trim().to_string());
            }
        }
    }

    // Try X-Real-IP
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(real_ip_str) = real_ip.to_str() {
            return Some(real_ip_str.trim().to_string());
        }
    }

    // Fallback: try to parse from Remote-Addr (if available)
    // Note: In most cases, this won't be available in headers
    None
}

/// Extract User-Agent from request headers
pub fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
