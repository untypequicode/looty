use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Json, Router,
};
use openidconnect::{
    core::{CoreClient, CoreProviderMetadata, CoreResponseType, CoreUserInfoClaims},
    reqwest::async_http_client,
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use crate::access::{AuthUser, SESSION_USER_KEY};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct ProviderDto {
    pub id: Uuid,
    pub name: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/providers", get(get_active_providers))
        .route("/login/:provider_id", get(login_handler))
        .route("/callback/:provider_id", get(callback_handler))
        .route("/me", get(me_handler))
        .route("/logout", get(logout_handler).post(logout_handler))
}

async fn get_active_providers(State(state): State<AppState>) -> ApiResult<Json<Vec<ProviderDto>>> {
    Ok(Json(
        sqlx::query_as("SELECT id, name FROM oauth_providers WHERE is_active = TRUE ORDER BY name")
            .fetch_all(&state.pool)
            .await?,
    ))
}

#[derive(sqlx::FromRow)]
struct ProviderConfig {
    issuer_url: String,
    client_id: String,
    client_secret: String,
}

async fn get_oidc_client(state: &AppState, provider_id: Uuid) -> Result<CoreClient, String> {
    if let Some(client) = state.oidc_clients.get(&provider_id) {
        return Ok(client.clone());
    }
    let provider = sqlx::query_as::<_, ProviderConfig>(
        "SELECT issuer_url, client_id, client_secret FROM oauth_providers WHERE id = $1 AND is_active = TRUE",
    )
    .bind(provider_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "provider_not_found".to_string())?;

    let issuer_url = IssuerUrl::new(provider.issuer_url).map_err(|e| e.to_string())?;
    let redirect_url = RedirectUrl::new(format!("{}/api/auth/callback/{}", state.base_url, provider_id))
        .map_err(|e| e.to_string())?;
    let metadata = CoreProviderMetadata::discover_async(issuer_url, async_http_client)
        .await
        .map_err(|e| e.to_string())?;
    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(provider.client_id),
        Some(ClientSecret::new(provider.client_secret)),
    )
    .set_redirect_uri(redirect_url);
    state.oidc_clients.insert(provider_id, client.clone());
    Ok(client)
}

/// N'accepte que des chemins internes (évite les redirections ouvertes).
pub fn safe_redirect(target: Option<&str>) -> String {
    match target {
        Some(t) if t.starts_with('/') && !t.starts_with("//") && !t.starts_with("/\\") && !t.contains("://") => {
            t.to_string()
        }
        _ => "/".to_string(),
    }
}

#[derive(Deserialize)]
struct LoginQuery {
    redirect: Option<String>,
}

async fn login_handler(
    session: Session,
    State(state): State<AppState>,
    Path(provider_id): Path<Uuid>,
    Query(q): Query<LoginQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let client = get_oidc_client(&state, provider_id)
        .await
        .map_err(|e| ApiError::BadRequest(format!("provider_error: {e}")))?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".into()))
        .add_scope(Scope::new("profile".into()))
        .add_scope(Scope::new("email".into()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let store = async {
        session.insert("pkce_verifier", pkce_verifier.secret()).await?;
        session.insert("csrf_token", csrf_token.secret()).await?;
        session.insert("nonce", nonce.secret()).await?;
        session.insert("post_login_redirect", safe_redirect(q.redirect.as_deref())).await
    };
    store.await.map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "url": auth_url.as_str() })))
}

#[derive(Deserialize)]
struct AuthCallback {
    code: String,
    state: String,
}

fn login_error(code: &str) -> Response {
    Redirect::to(&format!("/login?error={code}")).into_response()
}

async fn callback_handler(
    session: Session,
    State(state): State<AppState>,
    Path(provider_id): Path<Uuid>,
    Query(params): Query<AuthCallback>,
) -> Response {
    let stored_csrf: Option<String> = session.remove("csrf_token").await.ok().flatten();
    if stored_csrf.as_deref() != Some(params.state.as_str()) {
        return login_error("csrf_mismatch");
    }
    let Some(pkce_secret) = session.remove::<String>("pkce_verifier").await.ok().flatten() else {
        return login_error("missing_pkce");
    };
    let Ok(client) = get_oidc_client(&state, provider_id).await else {
        return login_error("provider_error");
    };

    let Ok(token) = client
        .exchange_code(AuthorizationCode::new(params.code))
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_secret))
        .request_async(async_http_client)
        .await
    else {
        return login_error("exchange_failed");
    };

    let Ok(req) = client.user_info(token.access_token().clone(), None) else {
        return login_error("userinfo_failed");
    };
    let userinfo: CoreUserInfoClaims = match req.request_async(async_http_client).await {
        Ok(u) => u,
        Err(_) => return login_error("userinfo_failed"),
    };

    let email = userinfo.email().map(|e| e.as_str().trim().to_lowercase()).unwrap_or_default();
    let email_verified = userinfo.email_verified().unwrap_or(true);
    let name = userinfo
        .name()
        .and_then(|n| n.get(None))
        .map(|n| n.as_str().to_string())
        .or_else(|| userinfo.preferred_username().map(|u| u.as_str().to_string()))
        .unwrap_or_else(|| email.split('@').next().unwrap_or("User").to_string());
    let picture = userinfo.picture().and_then(|p| p.get(None)).map(|p| p.as_str().to_string());
    let subject = userinfo.subject().as_str().to_string();
    let provider_key = provider_id.to_string();

    match resolve_user(&state, &provider_key, &subject, &email, email_verified, &name, picture).await {
        Ok(user_id) => {
            // Rotation de l'identifiant de session (anti fixation) puis connexion.
            let redirect: String = session
                .remove("post_login_redirect")
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "/".into());
            let _ = session.remove::<String>("nonce").await;
            if session.cycle_id().await.is_err() || session.insert(SESSION_USER_KEY, user_id).await.is_err() {
                return login_error("session_error");
            }
            Redirect::to(&safe_redirect(Some(&redirect))).into_response()
        }
        Err(code) => login_error(code),
    }
}

/// Trouve ou crée l'utilisateur correspondant à une identité OIDC.
///
/// 1. identité (provider, sub) déjà liée -> cet utilisateur ;
/// 2. sinon e-mail vérifié correspondant à un compte -> on lie l'identité ;
/// 3. sinon création du compte (si les inscriptions sont ouvertes ou premier utilisateur).
async fn resolve_user(
    state: &AppState,
    provider: &str,
    subject: &str,
    email: &str,
    email_verified: bool,
    name: &str,
    picture: Option<String>,
) -> Result<Uuid, &'static str> {
    let pool = &state.pool;
    let db = |_| "database_error";

    let existing: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT u.id, u.is_suspended FROM identities i JOIN users u ON u.id = i.user_id
         WHERE i.provider = $1 AND i.provider_user_id = $2",
    )
    .bind(provider)
    .bind(subject)
    .fetch_optional(pool)
    .await
    .map_err(db)?;

    if let Some((id, suspended)) = existing {
        if suspended {
            return Err("suspended");
        }
        return Ok(id);
    }

    if email.is_empty() {
        return Err("missing_email");
    }

    let by_email: Option<(Uuid, bool)> = sqlx::query_as("SELECT id, is_suspended FROM users WHERE lower(email) = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(db)?;

    let user_id = match by_email {
        Some((id, suspended)) => {
            if suspended {
                return Err("suspended");
            }
            if !email_verified {
                return Err("email_not_verified");
            }
            id
        }
        None => {
            let user_count: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(pool).await.map_err(db)?;
            let first = user_count == 0;
            if !first {
                let allow: bool = sqlx::query_scalar("SELECT allow_public_signups FROM global_settings WHERE id = 1")
                    .fetch_optional(pool)
                    .await
                    .map_err(db)?
                    .unwrap_or(true);
                if !allow {
                    return Err("signups_disabled");
                }
            }
            sqlx::query_scalar(
                "INSERT INTO users (email, name, avatar_url, is_super_admin) VALUES ($1, $2, $3, $4) RETURNING id",
            )
            .bind(email)
            .bind(name)
            .bind(picture)
            .bind(first)
            .fetch_one(pool)
            .await
            .map_err(db)?
        }
    };

    sqlx::query(
        "INSERT INTO identities (user_id, provider, provider_user_id) VALUES ($1, $2, $3)
         ON CONFLICT (provider, provider_user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(provider)
    .bind(subject)
    .execute(pool)
    .await
    .map_err(db)?;

    Ok(user_id)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub theme: Option<String>,
    pub locale: Option<String>,
    pub is_super_admin: bool,
}

async fn me_handler(State(state): State<AppState>, user: Option<AuthUser>) -> ApiResult<Json<serde_json::Value>> {
    let Some(user) = user else {
        return Ok(Json(serde_json::json!({ "authenticated": false })));
    };
    let profile: UserProfile = sqlx::query_as(
        "SELECT id, email, name, avatar_url, theme, locale, is_super_admin FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "authenticated": true, "user": profile })))
}

async fn logout_handler(session: Session) -> Response {
    let _ = session.flush().await;
    Redirect::to("/login").into_response()
}
