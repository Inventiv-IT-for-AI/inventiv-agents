use sqlx::{Pool, Postgres};

pub async fn preferred_locale_code(db: &Pool<Postgres>, user_id: uuid::Uuid) -> String {
    sqlx::query_scalar::<_, String>("SELECT locale_code FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "en-US".to_string())
}


