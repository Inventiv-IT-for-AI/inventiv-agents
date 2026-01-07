use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

/// Create and configure database connection pool
pub async fn create_pool(database_url: &str) -> Result<Pool<Postgres>, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}
