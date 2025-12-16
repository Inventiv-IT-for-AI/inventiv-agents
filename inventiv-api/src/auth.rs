use axum::{
    body::Body,
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
    sub: String, // user_id
    email: String,
    role: String,
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
    let claims = Claims {
        iss: jwt_issuer(),
        sub: user.user_id.to_string(),
        email: user.email.clone(),
        role: user.role.clone(),
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

fn decode_session_jwt(token: &str) -> anyhow::Result<AuthUser> {
    let mut validation = Validation::default();
    validation.set_issuer(&[jwt_issuer()]);

    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret().as_bytes()),
        &validation,
    )?;
    let user_id = uuid::Uuid::parse_str(&data.claims.sub)?;
    Ok(AuthUser {
        user_id,
        email: data.claims.email,
        role: data.claims.role,
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
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
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
    HeaderValue::from_str(&s).unwrap_or_else(|_| HeaderValue::from_static("inventiv_session=; Path=/"))
}

pub fn clear_session_cookie_value() -> HeaderValue {
    let secure = std::env::var("COOKIE_SECURE")
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    let mut s = format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        session_cookie_name()
    );
    if secure {
        s.push_str("; Secure");
    }
    HeaderValue::from_str(&s).unwrap_or_else(|_| HeaderValue::from_static("inventiv_session=; Path=/; Max-Age=0"))
}

pub fn current_user_from_headers(headers: &HeaderMap) -> anyhow::Result<AuthUser> {
    // Prefer cookie (browser sessions), fallback to Authorization Bearer (API clients).
    let token = extract_cookie(headers, &session_cookie_name())
        .or_else(|| extract_bearer(headers))
        .ok_or_else(|| anyhow::anyhow!("missing_token"))?;
    decode_session_jwt(&token)
}

pub async fn require_user(mut req: Request<Body>, next: Next) -> Response {
    match current_user_from_headers(req.headers()) {
        Ok(user) => {
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"unauthorized","message":"login_required"})),
        )
            .into_response(),
    }
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


