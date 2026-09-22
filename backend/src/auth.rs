use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
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
use std::env;
use tower_sessions::Session;

#[derive(Clone)]
pub struct AuthState {
    pub pool: PgPool,
    pub oidc_client: CoreClient,
}

pub async fn build_oidc_client() -> CoreClient {
    let issuer_url = IssuerUrl::new(env::var("OIDC_ISSUER").expect("OIDC_ISSUER missing")).unwrap();
    let client_id = ClientId::new(env::var("OIDC_CLIENT_ID").expect("OIDC_CLIENT_ID missing"));
    let client_secret = ClientSecret::new(env::var("OIDC_CLIENT_SECRET").expect("OIDC_CLIENT_SECRET missing"));
    let redirect_url = RedirectUrl::new(
        env::var("OIDC_REDIRECT_URL").unwrap_or_else(|_| "http://localhost:3001/api/auth/callback".to_string())
    ).unwrap();

    let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, async_http_client)
        .await
        .expect("Failed to discover OIDC provider");

    CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
        .set_redirect_uri(redirect_url)
}

pub fn auth_routes(state: AuthState) -> Router {
    Router::new()
        .route("/login", get(login_handler))
        .route("/callback", get(callback_handler))
        .route("/me", get(me_handler))
        .route("/logout", get(logout_handler))
        .with_state(state)
}

async fn login_handler(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = state
        .oidc_client
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

    Json(serde_json::json!({ "url": auth_url.as_str() }))
}

#[derive(Deserialize)]
struct AuthCallback {
    code: String,
    state: String,
}

async fn callback_handler(
    session: Session,
    State(state): State<AuthState>,
    Query(params): Query<AuthCallback>,
) -> impl IntoResponse {
    let stored_csrf: String = session.get("csrf_token").await.unwrap().unwrap_or_default();
    println!(">>> CALLBACK HANDLER HIT: code={}, state={}, stored_csrf={}", params.code, params.state, stored_csrf);
    if params.state != stored_csrf {
        return axum::response::Html(r#"<script>window.location.href = "/login?error=csrf_mismatch";</script>"#).into_response();
    }

    let pkce_secret_opt: Option<String> = session.get("pkce_verifier").await.unwrap();
    let pkce_secret = match pkce_secret_opt {
        Some(secret) => secret,
        None => return axum::response::Html(r#"<script>window.location.href = "/login?error=missing_pkce";</script>"#).into_response(),
    };
    let pkce_verifier = PkceCodeVerifier::new(pkce_secret);

    let token_response_res = state
        .oidc_client
        .exchange_code(AuthorizationCode::new(params.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await;

    let token_response = match token_response_res {
        Ok(t) => t,
        Err(e) => {
            println!("Failed to exchange code: {:?}", e);
            return axum::response::Html(r#"<script>window.location.href = "/login?error=exchange_failed";</script>"#).into_response();
        }
    };

    let access_token = token_response.access_token().clone();
    
    // Fetch user info
    let userinfo_res = state
        .oidc_client
        .user_info(access_token, None)
        .expect("Failed to create userinfo request")
        .request_async(async_http_client)
        .await;

    let userinfo: CoreUserInfoClaims = match userinfo_res {
        Ok(u) => u,
        Err(e) => {
            println!("Failed to fetch user info: {:?}", e);
            return axum::response::Html(r#"<script>window.location.href = "/login?error=userinfo_failed";</script>"#).into_response();
        }
    };

    let email = userinfo.email().map(|e| e.as_str().to_string()).unwrap_or_default();
    let name = userinfo.name().and_then(|n| n.get(None)).map(|n| n.as_str().to_string()).unwrap_or_default();
    let provider_user_id = userinfo.subject().as_str().to_string();

    // Insert or get user
    let user_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO users (email, name) VALUES ($1, $2)
         ON CONFLICT (email) DO UPDATE SET name = $2
         RETURNING id"
    )
    .bind(&email)
    .bind(&name)
    .fetch_one(&state.pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO identities (user_id, provider, provider_user_id) 
         VALUES ($1, 'zitadel', $2) 
         ON CONFLICT (provider, provider_user_id) DO NOTHING"
    )
    .bind(user_id)
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
}

async fn me_handler(session: Session, State(state): State<AuthState>) -> impl IntoResponse {
    let user_id: Option<uuid::Uuid> = session.get("user_id").await.unwrap();
    
    match user_id {
        Some(uid) => {
            let user = sqlx::query_as::<_, UserProfile>(
                "SELECT id, email, name FROM users WHERE id = $1"
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
