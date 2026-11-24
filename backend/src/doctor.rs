//! Diagnostic de l'instance (issue 164, ADR 0015) : page d'administration et commande `looty doctor`.
//!
//! Les contrôles qui se font depuis le serveur sont ici ; ceux qui doivent partir du navigateur (WebSocket
//! `/api/ws` et `/collab`, adresse affichée) sont faits par la page. Chaque contrôle rend `ok`, `warn`, `error`
//! ou `skip`, un code court que l'interface traduit, et un détail technique (valeur lue, message d'erreur).

use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use serde::Serialize;
use sqlx::PgPool;
use std::time::Duration;

use crate::access::AuthUser;
use crate::admin::require_super_admin;
use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Error,
    Skip,
}

#[derive(Serialize, Clone, Debug)]
pub struct Check {
    pub id: &'static str,
    pub status: Status,
    /// Code du message à traduire (vide : le détail suffit).
    pub code: &'static str,
    pub detail: String,
}

fn check(id: &'static str, status: Status, code: &'static str, detail: impl Into<String>) -> Check {
    Check { id, status, code, detail: detail.into() }
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(run_http))
}

async fn run_http(State(state): State<AppState>, headers: HeaderMap, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    require_super_admin(&user)?;
    let checks = run(Some(&state.pool), &state.base_url, state.local_accounts, Some(&headers)).await;
    Ok(Json(serde_json::json!({ "version": env!("CARGO_PKG_VERSION"), "base_url": state.base_url, "checks": checks })))
}

/// Tous les contrôles côté serveur. `headers` : ceux de la requête de la page (absents en ligne de commande).
pub async fn run(pool: Option<&PgPool>, base_url: &str, local_accounts: bool, headers: Option<&HeaderMap>) -> Vec<Check> {
    let mut out = vec![check_base_url(base_url)];
    out.push(check_reachable(base_url).await);
    out.push(check_proxy(base_url, headers));
    out.push(check_caldav(base_url).await);
    match pool {
        Some(pool) => {
            out.extend(check_oidc(pool, base_url, local_accounts).await);
            out.push(check_smtp(pool).await);
            out.push(check_database(pool).await);
        }
        None => out.push(check("database", Status::Error, "db_unreachable", "")),
    }
    out.push(check_files());
    out.push(check_secret_key());
    out.push(check_version().await);
    out
}

fn check_base_url(base: &str) -> Check {
    let Ok(url) = reqwest::Url::parse(base) else {
        return check("base_url", Status::Error, "base_url_invalid", base);
    };
    let host = url.host_str().unwrap_or("");
    let local = matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]");
    if !matches!(url.scheme(), "http" | "https") || url.path() != "/" && !url.path().is_empty() || base.ends_with('/') {
        return check("base_url", Status::Error, "base_url_shape", base);
    }
    if local {
        return check("base_url", Status::Warn, "base_url_local", base);
    }
    if url.scheme() == "http" {
        return check("base_url", Status::Warn, "base_url_http", base);
    }
    check("base_url", Status::Ok, "", base)
}

fn client(redirects: bool) -> Option<reqwest::Client> {
    let mut b = reqwest::Client::builder().timeout(Duration::from_secs(8)).user_agent("looty-doctor");
    if !redirects {
        b = b.redirect(reqwest::redirect::Policy::none());
    }
    b.build().ok()
}

fn chain(e: &dyn std::error::Error) -> String {
    let mut out = e.to_string();
    let mut source = e.source();
    while let Some(cause) = source {
        out.push_str(" : ");
        out.push_str(&cause.to_string());
        source = cause.source();
    }
    out
}

/// L'adresse publique répond, avec un certificat valide, et c'est bien ce serveur.
async fn check_reachable(base: &str) -> Check {
    let Some(http) = client(false) else { return check("reachable", Status::Error, "client", "") };
    match http.get(format!("{base}/api/health")).send().await {
        Ok(r) if r.status().is_success() => {
            let version = r.json::<serde_json::Value>().await.ok().and_then(|v| v["version"].as_str().map(str::to_string));
            match version {
                Some(v) if v == env!("CARGO_PKG_VERSION") => check("reachable", Status::Ok, "", format!("Looty {v}")),
                Some(v) => check("reachable", Status::Warn, "other_version", v),
                None => check("reachable", Status::Warn, "not_looty", base),
            }
        }
        Ok(r) => check("reachable", Status::Warn, "bad_status", r.status().to_string()),
        Err(e) => {
            let text = chain(&e);
            let lower = text.to_lowercase();
            if lower.contains("certificate") || lower.contains("cert ") || lower.contains("tls") || lower.contains("ssl") {
                check("reachable", Status::Error, "tls_invalid", text)
            } else {
                // Depuis un conteneur, l'adresse publique n'est pas toujours joignable (boucle NAT) : la page
                // confirme depuis le navigateur.
                check("reachable", Status::Warn, "self_unreachable", text)
            }
        }
    }
}

/// En-têtes du proxy inverse, tels que cette requête les a reçus (page d'administration seulement).
fn check_proxy(base: &str, headers: Option<&HeaderMap>) -> Check {
    let Some(h) = headers else { return check("proxy", Status::Skip, "needs_browser", "") };
    let get = |n: &str| h.get(n).and_then(|v| v.to_str().ok()).map(str::to_string);
    let base_host = reqwest::Url::parse(base).ok().and_then(|u| u.host_str().map(str::to_string).map(|h| match u.port() { Some(p) => format!("{h}:{p}"), None => h }));
    let seen = get("x-forwarded-host").or_else(|| get("host"));
    if let (Some(b), Some(s)) = (&base_host, &seen) {
        if !b.eq_ignore_ascii_case(s) {
            return check("proxy", Status::Error, "host_mismatch", format!("BASE_URL {b}, request {s}"));
        }
    }
    if base.starts_with("https://") && get("x-forwarded-proto").as_deref() != Some("https") {
        return check("proxy", Status::Warn, "no_forwarded_proto", "");
    }
    if get("x-forwarded-for").is_none() {
        return check("proxy", Status::Warn, "no_forwarded_for", "");
    }
    check("proxy", Status::Ok, "", seen.unwrap_or_default())
}

/// Découverte CalDAV : `/.well-known/caldav` doit rediriger vers le serveur CalDAV (ou répondre directement).
async fn check_caldav(base: &str) -> Check {
    let Some(http) = client(false) else { return check("caldav", Status::Error, "client", "") };
    match http.get(format!("{base}/.well-known/caldav")).send().await {
        Ok(r) if r.status().is_redirection() => {
            let to = r.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
            check("caldav", Status::Ok, "", to)
        }
        Ok(r) if matches!(r.status().as_u16(), 207 | 401) => check("caldav", Status::Ok, "", r.status().to_string()),
        Ok(r) => check("caldav", Status::Warn, "caldav_bad_status", r.status().to_string()),
        Err(e) => check("caldav", Status::Warn, "self_unreachable", chain(&e)),
    }
}

/// Un contrôle par fournisseur OpenID Connect actif : l'émetteur répond à la découverte, et l'adresse de retour.
async fn check_oidc(pool: &PgPool, base: &str, local_accounts: bool) -> Vec<Check> {
    let providers: Vec<(uuid::Uuid, String, String)> = sqlx::query_as("SELECT id, name, issuer_url FROM oauth_providers WHERE is_active ORDER BY name").fetch_all(pool).await.unwrap_or_default();
    if providers.is_empty() {
        return vec![if local_accounts { check("oidc", Status::Skip, "oidc_none_local", "") } else { check("oidc", Status::Error, "oidc_none", "") }];
    }
    let http = crate::auth::oidc_http();
    let mut out = Vec::new();
    for (id, name, issuer) in providers {
        let url = format!("{}/.well-known/openid-configuration", issuer.trim_end_matches('/'));
        let redirect = format!("{base}/api/auth/callback/{id}");
        let res = match http.get(&url).send().await {
            Ok(r) if r.status().is_success() => match r.bytes().await.map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).map_err(|e| e.to_string())) {
                Ok(v) if v["authorization_endpoint"].is_string() && v["issuer"].as_str().map(|i| i.trim_end_matches('/')) == Some(issuer.trim_end_matches('/')) => {
                    check("oidc", Status::Ok, "oidc_ok", format!("{name} · {redirect}"))
                }
                Ok(v) => check("oidc", Status::Error, "oidc_issuer_mismatch", format!("{name}: {} ≠ {issuer}", v["issuer"].as_str().unwrap_or("?"))),
                Err(e) => check("oidc", Status::Error, "oidc_not_discovery", format!("{name}: {e}")),
            },
            Ok(r) => check("oidc", Status::Error, "oidc_unreachable", format!("{name}: HTTP {}", r.status())),
            Err(e) => check("oidc", Status::Error, "oidc_unreachable", format!("{name}: {}", chain(&e))),
        };
        out.push(res);
    }
    out
}

async fn check_smtp(pool: &PgPool) -> Check {
    let Some(c) = crate::mail::config(pool).await else { return check("smtp", Status::Skip, "smtp_off", "") };
    match crate::mail::test_connection(&c).await {
        Ok(()) => check("smtp", Status::Ok, "", format!("{}:{}", c.host, c.port)),
        Err(e) => check("smtp", Status::Error, "smtp_failed", format!("{}:{} : {e}", c.host, c.port)),
    }
}

async fn check_database(pool: &PgPool) -> Check {
    let row: Result<(String, i64), _> = sqlx::query_as("SELECT current_setting('server_version'), pg_database_size(current_database())").fetch_one(pool).await;
    match row {
        Ok((version, size)) => {
            let major: u32 = version.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0);
            let detail = format!("PostgreSQL {version} · {} MB", size / 1_048_576);
            if major < 14 { check("database", Status::Warn, "db_old", detail) } else { check("database", Status::Ok, "", detail) }
        }
        Err(e) => check("database", Status::Error, "db_unreachable", e.to_string()),
    }
}

/// Dossier des fichiers : écriture possible, espace libre.
fn check_files() -> Check {
    let dir = crate::upload::UPLOAD_DIR;
    let probe = std::path::Path::new(dir).join(".doctor-probe");
    if std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&probe, b"ok")).is_err() {
        return check("files", Status::Error, "files_not_writable", dir);
    }
    let _ = std::fs::remove_file(&probe);
    match crate::storage::disk_space(dir) {
        Some((total, avail)) => {
            let detail = format!("{} GB free of {} GB", avail / 1_000_000_000, total / 1_000_000_000);
            if avail < 200_000_000 {
                check("files", Status::Error, "disk_full", detail)
            } else if avail < 2_000_000_000 || avail * 10 < total {
                check("files", Status::Warn, "disk_low", detail)
            } else {
                check("files", Status::Ok, "", detail)
            }
        }
        None => check("files", Status::Ok, "", dir),
    }
}

/// Sans `LOOTY_SECRET_KEY`, la clé de chiffrement est un fichier du volume : à sauvegarder avec lui.
fn check_secret_key() -> Check {
    if std::env::var("LOOTY_SECRET_KEY").is_ok_and(|k| !k.trim().is_empty()) {
        check("secret_key", Status::Ok, "", "")
    } else {
        check("secret_key", Status::Warn, "secret_key_file", "")
    }
}

async fn check_version() -> Check {
    let u = crate::status::update_status().await;
    let current = env!("CARGO_PKG_VERSION");
    if !u["checked"].as_bool().unwrap_or(false) {
        return check("version", Status::Skip, "update_off", current);
    }
    if u["error"].as_bool().unwrap_or(false) {
        return check("version", Status::Warn, "update_error", current);
    }
    match (u["available"].as_bool(), u["latest"].as_str()) {
        (Some(true), Some(l)) => check("version", Status::Warn, "update_available", format!("{current} → {l}")),
        _ => check("version", Status::Ok, "", current),
    }
}

// ---------------------------------------------------------------------------
// Commande `looty doctor`
// ---------------------------------------------------------------------------

/// `looty doctor [--json]` : même contrôles que la page, sans les contrôles du navigateur. Code de sortie 1 s'il y a une erreur.
pub async fn cli(args: &[String]) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let base = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".into());
    let local = crate::local_auth::env_enabled();
    // Sans migrer : le diagnostic ne modifie rien.
    let pool = match std::env::var("DATABASE_URL") {
        Ok(url) => sqlx::postgres::PgPoolOptions::new().max_connections(2).acquire_timeout(Duration::from_secs(8)).connect(&url).await.ok(),
        Err(_) => None,
    };
    let checks = run(pool.as_ref(), &base, local, None).await;
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "version": env!("CARGO_PKG_VERSION"), "base_url": base, "checks": checks })).unwrap_or_default());
    } else {
        println!("Looty {} · {base}\n", env!("CARGO_PKG_VERSION"));
        for c in &checks {
            let tag = match c.status { Status::Ok => "OK   ", Status::Warn => "WARN ", Status::Error => "ERROR", Status::Skip => "SKIP " };
            let code = if c.code.is_empty() { String::new() } else { format!(" [{}]", c.code) };
            println!("{tag} {}{code} {}", c.id, c.detail);
        }
        println!("\nDetails and fixes: https://looty.untypequicode.fr/docs/host/operate/troubleshooting/");
    }
    if checks.iter().any(|c| c.status == Status::Error) { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_url_is_checked_for_shape_and_scheme() {
        assert_eq!(check_base_url("https://looty.example.org").status, Status::Ok);
        assert_eq!(check_base_url("http://looty.example.org").code, "base_url_http");
        assert_eq!(check_base_url("http://localhost:3000").code, "base_url_local");
        assert_eq!(check_base_url("https://looty.example.org/").code, "base_url_shape");
        assert_eq!(check_base_url("https://looty.example.org/app").code, "base_url_shape");
        assert_eq!(check_base_url("looty.example.org").code, "base_url_invalid");
    }

    #[test]
    fn proxy_headers_are_read_from_the_request() {
        let base = "https://looty.example.org";
        assert_eq!(check_proxy(base, None).status, Status::Skip);
        let mut h = HeaderMap::new();
        h.insert("host", "looty.example.org".parse().unwrap());
        assert_eq!(check_proxy(base, Some(&h)).code, "no_forwarded_proto");
        h.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(check_proxy(base, Some(&h)).code, "no_forwarded_for");
        h.insert("x-forwarded-for", "203.0.113.4".parse().unwrap());
        assert_eq!(check_proxy(base, Some(&h)).status, Status::Ok);
        h.insert("host", "backend:8080".parse().unwrap());
        assert_eq!(check_proxy(base, Some(&h)).code, "host_mismatch");
    }

    #[sqlx::test]
    async fn the_database_check_reports_the_version(pool: PgPool) {
        let c = check_database(&pool).await;
        assert_eq!(c.status, Status::Ok);
        assert!(c.detail.starts_with("PostgreSQL "));
    }

    #[sqlx::test]
    async fn oidc_is_an_error_without_provider_unless_local_accounts_are_on(pool: PgPool) {
        assert_eq!(check_oidc(&pool, "https://x.org", false).await[0].code, "oidc_none");
        assert_eq!(check_oidc(&pool, "https://x.org", true).await[0].status, Status::Skip);
    }

    #[sqlx::test]
    async fn an_unreachable_issuer_is_an_error_with_the_return_address(pool: PgPool) {
        sqlx::query("INSERT INTO oauth_providers (name, issuer_url, client_id, client_secret) VALUES ('SSO', 'http://127.0.0.1:9', 'id', 's')").execute(&pool).await.unwrap();
        let c = &check_oidc(&pool, "https://x.org", false).await[0];
        assert_eq!((c.status, c.code), (Status::Error, "oidc_unreachable"));
    }

    #[sqlx::test]
    async fn smtp_is_skipped_when_not_configured(pool: PgPool) {
        let c = check_smtp(&pool).await;
        assert_eq!((c.status, c.code), (Status::Skip, "smtp_off"));
    }

    #[test]
    fn the_files_check_reads_the_upload_folder() {
        let c = check_files();
        assert!(matches!(c.status, Status::Ok | Status::Warn), "{c:?}");
    }

    #[test]
    fn a_missing_secret_key_is_a_warning() {
        // La variable n'est pas définie dans les tests.
        if std::env::var("LOOTY_SECRET_KEY").is_err() {
            assert_eq!(check_secret_key().code, "secret_key_file");
        }
    }

    #[sqlx::test]
    async fn a_closed_local_port_is_not_reachable_but_not_fatal(_pool: PgPool) {
        let c = check_reachable("http://127.0.0.1:9").await;
        assert_eq!((c.status, c.code), (Status::Warn, "self_unreachable"));
    }
}

#[cfg(test)]
mod http_tests {
    use axum::{body::Body, http::Request};
    use sqlx::PgPool;
    use tower::ServiceExt;
    use uuid::Uuid;

    #[sqlx::test]
    async fn the_doctor_page_is_for_administrators_and_lists_every_server_check(pool: PgPool) {
        crate::session_store::PgSessionStore::new(pool.clone()).migrate().await.unwrap();
        let app = crate::app(crate::state::AppState::new(pool.clone(), "https://looty.example.org".into()));
        let mut tokens = Vec::new();
        for (email, admin) in [("root@example.org", true), ("ana@example.org", false)] {
            let id: Uuid = sqlx::query_scalar("INSERT INTO users (email, name, is_super_admin) VALUES ($1, 'x', $2) RETURNING id").bind(email).bind(admin).fetch_one(&pool).await.unwrap();
            let raw = format!("looty_{}", Uuid::new_v4().simple());
            sqlx::query("INSERT INTO api_tokens (user_id, name, token_hash, scope, access) VALUES ($1, 't', $2, 'api', 'write')").bind(id).bind(crate::access::hash_token(&raw)).execute(&pool).await.unwrap();
            tokens.push(raw);
        }
        let get = |token: String| {
            let app = app.clone();
            async move {
                let req = Request::builder().uri("/api/admin/doctor").header("authorization", format!("Bearer {token}")).header("host", "backend:8080").body(Body::empty()).unwrap();
                let res = app.oneshot(req).await.unwrap();
                let status = res.status().as_u16();
                (status, serde_json::from_slice::<serde_json::Value>(&axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap_or_default())
            }
        };
        assert_eq!(get(tokens[1].clone()).await.0, 403);
        let (status, body) = get(tokens[0].clone()).await;
        assert_eq!(status, 200);
        let ids: Vec<&str> = body["checks"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
        for id in ["base_url", "reachable", "proxy", "caldav", "oidc", "smtp", "database", "files", "secret_key", "version"] {
            assert!(ids.contains(&id), "{id} missing from {ids:?}");
        }
        // L'en-tête Host de la requête ne correspond pas à BASE_URL : le contrôle du proxy le signale.
        let proxy = body["checks"].as_array().unwrap().iter().find(|c| c["id"] == "proxy").unwrap();
        assert_eq!((proxy["status"].as_str(), proxy["code"].as_str()), (Some("error"), Some("host_mismatch")));
    }
}
