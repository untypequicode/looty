use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use openidconnect::{
    core::{CoreClient, CoreProviderMetadata, CoreResponseType, CoreUserInfoClaims},
    reqwest::async_http_client,
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, OAuth2TokenResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{env, sync::Arc};
use tower_sessions::Session;
use uuid::Uuid;
use dashmap::DashMap;

#[derive(Clone)]
pub struct AuthState {
    pub pool: PgPool,
    pub oidc_clients: Arc<DashMap<Uuid, CoreClient>>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ProviderDto {
    pub id: Uuid,
    pub name: String,
}

pub fn auth_routes(state: AuthState) -> Router {
    Router::new()
        .route("/providers", get(get_active_providers))
        .route("/login/:provider_id", get(login_handler))
        .route("/callback/:provider_id", get(callback_handler))
        .route("/me", get(me_handler))
        .route("/logout", get(logout_handler))
        .with_state(state)
}

async fn get_active_providers(State(state): State<AuthState>) -> impl IntoResponse {
    let providers = sqlx::query_as::<_, ProviderDto>(
        "SELECT id, name FROM oauth_providers WHERE is_active = TRUE"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Json(providers)
}

#[derive(sqlx::FromRow)]
struct ProviderConfig {
    issuer_url: String,
    client_id: String,
    client_secret: String,
}

async fn get_oidc_client(state: &AuthState, provider_id: Uuid) -> Result<CoreClient, String> {
    if let Some(client) = state.oidc_clients.get(&provider_id) {
        return Ok(client.clone());
    }

    let provider = sqlx::query_as::<_, ProviderConfig>(
        "SELECT issuer_url, client_id, client_secret FROM oauth_providers WHERE id = $1 AND is_active = TRUE"
    )
    .bind(provider_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    let provider = provider.ok_or_else(|| "Provider not found or inactive".to_string())?;

    let issuer_url = IssuerUrl::new(provider.issuer_url).map_err(|e| e.to_string())?;
    let client_id = ClientId::new(provider.client_id);
    let client_secret = ClientSecret::new(provider.client_secret);
    
    // We assume backend runs on localhost:8080 or behind a proxy. 
    // Ideally we should have a BASE_URL in env for the callback.
    let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());
    let redirect_url = RedirectUrl::new(format!("{}/api/auth/callback/{}", base_url, provider_id))
        .map_err(|e| e.to_string())?;

    let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, async_http_client)
        .await
        .map_err(|e| e.to_string())?;

    let client = CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
        .set_redirect_uri(redirect_url);

    state.oidc_clients.insert(provider_id, client.clone());

    Ok(client)
}

async fn login_handler(
    session: Session, 
    State(state): State<AuthState>,
    Path(provider_id): Path<Uuid>,
) -> impl IntoResponse {
    let client = match get_oidc_client(&state, provider_id).await {
        Ok(c) => c,
        Err(e) => return Json(serde_json::json!({ "error": e })).into_response(),
    };

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    session.insert("pkce_verifier", pkce_verifier.secret()).await.unwrap();
    session.insert("csrf_token", csrf_token.secret()).await.unwrap();
    session.insert("nonce", nonce.secret()).await.unwrap();

    Json(serde_json::json!({ "url": auth_url.as_str() })).into_response()
}

#[derive(Deserialize)]
struct AuthCallback {
    code: String,
    state: String,
}

async fn callback_handler(
    session: Session,
    State(state): State<AuthState>,
    Path(provider_id): Path<Uuid>,
    Query(params): Query<AuthCallback>,
) -> impl IntoResponse {
    let stored_csrf: String = session.get("csrf_token").await.unwrap().unwrap_or_default();
    if params.state != stored_csrf {
        return axum::response::Html(r#"<script>window.location.href = "/login?error=csrf_mismatch";</script>"#).into_response();
    }

    let client = match get_oidc_client(&state, provider_id).await {
        Ok(c) => c,
        Err(_) => return axum::response::Html(r#"<script>window.location.href = "/login?error=provider_error";</script>"#).into_response(),
    };

    let pkce_secret_opt: Option<String> = session.get("pkce_verifier").await.unwrap();
    let pkce_secret = match pkce_secret_opt {
        Some(secret) => secret,
        None => return axum::response::Html(r#"<script>window.location.href = "/login?error=missing_pkce";</script>"#).into_response(),
    };
    let pkce_verifier = PkceCodeVerifier::new(pkce_secret);

    let token_response_res = client
        .exchange_code(AuthorizationCode::new(params.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await;

    let token_response = match token_response_res {
        Ok(t) => t,
        Err(_) => {
            return axum::response::Html(r#"<script>window.location.href = "/login?error=exchange_failed";</script>"#).into_response();
        }
    };

    let access_token = token_response.access_token().clone();
    
    let userinfo_res = client
        .user_info(access_token, None)
        .expect("Failed to create userinfo request")
        .request_async(async_http_client)
        .await;

    let userinfo: CoreUserInfoClaims = match userinfo_res {
        Ok(u) => u,
        Err(_) => {
            return axum::response::Html(r#"<script>window.location.href = "/login?error=userinfo_failed";</script>"#).into_response();
        }
    };

    let email = userinfo.email().map(|e| e.as_str().to_string()).unwrap_or_default();
    let name = userinfo.name().and_then(|n| n.get(None)).map(|n| n.as_str().to_string()).unwrap_or_default();
    let provider_user_id = userinfo.subject().as_str().to_string();
    let provider_name = provider_id.to_string(); // we use provider_id as string for identities table provider column

    let user_count: i64 = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users").fetch_one(&state.pool).await.unwrap_or(0);
    let is_super_admin = user_count == 0;

    let user_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO users (email, name, is_super_admin) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO UPDATE SET name = $2
         RETURNING id"
    )
    .bind(&email)
    .bind(&name)
    .bind(is_super_admin)
    .fetch_one(&state.pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO identities (user_id, provider, provider_user_id) 
         VALUES ($1, $2, $3) 
         ON CONFLICT (provider, provider_user_id) DO NOTHING"
    )
    .bind(user_id)
    .bind(&provider_name)
    .bind(&provider_user_id)
    .execute(&state.pool)
    .await
    .unwrap();

    session.insert("user_id", user_id).await.unwrap();

    axum::response::Html(r#"<script>window.location.href = "/";</script>"#).into_response()
}

#[derive(Serialize, sqlx::FromRow)]
struct UserProfile {
    id: uuid::Uuid,
    email: String,
    name: String,
    is_super_admin: bool,
}

async fn me_handler(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    let user_id: Option<uuid::Uuid> = session.get("user_id").await.unwrap_or(None);
    
    match user_id {
        Some(uid) => {
            let user = sqlx::query_as::<_, UserProfile>(
                "SELECT id, email, name, is_super_admin FROM users WHERE id = $1"
            )
            .bind(uid)
            .fetch_one(&state.pool)
            .await;

            match user {
                Ok(u) => Json(serde_json::json!({ "authenticated": true, "user": u })),
                Err(_) => Json(serde_json::json!({ "authenticated": false }))
            }
        },
        None => Json(serde_json::json!({ "authenticated": false }))
    }
}

async fn logout_handler(session: Session) -> impl IntoResponse {
    session.delete().await.unwrap();
    axum::response::Html(r#"<script>window.location.href = "/login";</script>"#).into_response()
}
