//! Assistant de premier lancement (issue 163, ADR 0015).
//!
//! L'assistant s'affiche à l'administrateur tant que `global_settings.setup_completed_at` est vide. Les instances qui
//! ont déjà des comptes l'ont reçu rempli par la migration. Chaque étape utilise une route de l'administration
//! (nom, langue et fuseau : `/api/admin/settings` ; e-mails : `/api/admin/mail`), donc se refait depuis la page
//! d'administration ; la page `/setup` reste ouverte aux administrateurs.

use axum::{extract::State, routing::{get, post}, Json, Router};

use crate::access::AuthUser;
use crate::admin::require_super_admin;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/state", get(state)).route("/complete", post(complete))
}

/// Fuseau IANA reconnu (la liste de `chrono-tz`).
pub fn valid_timezone(tz: &str) -> bool {
    tz.parse::<chrono_tz::Tz>().is_ok()
}

async fn state(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    // Démo, et instance de développement (`LOOTY_DEV_LOGIN`, jamais en production) : pas d'assistant.
    if state.demo || std::env::var("LOOTY_DEV_LOGIN").as_deref() == Ok("true") {
        return Ok(Json(serde_json::json!({ "needed": false, "admin_exists": true })));
    }
    let done: bool = sqlx::query_scalar("SELECT setup_completed_at IS NOT NULL FROM global_settings WHERE id = 1").fetch_optional(&state.pool).await?.unwrap_or(false);
    let admin_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE is_super_admin AND id <> $1)").bind(uuid::Uuid::nil()).fetch_one(&state.pool).await?;
    let providers: i64 = sqlx::query_scalar("SELECT count(*) FROM oauth_providers WHERE is_active").fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({
        "needed": !done,
        "admin_exists": admin_exists,
        "local_accounts": state.local_accounts,
        "oidc_providers": providers,
    })))
}

/// « Terminer » ou « Passer » : l'assistant ne revient plus tout seul.
async fn complete(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    sqlx::query("INSERT INTO global_settings (id, setup_completed_at) VALUES (1, NOW()) ON CONFLICT (id) DO UPDATE SET setup_completed_at = COALESCE(global_settings.setup_completed_at, NOW())")
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use tower::ServiceExt;
    use uuid::Uuid;

    struct Api {
        app: axum::Router,
        pool: PgPool,
    }

    impl Api {
        async fn new(pool: &PgPool) -> Api {
            crate::session_store::PgSessionStore::new(pool.clone()).migrate().await.unwrap();
            let mut state = AppState::new(pool.clone(), "http://localhost".into());
            state.local_accounts = true;
            Api { app: crate::app(state), pool: pool.clone() }
        }
        async fn admin(&self, email: &str, admin: bool) -> String {
            let id: Uuid = sqlx::query_scalar("INSERT INTO users (email, name, is_super_admin) VALUES ($1, 'x', $2) RETURNING id").bind(email).bind(admin).fetch_one(&self.pool).await.unwrap();
            let raw = format!("looty_{}", Uuid::new_v4().simple());
            sqlx::query("INSERT INTO api_tokens (user_id, name, token_hash, scope, access) VALUES ($1, 't', $2, 'api', 'write')").bind(id).bind(crate::access::hash_token(&raw)).execute(&self.pool).await.unwrap();
            raw
        }
        async fn call(&self, token: Option<&str>, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
            let mut req = Request::builder().method(method).uri(path).header("content-type", "application/json");
            if let Some(t) = token {
                req = req.header("authorization", format!("Bearer {t}"));
            }
            let res = self.app.clone().oneshot(req.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap()).await.unwrap();
            let status = res.status().as_u16();
            let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
            (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
        }
    }

    #[sqlx::test]
    async fn the_wizard_shows_until_an_administrator_finishes_it(pool: PgPool) {
        let api = Api::new(&pool).await;
        let (_, s) = api.call(None, "GET", "/api/setup/state", None).await;
        assert_eq!((s["needed"].clone(), s["admin_exists"].clone(), s["local_accounts"].clone()), (json!(true), json!(false), json!(true)));
        let admin = api.admin("root@example.org", true).await;
        let member = api.admin("ana@example.org", false).await;
        let (_, s) = api.call(None, "GET", "/api/setup/state", None).await;
        assert_eq!((s["needed"].clone(), s["admin_exists"].clone()), (json!(true), json!(true)));
        // Seul un administrateur le termine ; un jeton n'y accède jamais.
        assert_eq!(api.call(Some(&member), "POST", "/api/setup/complete", None).await.0, 403);
        assert_eq!(api.call(Some(&admin), "POST", "/api/setup/complete", None).await.0, 403, "tokens are refused");
        sqlx::query("UPDATE global_settings SET setup_completed_at = NOW() WHERE id = 1").execute(&pool).await.unwrap();
        let (_, s) = api.call(None, "GET", "/api/setup/state", None).await;
        assert_eq!(s["needed"], false);
    }

    #[sqlx::test]
    async fn completing_is_idempotent_and_keeps_the_first_date(pool: PgPool) {
        let state = AppState::new(pool.clone(), "http://localhost".into());
        let user = AuthUser { id: Uuid::new_v4(), is_super_admin: true };
        complete(State(state.clone()), user).await.unwrap();
        let first: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT setup_completed_at FROM global_settings WHERE id = 1").fetch_one(&pool).await.unwrap();
        complete(State(state), user).await.unwrap();
        let second: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT setup_completed_at FROM global_settings WHERE id = 1").fetch_one(&pool).await.unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn timezones_must_be_known() {
        assert!(valid_timezone("Europe/Paris"));
        assert!(valid_timezone("UTC"));
        assert!(!valid_timezone("Mars/Olympus"));
    }

    #[sqlx::test]
    async fn instance_name_language_and_timezone_are_saved_and_given_to_new_accounts(pool: PgPool) {
        let api = Api::new(&pool).await;
        let admin = api.admin("root@example.org", true).await;
        let bad = |extra: Value| {
            let mut b = json!({ "app_name": "Campus", "allow_public_signups": true });
            b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            b
        };
        let (s, e) = api.call(Some(&admin), "PUT", "/api/admin/settings", Some(bad(json!({ "default_timezone": "Mars/Olympus" })))).await;
        assert_eq!((s, e["error"].as_str()), (400, Some("invalid_timezone")));
        let (s, e) = api.call(Some(&admin), "PUT", "/api/admin/settings", Some(bad(json!({ "default_locale": "de" })))).await;
        assert_eq!((s, e["error"].as_str()), (400, Some("invalid_locale")));
        let (s, saved) = api.call(Some(&admin), "PUT", "/api/admin/settings", Some(bad(json!({ "default_locale": "fr", "default_timezone": "Europe/Paris" })))).await;
        assert_eq!(s, 200, "{saved}");
        assert_eq!((saved["default_locale"].clone(), saved["default_timezone"].clone()), (json!("fr"), json!("Europe/Paris")));
        // Un compte créé ensuite (invitation locale) reçoit ces valeurs.
        let hash = "x";
        let id = crate::local_auth::insert_user(&pool, "new@example.org", "New", hash, false, true, None).await.unwrap();
        let (locale, tz): (Option<String>, Option<String>) = sqlx::query_as("SELECT locale, timezone FROM users WHERE id = $1").bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!((locale.as_deref(), tz.as_deref()), (Some("fr"), Some("Europe/Paris")));
    }
}
