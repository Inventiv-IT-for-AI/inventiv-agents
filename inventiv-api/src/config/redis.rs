use redis::Client;

/// Create Redis client from URL
pub fn create_client(redis_url: &str) -> Result<Client, redis::RedisError> {
    Client::open(redis_url)
}

