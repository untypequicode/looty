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

/// Durée d'inactivité après laquelle une session expire.
pub const INACTIVITY_DAYS: i64 = 14;
/// La session n'est prolongée qu'au plus une fois par heure.
const SLIDE_AFTER: time::Duration = time::Duration::hours(1);

impl PgSessionStore {
    /// Repousse l'expiration de la session `id` à `new_expiry`, si elle est encore valide et que son expiration
    /// actuelle est antérieure à `older_than` (au plus une écriture par heure). Seule la date change : les données
    /// de la session ne sont ni lues ni réécrites, donc une requête concurrente (déconnexion, nouvelle connexion)
    /// ne peut ni perdre `user_id` ni ressusciter une session supprimée. Vrai si la date a bougé.
    pub async fn slide(&self, id: &Id, new_expiry: OffsetDateTime, older_than: OffsetDateTime) -> sqlx::Result<bool> {
        let done = sqlx::query(
            r#"update "tower_sessions"."session" set expiry_date = $2
               where id = $1 and expiry_date > now() and expiry_date < $3"#,
        )
        .bind(id.to_string())
        .bind(new_expiry)
        .bind(older_than)
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() > 0)
    }
}

#[derive(Clone)]
pub struct Slide {
    pub store: PgSessionStore,
    pub secure: bool,
}

/// tower-sessions n'enregistre une session que si un gestionnaire la modifie : sans cela, l'expiration
/// (`OnInactivity`) ne glisse jamais et la session meurt 14 jours après la connexion. Ce middleware, placé
/// sous la couche de session, prolonge une session connectée (en base et dans le cookie) au plus une fois par heure.
pub async fn slide(
    axum::extract::State(s): axum::extract::State<Slide>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let session = req.extensions().get::<tower_sessions::Session>().cloned();
    let mut res = next.run(req).await;
    let Some(session) = session else { return res };
    // Modifiée : la couche de session l'enregistre elle-même, avec une expiration neuve.
    if session.is_modified() || res.status().is_server_error() {
        return res;
    }
    let Some(id) = session.id() else { return res };
    if !matches!(session.get::<uuid::Uuid>(crate::access::SESSION_USER_KEY).await, Ok(Some(_))) {
        return res;
    }
    let max_age = time::Duration::days(INACTIVITY_DAYS);
    let now = OffsetDateTime::now_utc();
    match s.store.slide(&id, now + max_age, now + max_age - SLIDE_AFTER).await {
        Ok(true) => {
            let cookie = tower_sessions::cookie::Cookie::build(("looty_session", id.to_string()))
                .path("/")
                .http_only(true)
                .secure(s.secure)
                .same_site(tower_sessions::cookie::SameSite::Lax)
                .max_age(max_age)
                .build();
            if let Ok(v) = axum::http::HeaderValue::from_str(&cookie.to_string()) {
                res.headers_mut().append(axum::http::header::SET_COOKIE, v);
            }
        }
        Ok(false) => {}
        Err(e) => tracing::warn!("session slide: {e}"),
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;
    use tower_sessions::{Expiry, Session, SessionManagerLayer};

    fn app(store: PgSessionStore) -> axum::Router {
        let layer = SessionManagerLayer::new(store.clone())
            .with_name("looty_session")
            .with_expiry(Expiry::OnInactivity(time::Duration::days(INACTIVITY_DAYS)));
        axum::Router::new()
            .route("/login", get(|s: Session| async move { s.insert(crate::access::SESSION_USER_KEY, uuid::Uuid::new_v4()).await.unwrap() }))
            .route("/anon", get(|_s: Session| async {}))
            .route("/ping", get(|_s: Session| async {}))
            .layer(axum::middleware::from_fn_with_state(Slide { store, secure: false }, slide))
            .layer(layer)
    }

    fn cookie_of(res: &axum::response::Response) -> Option<String> {
        res.headers().get(header::SET_COOKIE).map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string())
    }

    async fn call(app: &axum::Router, path: &str, cookie: Option<&str>) -> axum::response::Response {
        let mut req = Request::builder().uri(path);
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, c);
        }
        app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap()
    }

    async fn expiry(pool: &sqlx::PgPool, cookie: &str) -> OffsetDateTime {
        let id = cookie.split('=').nth(1).unwrap();
        sqlx::query_scalar(r#"select expiry_date from "tower_sessions"."session" where id = $1"#).bind(id).fetch_one(pool).await.unwrap()
    }

    async fn age_session(pool: &sqlx::PgPool, cookie: &str, days: i64) {
        let id = cookie.split('=').nth(1).unwrap();
        sqlx::query(r#"update "tower_sessions"."session" set expiry_date = now() + make_interval(days => $2) where id = $1"#)
            .bind(id)
            .bind(days as i32)
            .execute(pool)
            .await
            .unwrap();
    }

    #[sqlx::test]
    async fn an_active_session_slides_at_most_once_an_hour_and_keeps_its_user(pool: sqlx::PgPool) {
        let store = PgSessionStore::new(pool.clone());
        store.migrate().await.unwrap();
        let app = app(store);
        let login = call(&app, "/login", None).await;
        let cookie = cookie_of(&login).expect("login sets the cookie");

        // Fraîchement ouverte : rien à prolonger, pas de nouveau cookie.
        let res = call(&app, "/ping", Some(&cookie)).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(cookie_of(&res).is_none());

        // Dix jours d'expiration restants (4 jours d'inactivité) : elle repart pour 14 jours, cookie compris.
        age_session(&pool, &cookie, 10).await;
        let res = call(&app, "/ping", Some(&cookie)).await;
        assert_eq!(cookie_of(&res).as_deref(), Some(cookie.as_str()));
        let set = res.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().to_string();
        assert!(set.contains("Max-Age=1209600"), "{set}");
        let left = expiry(&pool, &cookie).await - OffsetDateTime::now_utc();
        assert!(left > time::Duration::days(13), "{left}");

        // Aussitôt après : pas de nouvelle écriture.
        assert!(cookie_of(&call(&app, "/ping", Some(&cookie)).await).is_none());

        // L'utilisateur est toujours dans la session.
        let data: Vec<u8> = sqlx::query_scalar(r#"select data from "tower_sessions"."session""#).fetch_one(&pool).await.unwrap();
        let record: Record = rmp_serde::from_slice(&data).unwrap();
        assert!(record.data.contains_key(crate::access::SESSION_USER_KEY));
    }

    #[sqlx::test]
    async fn anonymous_and_expired_sessions_are_not_extended(pool: sqlx::PgPool) {
        let store = PgSessionStore::new(pool.clone());
        store.migrate().await.unwrap();
        let app = app(store);
        assert!(cookie_of(&call(&app, "/anon", None).await).is_none());

        let cookie = cookie_of(&call(&app, "/login", None).await).unwrap();
        age_session(&pool, &cookie, -1).await;
        call(&app, "/ping", Some(&cookie)).await;
        assert!(expiry(&pool, &cookie).await < OffsetDateTime::now_utc());
    }
}
