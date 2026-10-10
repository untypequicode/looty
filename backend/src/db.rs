use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;

/// Migrations entièrement rejouables (ajouts « IF NOT EXISTS » uniquement) dont une version
/// provisoire a pu être appliquée par une base construite pendant leur développement : leur
/// somme de contrôle diffère alors du fichier publié et sqlx refuse de démarrer. Elles sont
/// rejouées (ce qui manque est ajouté, rien n'est supprimé) puis marquées à jour.
const REPLAYABLE: &[i64] = &[20261020120000];

pub async fn init_db() -> Result<PgPool, sqlx::Error> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new().max_connections(20).connect(&database_url).await?;
    let mut migrator = sqlx::migrate!("./migrations");
    // Une ancienne migration (« 20260929 ») a été renommée : les bases existantes
    // la connaissent encore sous son ancien numéro.
    migrator.set_ignore_missing(true);
    repair_replayable(&pool, &migrator).await?;
    migrator.run(&pool).await?;
    Ok(pool)
}

async fn repair_replayable(pool: &PgPool, migrator: &sqlx::migrate::Migrator) -> Result<(), sqlx::Error> {
    let table: Option<String> = sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations')::text").fetch_one(pool).await?;
    if table.is_none() {
        return Ok(());
    }
    for m in migrator.iter().filter(|m| REPLAYABLE.contains(&m.version)) {
        let stored: Option<Vec<u8>> = sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = $1")
            .bind(m.version)
            .fetch_optional(pool)
            .await?;
        let Some(stored) = stored else { continue };
        if stored == m.checksum.as_ref() {
            continue;
        }
        tracing::warn!("migration {} ({}) : version provisoire détectée, rejouée", m.version, m.description);
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(&m.sql).execute(&mut *tx).await?;
        sqlx::query("UPDATE _sqlx_migrations SET checksum = $1 WHERE version = $2")
            .bind(m.checksum.as_ref())
            .bind(m.version)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }
    Ok(())
}
