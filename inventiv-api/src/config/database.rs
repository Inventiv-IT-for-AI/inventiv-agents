use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

/// Create and configure database connection pool
pub async fn create_pool(database_url: &str) -> Result<Pool<Postgres>, sqlx::Error> {
    let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(5);

    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await
}
