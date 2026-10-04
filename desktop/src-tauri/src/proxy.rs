//! Proxy local entre la fenêtre et le serveur Looty.
//!
//! La vue web charge `http://127.0.0.1:<port>/` : l'interface, l'API, les fichiers et
//! les WebSockets (temps réel, édition collaborative) sont relayés vers le serveur,
//! avec la session détenue par Rust. C'est aussi le point où se grefferont le cache
//! et la file d'attente hors ligne.
//!
//! Sécurité : le port local est joignable par les autres programmes et par les sites
//! ouverts dans un navigateur. Seule la fenêtre de l'application est servie :
//! - l'en-tête Host doit être l'adresse locale (protection contre le DNS rebinding) ;
//! - chaque requête doit porter le cookie `looty_local`, posé au démarrage via une URL
//!   à secret aléatoire. Il n'existe que dans le profil WebKit de l'application (jamais dans
//!   un navigateur) ; SameSite=Lax empêche les pages tierces affichées pendant une connexion
//!   SSO de l'utiliser en arrière-plan, tout en l'envoyant au retour de la chaîne de
//!   redirections du fournisseur (qu'une valeur Strict bloquerait).

use axum::{
    body::Body,
    extract::{ws::WebSocketUpgrade, FromRequestParts, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Redirect, Response},
    Router,
};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};
use url::Url;

use crate::config::{self, Config, Jar};

pub const COOKIE: &str = "looty_local";

pub struct Proxy {
    pub port: u16,
    pub secret: String,
    pub config: RwLock<Config>,
    pub jar: Jar,
    client: reqwest::Client,
    /// Connexion SSO en cours (jusqu'à cette échéance) : la fenêtre peut parcourir toute
    /// la chaîne de fournisseurs (ex. Zitadel → GitHub → Zitadel) sans en sortir.
    sign_in: Mutex<Option<Instant>>,
    /// Connexion en cours dans le navigateur système (RFC 8252).
    sso: Mutex<Option<PendingSso>>,
    /// Le serveur sait-il connecter l'application via le navigateur ? (par serveur)
    desktop_sso: Mutex<Option<(String, bool)>>,
    pub hooks: OnceLock<Hooks>,
}

/// Actions confiées à l'application (navigateur système, fenêtre).
pub struct Hooks {
    pub open_url: Box<dyn Fn(&str) + Send + Sync>,
    pub signed_in: Box<dyn Fn() + Send + Sync>,
    /// Fenêtre d'ajout rapide fermée (titre de la tâche si elle a été créée).
    pub quick_add_closed: Box<dyn Fn(Option<String>) + Send + Sync>,
}

struct PendingSso {
    state: String,
    verifier: String,
    provider: String,
    redirect: String,
    browser_url: String,
    done: bool,
    error: Option<String>,
}

fn random_token() -> String {
    use base64::Engine;
    let mut b = [0u8; 32];
    rand::Rng::fill(&mut rand::thread_rng(), &mut b);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// Durée maximale d'une connexion SSO.
const SIGN_IN_WINDOW: Duration = Duration::from_secs(600);

impl Proxy {
    pub fn server(&self) -> Option<Url> {
        self.config.read().ok()?.server.as_deref().and_then(|s| Url::parse(s).ok())
    }
    pub fn local_origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
    /// Client HTTP partageant la session (magasin de cookies) de l'application.
    pub fn http(&self) -> &reqwest::Client {
        &self.client
    }
    /// En-tête Cookie de la session pour une URL du serveur (WebSockets).
    pub fn cookie_header(&self, url: &Url) -> String {
        self.jar
            .lock()
            .map(|s| s.get_request_values(url).map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; "))
            .unwrap_or_default()
    }
    pub fn start_sign_in(&self) {
        *self.sign_in.lock().unwrap() = Some(Instant::now() + SIGN_IN_WINDOW);
    }
    pub fn signing_in(&self) -> bool {
        self.sign_in.lock().unwrap().is_some_and(|until| Instant::now() < until)
    }
    pub fn end_sign_in(&self) {
        *self.sign_in.lock().unwrap() = None;
    }
    /// URL d'amorçage de la fenêtre (pose le cookie d'accès).
    pub fn boot_url(&self) -> String {
        format!("{}/__looty/boot?k={}", self.local_origin(), self.secret)
    }
}

pub type Shared = Arc<Proxy>;

pub async fn start(config: Config, jar: Jar) -> std::io::Result<Shared> {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let port = listener.local_addr()?.port();
    let mut secret = [0u8; 32];
    rand::Rng::fill(&mut rand::thread_rng(), &mut secret);
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Looty-Desktop/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("http client");
    let proxy = Arc::new(Proxy {
        port,
        secret: hex::encode(secret),
        config: RwLock::new(config),
        jar,
        client,
        sign_in: Mutex::new(None),
        sso: Mutex::new(None),
        desktop_sso: Mutex::new(None),
        hooks: OnceLock::new(),
    });
    let app = Router::new().fallback(handle).with_state(proxy.clone());
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("local proxy stopped: {e}");
        }
    });
    tracing::info!("local proxy on 127.0.0.1:{port}");
    Ok(proxy)
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "Forbidden").into_response()
}

fn has_access_cookie(headers: &HeaderMap, secret: &str) -> bool {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .any(|c| c.trim() == format!("{COOKIE}={secret}"))
}

async fn handle(State(p): State<Shared>, req: Request) -> Response {
    // 1. Host local uniquement
    let host = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("");
    if host != format!("127.0.0.1:{}", p.port) && host != format!("localhost:{}", p.port) {
        return forbidden();
    }
    let path = req.uri().path().to_string();

    // 2. Amorçage : échange le secret de l'URL contre le cookie d'accès
    if path == "/__looty/boot" {
        let ok = req.uri().query().unwrap_or("").split('&').any(|kv| kv == format!("k={}", p.secret));
        if !ok {
            return forbidden();
        }
        // Tests automatisés (versions de débogage uniquement) : page de départ imposée.
        let start = if cfg!(debug_assertions) { std::env::var("LOOTY_START_PATH").ok().filter(|v| v.starts_with('/')) } else { None };
        let mut res = Redirect::to(start.as_deref().unwrap_or("/")).into_response();
        let cookie = format!("{COOKIE}={}; Path=/; HttpOnly; SameSite=Lax", p.secret);
        res.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
        return res;
    }
    // Retour du navigateur système après connexion : il n'a pas le cookie d'accès, le
    // paramètre `state` (aléatoire, propre à cette connexion) en tient lieu.
    if path == "/__looty/sso" {
        let fr = req.headers().get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.trim_start().starts_with("fr"));
        return sso_return(&p, fr, query_param(&req, "code"), query_param(&req, "state")).await;
    }
    if !has_access_cookie(req.headers(), &p.secret) {
        return forbidden();
    }
    // Bouton SSO de la page de connexion : connexion dans le navigateur système
    // (WebKitGTK ne gère pas WebAuthn), sauf serveur trop ancien.
    if req.method() == Method::GET && path.starts_with("/api/auth/login/") {
        if let Some(server) = p.server() {
            if supports_desktop_sso(&p, &server).await {
                return start_browser_sso(&p, &server, &req);
            }
        }
    }

    // 3. Pages propres à l'application
    if let Some(rest) = path.strip_prefix("/__looty/") {
        return local_route(&p, rest, req).await;
    }
    let Some(server) = p.server() else {
        return setup_page();
    };

    // 4. Relais vers le serveur
    let is_ws = req.headers().get(header::UPGRADE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    if is_ws {
        return proxy_ws(p, server, req).await;
    }
    match proxy_http(&p, &server, req).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("{path}: {e}");
            offline_response(&path)
        }
    }
}

fn html(body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], body).into_response()
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

fn query_param(req: &Request, key: &str) -> Option<String> {
    req.uri().query().and_then(|q| url::form_urlencoded::parse(q.as_bytes()).find(|(k, _)| k == key).map(|(_, v)| v.into_owned()))
}

async fn supports_desktop_sso(p: &Shared, server: &Url) -> bool {
    let key = server.origin().ascii_serialization();
    if let Some((k, v)) = p.desktop_sso.lock().unwrap().as_ref() {
        if *k == key {
            return *v;
        }
    }
    let ok = async {
        let r = p.client.get(format!("{key}/api/health")).timeout(std::time::Duration::from_secs(5)).send().await.ok()?;
        let v: serde_json::Value = r.json().await.ok()?;
        Some(v["desktop_sso"] == true)
    }
    .await;
    // En cas d'échec réseau, on ne mémorise rien (nouvel essai au prochain clic).
    if let Some(v) = ok {
        *p.desktop_sso.lock().unwrap() = Some((key, v));
    }
    ok.unwrap_or(false)
}

/// Ouvre le navigateur système sur le fournisseur et fait patienter la fenêtre.
fn start_browser_sso(p: &Shared, server: &Url, req: &Request) -> Response {
    use base64::Engine;
    use sha2::Digest;
    let provider = req.uri().path().trim_start_matches("/api/auth/login/").to_string();
    let redirect = query_param(req, "redirect").filter(|r| r.starts_with('/') && !r.starts_with("//")).unwrap_or_else(|| "/".into());
    let state = random_token();
    let verifier = random_token();
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    let browser_url = format!(
        "{}/api/auth/desktop/start?provider={}&port={}&state={}&challenge={}",
        server.origin().ascii_serialization(),
        enc(&provider),
        p.port,
        state,
        challenge
    );
    if let Some(h) = p.hooks.get() {
        (h.open_url)(&browser_url);
    }
    *p.sso.lock().unwrap() = Some(PendingSso { state, verifier, provider, redirect, browser_url, done: false, error: None });
    axum::Json(serde_json::json!({ "url": "/__looty/sso-wait" })).into_response()
}

/// Le navigateur revient avec le code : échange contre la session de l'application.
async fn sso_return(p: &Shared, fr: bool, code: Option<String>, state: Option<String>) -> Response {
    let page = |ok: bool| {
        let (title, text) = match (ok, fr) {
            (true, true) => ("Connecté à Looty", "Vous pouvez fermer cet onglet et revenir à l’application."),
            (true, false) => ("Signed in to Looty", "You can close this tab and return to the app."),
            (false, true) => ("Connexion impossible", "Ce lien a expiré ou a déjà servi. Relancez la connexion depuis l’application."),
            (false, false) => ("Sign-in failed", "This link has expired or was already used. Start again from the app."),
        };
        let body = include_str!("sso-done.html").replace("{{title}}", title).replace("{{text}}", text).replace("{{ok}}", if ok { "ok" } else { "err" });
        ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], body).into_response()
    };
    let (Some(code), Some(state)) = (code, state) else { return page(false) };
    let verifier = {
        let g = p.sso.lock().unwrap();
        match g.as_ref() {
            Some(s) if !s.done && s.state == state => s.verifier.clone(),
            _ => return page(false),
        }
    };
    let Some(server) = p.server() else { return page(false) };
    let res = p
        .client
        .post(format!("{}/api/auth/desktop/exchange", server.origin().ascii_serialization()))
        .json(&serde_json::json!({ "code": code, "verifier": verifier }))
        .send()
        .await;
    let ok = matches!(&res, Ok(r) if r.status().is_success());
    config::save_jar(&p.jar);
    if let Some(s) = p.sso.lock().unwrap().as_mut() {
        if ok {
            s.done = true;
        } else {
            s.error = Some("exchange_failed".into());
        }
    }
    if ok {
        if let Some(h) = p.hooks.get() {
            (h.signed_in)();
        }
    }
    page(ok)
}

fn setup_page() -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], include_str!("setup.html")).into_response()
}

/// Serveur injoignable : page d'erreur lisible pour la navigation, 502 pour l'API.
fn offline_response(path: &str) -> Response {
    if path.starts_with("/api/") || path.starts_with("/uploads/") || path.starts_with("/_nuxt/") {
        return (StatusCode::BAD_GATEWAY, "Looty server unreachable").into_response();
    }
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], include_str!("offline.html")).into_response()
}

async fn local_route(p: &Shared, rest: &str, req: Request) -> Response {
    match (req.method().clone(), rest) {
        (Method::GET, "setup") => setup_page(),
        (Method::POST, "setup") => {
            let body = match axum::body::to_bytes(req.into_body(), 64 * 1024).await {
                Ok(b) => b,
                Err(_) => return StatusCode::BAD_REQUEST.into_response(),
            };
            let url = serde_json::from_slice::<serde_json::Value>(&body).ok().and_then(|v| v["server"].as_str().map(str::to_string)).unwrap_or_default();
            match check_server(&p.client, &url).await {
                Ok(server) => {
                    config::clear_jar(&p.jar);
                    let mut cfg = p.config.write().unwrap();
                    cfg.server = Some(server.clone());
                    let _ = cfg.save();
                    axum::Json(serde_json::json!({ "server": server })).into_response()
                }
                Err(code) => (StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({ "error": code }))).into_response(),
            }
        }
        (Method::GET, "sso-wait") => html(include_str!("sso-wait.html")),
        (Method::GET, "quick-add") => html(include_str!("quick-add.html")),
        (Method::POST, "quick-add/close") | (Method::POST, "quick-add/done") => {
            let done = rest.ends_with("done");
            let body = axum::body::to_bytes(req.into_body(), 16 * 1024).await.unwrap_or_default();
            let title = serde_json::from_slice::<serde_json::Value>(&body).ok().and_then(|v| v["title"].as_str().map(str::to_string));
            if let Some(h) = p.hooks.get() {
                (h.quick_add_closed)(if done { Some(title.unwrap_or_default()) } else { None });
            }
            StatusCode::NO_CONTENT.into_response()
        }
        (Method::GET, "sso-status") => {
            let g = p.sso.lock().unwrap();
            match g.as_ref() {
                Some(s) => axum::Json(serde_json::json!({ "pending": !s.done, "done": s.done, "error": s.error, "redirect": s.redirect })).into_response(),
                None => axum::Json(serde_json::json!({ "pending": false, "done": false })).into_response(),
            }
        }
        (Method::POST, "sso-reopen") => {
            let url = p.sso.lock().unwrap().as_ref().map(|s| s.browser_url.clone());
            if let (Some(url), Some(h)) = (url, p.hooks.get()) {
                (h.open_url)(&url);
            }
            StatusCode::NO_CONTENT.into_response()
        }
        (Method::GET, "sso-cancel") => {
            *p.sso.lock().unwrap() = None;
            Redirect::to("/login").into_response()
        }
        // Repli : connexion dans la fenêtre (sans WebAuthn).
        (Method::GET, "sso-inapp") => {
            let pending = p.sso.lock().unwrap().take();
            let (Some(server), Some(s)) = (p.server(), pending) else { return Redirect::to("/login").into_response() };
            let url = format!("{}/api/auth/login/{}?redirect={}", server.origin().ascii_serialization(), s.provider, enc(&s.redirect));
            let target = async {
                let r = p.client.get(&url).send().await.ok()?;
                let v: serde_json::Value = r.json().await.ok()?;
                v["url"].as_str().map(str::to_string)
            }
            .await;
            config::save_jar(&p.jar);
            match target {
                Some(t) => {
                    p.start_sign_in();
                    Redirect::to(&t).into_response()
                }
                None => Redirect::to("/login?error=provider_error").into_response(),
            }
        }
        (Method::GET, "status") => {
            let server = p.server().map(|u| u.to_string());
            axum::Json(serde_json::json!({ "server": server, "version": env!("CARGO_PKG_VERSION") })).into_response()
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Vérifie qu'une adresse répond comme un serveur Looty ; renvoie l'origine normalisée.
async fn check_server(client: &reqwest::Client, raw: &str) -> Result<String, &'static str> {
    let raw = raw.trim().trim_end_matches('/');
    let with_scheme = if raw.contains("://") { raw.to_string() } else { format!("https://{raw}") };
    let url = Url::parse(&with_scheme).map_err(|_| "invalid_url")?;
    if !matches!(url.scheme(), "https" | "http") || url.host_str().is_none() {
        return Err("invalid_url");
    }
    let origin = url.origin().ascii_serialization();
    let res = client.get(format!("{origin}/api/health")).timeout(std::time::Duration::from_secs(10)).send().await.map_err(|_| "unreachable")?;
    let json: serde_json::Value = res.json().await.map_err(|_| "not_looty")?;
    if json["status"] != "ok" {
        return Err("not_looty");
    }
    Ok(origin)
}

/// En-têtes à ne pas relayer (connexion, ou gérés par le proxy lui-même).
fn hop(name: &str) -> bool {
    matches!(
        name,
        "connection" | "keep-alive" | "proxy-connection" | "transfer-encoding" | "te" | "trailer" | "upgrade" | "host" | "cookie" | "content-length"
    )
}

async fn proxy_http(p: &Shared, server: &Url, req: Request) -> Result<Response, reqwest::Error> {
    let (parts, body) = req.into_parts();
    let path_q = parts.uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
    let target = format!("{}{}", server.origin().ascii_serialization(), path_q);
    let origin = server.origin().ascii_serialization();
    let local = p.local_origin();

    let mut out = p.client.request(parts.method.clone(), &target);
    for (k, v) in parts.headers.iter() {
        let name = k.as_str();
        if hop(name) {
            continue;
        }
        // Origine et référent vus par le serveur : les siens.
        if name == "origin" {
            out = out.header(k, &origin);
        } else if name == "referer" {
            out = out.header(k, v.to_str().unwrap_or("").replacen(&local, &origin, 1));
        } else {
            out = out.header(k, v);
        }
    }
    if !matches!(parts.method, Method::GET | Method::HEAD) {
        let stream = body.into_data_stream().map_err(std::io::Error::other);
        out = out.body(reqwest::Body::wrap_stream(stream));
    }
    let res = out.send().await?;

    let status = res.status();
    let mut builder = Response::builder().status(status.as_u16());
    let saw_cookie = res.headers().contains_key(header::SET_COOKIE);
    for (k, v) in res.headers().iter() {
        let name = k.as_str();
        // Les cookies du serveur restent dans le magasin de Rust.
        if hop(name) || name == "set-cookie" {
            continue;
        }
        if name == "location" {
            let loc = v.to_str().unwrap_or("");
            if let Some(rest) = loc.strip_prefix(&origin) {
                builder = builder.header(k, if rest.is_empty() { "/" } else { rest });
                continue;
            }
            // Redirection vers le fournisseur SSO : début de la connexion.
            if parts.uri.path().starts_with("/api/auth/") && Url::parse(loc).is_ok() {
                p.start_sign_in();
            }
        }
        builder = builder.header(k, v);
    }
    if saw_cookie {
        config::save_jar(&p.jar);
    }
    // La page de connexion obtient l'adresse du fournisseur SSO en JSON ({ "url": … })
    // puis y navigue : début de la connexion.
    if status.is_success() && parts.uri.path().starts_with("/api/auth/login/") {
        let bytes = res.bytes().await?;
        if serde_json::from_slice::<serde_json::Value>(&bytes).ok().and_then(|v| v["url"].as_str().and_then(|u| Url::parse(u).ok())).is_some() {
            p.start_sign_in();
        }
        return Ok(builder.body(Body::from(bytes)).unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response()));
    }
    let stream = res.bytes_stream();
    Ok(builder.body(Body::from_stream(stream)).unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response()))
}

async fn proxy_ws(p: Shared, server: Url, req: Request) -> Response {
    let (mut parts, _body) = req.into_parts();
    let uri: Uri = parts.uri.clone();
    let protocols: Option<String> = parts.headers.get(header::SEC_WEBSOCKET_PROTOCOL).and_then(|v| v.to_str().ok()).map(str::to_string);
    let upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    let scheme = if server.scheme() == "https" { "wss" } else { "ws" };
    let host = server.host_str().unwrap_or("").to_string();
    let port = server.port().map(|p| format!(":{p}")).unwrap_or_default();
    let target = format!("{scheme}://{host}{port}{}", uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/"));
    let cookie_url = Url::parse(&format!("{}{}", server.origin().ascii_serialization(), uri.path())).unwrap_or(server.clone());
    let cookies = p.cookie_header(&cookie_url);

    let mut request = match tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(target.as_str()) {
        Ok(r) => r,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };
    let h = request.headers_mut();
    if !cookies.is_empty() {
        h.insert(header::COOKIE, HeaderValue::from_str(&cookies).unwrap_or(HeaderValue::from_static("")));
    }
    h.insert(header::ORIGIN, HeaderValue::from_str(&server.origin().ascii_serialization()).unwrap());
    if let Some(pr) = &protocols {
        if let Ok(v) = HeaderValue::from_str(pr) {
            h.insert(header::SEC_WEBSOCKET_PROTOCOL, v);
        }
    }
    let upstream = match tokio_tungstenite::connect_async(request).await {
        Ok((s, _)) => s,
        Err(e) => {
            tracing::warn!("websocket {}: {e}", uri.path());
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    let upgrade = match &protocols {
        Some(pr) => upgrade.protocols(pr.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>()),
        None => upgrade,
    };
    upgrade.on_upgrade(move |client| async move {
        use axum::extract::ws::Message as A;
        use tokio_tungstenite::tungstenite::Message as T;
        let (mut c_tx, mut c_rx) = client.split();
        let (mut u_tx, mut u_rx) = upstream.split();
        let up = async {
            while let Some(Ok(m)) = c_rx.next().await {
                let out = match m {
                    A::Text(t) => T::Text(t),
                    A::Binary(b) => T::Binary(b),
                    A::Ping(b) => T::Ping(b),
                    A::Pong(b) => T::Pong(b),
                    A::Close(_) => break,
                };
                if u_tx.send(out).await.is_err() {
                    break;
                }
            }
            let _ = u_tx.close().await;
        };
        let down = async {
            while let Some(Ok(m)) = u_rx.next().await {
                let out = match m {
                    T::Text(t) => A::Text(t),
                    T::Binary(b) => A::Binary(b),
                    T::Ping(b) => A::Ping(b),
                    T::Pong(b) => A::Pong(b),
                    T::Close(_) | T::Frame(_) => break,
                };
                if c_tx.send(out).await.is_err() {
                    break;
                }
            }
            let _ = c_tx.close().await;
        };
        tokio::select! { _ = up => {}, _ = down => {} }
    })
}
