//! Sessions de connexion dans PostgreSQL (table `tower_sessions.session`).
//!
//! Même table et même encodage (MessagePack) que `tower-sessions-sqlx-store`, qui n'est pas
//! disponible pour les versions actuelles de sqlx et tower-sessions : les sessions ouvertes
//! restent valides. Les sessions expirées sont supprimées toutes les heures.

use sqlx::PgPool;
use time::OffsetDateTime;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store::{self, Error, SessionStore};

#[derive(Clone, Debug)]
pub struct PgSessionStore {
    pool: PgPool,
}

fn backend(e: sqlx::Error) -> Error {
    Error::Backend(e.to_string())
}

impl PgSessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Crée la table si besoin (déjà présente sur les installations existantes).
    pub async fn migrate(&self) -> sqlx::Result<()> {
        let mut tx = self.pool.begin().await?;
        // Deux démarrages simultanés : la création concurrente du schéma peut échouer alors qu'il existe.
        if let Err(e) = sqlx::query(r#"create schema if not exists "tower_sessions""#).execute(&mut *tx).await {
            if !matches!(&e, sqlx::Error::Database(d) if d.code().as_deref() == Some("23505")) {
                return Err(e);
            }
        }
        sqlx::query(
            r#"create table if not exists "tower_sessions"."session" (
                id text primary key not null,
                data bytea not null,
                expiry_date timestamptz not null
            )"#,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }

    /// Supprime les sessions expirées, puis toutes les heures.
    pub fn start_cleanup(&self) {
        let pool = self.pool.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = sqlx::query(r#"delete from "tower_sessions"."session" where expiry_date < now()"#).execute(&pool).await {
                    tracing::warn!("expired sessions cleanup: {e}");
                }
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        });
    }

    async fn save_with<'e, E: sqlx::PgExecutor<'e>>(&self, exec: E, record: &Record) -> session_store::Result<()> {
        let data = rmp_serde::to_vec(record).map_err(|e| Error::Encode(e.to_string()))?;
        sqlx::query(
            r#"insert into "tower_sessions"."session" (id, data, expiry_date) values ($1, $2, $3)
               on conflict (id) do update set data = excluded.data, expiry_date = excluded.expiry_date"#,
        )
        .bind(record.id.to_string())
        .bind(data)
        .bind(record.expiry_date)
        .execute(exec)
        .await
        .map_err(backend)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl SessionStore for PgSessionStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let mut tx = self.pool.begin().await.map_err(backend)?;
        // Identifiant déjà pris (très improbable) : un autre est tiré.
        while sqlx::query_scalar::<_, bool>(r#"select exists(select 1 from "tower_sessions"."session" where id = $1)"#)
            .bind(record.id.to_string())
            .fetch_one(&mut *tx)
            .await
            .map_err(backend)?
        {
            record.id = Id::default();
        }
        self.save_with(&mut *tx, record).await?;
        tx.commit().await.map_err(backend)
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        self.save_with(&self.pool, record).await
    }

    async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
        let data: Option<Vec<u8>> =
            sqlx::query_scalar(r#"select data from "tower_sessions"."session" where id = $1 and expiry_date > $2"#)
                .bind(id.to_string())
                .bind(OffsetDateTime::now_utc())
                .fetch_optional(&self.pool)
                .await
                .map_err(backend)?;
        data.map(|d| rmp_serde::from_slice(&d).map_err(|e| Error::Decode(e.to_string()))).transpose()
    }

    async fn delete(&self, id: &Id) -> session_store::Result<()> {
        sqlx::query(r#"delete from "tower_sessions"."session" where id = $1"#)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }
}
