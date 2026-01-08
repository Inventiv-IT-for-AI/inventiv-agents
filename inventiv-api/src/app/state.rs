use redis::Client;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub redis_client: Client,
    pub db: Pool<Postgres>,
}

impl AppState {
    pub fn new(redis_client: Client, db: Pool<Postgres>) -> Arc<Self> {
        Arc::new(Self { redis_client, db })
    }
}
