use sqlx::Pool;
use sqlx::Postgres;

/// Run database migrations and verify critical tables exist
pub async fn run_migrations(pool: &Pool<Postgres>) -> Result<(), sqlx::migrate::MigrateError> {
    if let Err(e) = sqlx::migrate!("../sqlx-migrations").run(pool).await {
        // Log error but continue - migrations may have been applied manually
        eprintln!("[warn] Migration error (may be safe to ignore if migrations were applied manually): {}", e);

        // Check if critical tables exist
        let tables_exist: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'user_sessions')"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        let password_reset_table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'password_reset_tokens')"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !tables_exist {
            eprintln!("[error] Critical table 'user_sessions' does not exist - migrations must be applied!");
            return Err(e);
        }

        if !password_reset_table_exists {
            eprintln!("[warn] Table 'password_reset_tokens' does not exist - password reset feature will not work!");
        }

        eprintln!("[info] Critical tables exist - continuing despite migration error");
    }

    Ok(())
}
