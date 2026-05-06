use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

pub type PostgresPool = PgPool;

/// Create a connection pool and run any pending sqlx migrations.
pub async fn create_pool(url: &str) -> anyhow::Result<PostgresPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(url)
        .await
        .map_err(|e| anyhow::anyhow!("failed to connect to Postgres: {e}"))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;

    Ok(pool)
}
