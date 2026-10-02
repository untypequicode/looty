use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;

pub async fn init_db() -> Result<PgPool, sqlx::Error> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new().max_connections(20).connect(&database_url).await?;
    let mut migrator = sqlx::migrate!("./migrations");
    // Une ancienne migration (« 20260929 ») a été renommée : les bases existantes
    // la connaissent encore sous son ancien numéro.
    migrator.set_ignore_missing(true);
    migrator.run(&pool).await?;
    Ok(pool)
}
