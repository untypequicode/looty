use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use sha2::{Digest, Sha256};
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
        .route("/dev-login", get(dev_login_handler))
        .route("/desktop/start", get(desktop_start))
        .route("/desktop/complete", get(desktop_complete))
        .route("/desktop/exchange", post(desktop_exchange))
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
    let url = begin_login(&session, &state, provider_id, q.redirect.as_deref()).await?;
    Ok(Json(serde_json::json!({ "url": url })))
}

/// Prépare la connexion OIDC (PKCE, CSRF, nonce en session) et renvoie l'URL du fournisseur.
async fn begin_login(session: &Session, state: &AppState, provider_id: Uuid, redirect: Option<&str>) -> ApiResult<String> {
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
        session.insert("post_login_redirect", safe_redirect(redirect)).await
    };
    store.await.map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(auth_url.to_string())
}

// ---------------------------------------------------------------------------
// Application de bureau : connexion dans le navigateur système (RFC 8252)
// ---------------------------------------------------------------------------
//
// Le moteur web de l'application (WebKitGTK) ne gère pas WebAuthn (clés d'accès,
// empreinte…). L'application ouvre donc le navigateur sur `desktop/start` ; une fois
// connecté, `desktop/complete` renvoie le navigateur vers l'application
// (http://127.0.0.1:<port>, uniquement) avec un code à usage unique, que l'application
// échange contre sa propre session (`desktop/exchange`) en prouvant qu'elle détient le
// secret PKCE dont le navigateur n'a vu que l'empreinte.

#[derive(Deserialize)]
struct DesktopParams {
    port: u16,
    state: String,
    challenge: String,
    provider: Option<Uuid>,
}

fn token_ok(s: &str) -> bool {
    (16..=128).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl DesktopParams {
    fn valid(&self) -> bool {
        self.port >= 1024 && token_ok(&self.state) && token_ok(&self.challenge)
    }
    fn complete_path(&self) -> String {
        format!("/api/auth/desktop/complete?port={}&state={}&challenge={}", self.port, self.state, self.challenge)
    }
}

async fn desktop_start(session: Session, State(state): State<AppState>, Query(p): Query<DesktopParams>) -> Response {
    if !p.valid() {
        return login_error("invalid_request");
    }
    let Some(provider) = p.provider else {
        return Redirect::to(&format!("/login?redirect={}", urlencode(&p.complete_path()))).into_response();
    };
    match begin_login(&session, &state, provider, Some(&p.complete_path())).await {
        Ok(url) => Redirect::to(&url).into_response(),
        Err(_) => login_error("provider_error"),
    }
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

async fn desktop_complete(State(state): State<AppState>, user: Option<AuthUser>, Query(p): Query<DesktopParams>) -> Response {
    if !p.valid() {
        return login_error("invalid_request");
    }
    let Some(user) = user else {
        return Redirect::to(&format!("/login?redirect={}", urlencode(&p.complete_path()))).into_response();
    };
    let mut raw = [0u8; 32];
    rand::Rng::fill(&mut rand::thread_rng(), &mut raw);
    let code = hex::encode(raw);
    let _ = sqlx::query("DELETE FROM desktop_login_codes WHERE expires_at < NOW()").execute(&state.pool).await;
    let stored = sqlx::query(
        "INSERT INTO desktop_login_codes (code_hash, user_id, challenge, expires_at) VALUES ($1, $2, $3, NOW() + interval '2 minutes')",
    )
    .bind(crate::access::hash_token(&code))
    .bind(user.id)
    .bind(&p.challenge)
    .execute(&state.pool)
    .await;
    if stored.is_err() {
        return login_error("database_error");
    }
    // Destination fixe : la boucle locale de l'application (jamais un autre hôte).
    Redirect::to(&format!("http://127.0.0.1:{}/__looty/sso?code={}&state={}", p.port, code, p.state)).into_response()
}

#[derive(Deserialize)]
struct DesktopExchange {
    code: String,
    verifier: String,
}

async fn desktop_exchange(session: Session, State(state): State<AppState>, Json(p): Json<DesktopExchange>) -> ApiResult<Json<serde_json::Value>> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "DELETE FROM desktop_login_codes WHERE code_hash = $1 AND expires_at > NOW() RETURNING user_id, challenge",
    )
    .bind(crate::access::hash_token(p.code.trim()))
    .fetch_optional(&state.pool)
    .await?;
    let (user_id, challenge) = row.ok_or(ApiError::Unauthorized)?;
    let computed = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(p.verifier.as_bytes()));
    if computed != challenge {
        return Err(ApiError::Unauthorized);
    }
    let suspended: bool = sqlx::query_scalar("SELECT is_suspended FROM users WHERE id = $1").bind(user_id).fetch_one(&state.pool).await?;
    if suspended {
        return Err(ApiError::Forbidden);
    }
    session.cycle_id().await.map_err(|e| ApiError::Internal(e.to_string()))?;
    session.insert(SESSION_USER_KEY, user_id).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
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


#[derive(Deserialize)]
struct DevLoginQuery {
    email: String,
    name: Option<String>,
    redirect: Option<String>,
}

/// Connexion de développement SANS mot de passe, uniquement si `LOOTY_DEV_LOGIN=true`.
/// Ne jamais activer en production.
async fn dev_login_handler(
    session: Session,
    State(state): State<AppState>,
    Query(q): Query<DevLoginQuery>,
) -> Response {
    if std::env::var("LOOTY_DEV_LOGIN").as_deref() != Ok("true") {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    }
    let email = q.email.trim().to_lowercase();
    let name = q.name.unwrap_or_else(|| email.split('@').next().unwrap_or("dev").to_string());
    let first: bool = sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM users)")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(false);
    let id: Result<Uuid, _> = sqlx::query_scalar(
        "INSERT INTO users (email, name, is_super_admin) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO UPDATE SET email = EXCLUDED.email RETURNING id",
    )
    .bind(&email)
    .bind(&name)
    .bind(first)
    .fetch_one(&state.pool)
    .await;
    let Ok(id) = id else { return login_error("database_error") };
    if session.cycle_id().await.is_err() || session.insert(SESSION_USER_KEY, id).await.is_err() {
        return login_error("session_error");
    }
    Redirect::to(&safe_redirect(q.redirect.as_deref())).into_response()
}
