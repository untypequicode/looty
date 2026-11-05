//! Serveur Jupyter externe pour les notebooks d'un projet.
//!
//! Par défaut un notebook s'exécute dans le navigateur (Pyodide). Un projet peut aussi
//! déclarer un serveur Jupyter (Jupyter Server, JupyterLab, JupyterHub…) : un notebook qui
//! le choisit (`metadata.looty.engine = "jupyter"` dans le .ipynb) exécute ses cellules
//! sur un noyau distant, les sorties restent partagées dans le document collaboratif.
//!
//! - Le jeton est chiffré en base et ne quitte jamais le serveur : l'interface parle au
//!   backend (cookie de session), qui relaie l'API REST et le WebSocket du noyau avec
//!   l'en-tête `Authorization: token …`. Pas de jeton dans les URL, les journaux ni le
//!   notebook ; aucune exigence CORS côté Jupyter (le navigateur ne le contacte pas).
//! - Droits : voir la configuration demande la lecture du projet ; la modifier ou exécuter
//!   du code à distance, la modification.
//! - Chaque personne a son propre noyau (comme avec Pyodide). Le backend ne touche qu'aux
//!   noyaux qu'il a démarrés, pour la personne qui les a démarrés ; un noyau sans connexion
//!   depuis `IDLE_SHUTDOWN` est arrêté.
//! - Réseau : https et adresse publique, sauf `JUPYTER_ALLOW_PRIVATE=true` (serveur du
//!   réseau local, http accepté).

use axum::{
    extract::{
        ws::{Message as AxMsg, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use reqwest::{Method, Url};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest, Message as TMsg};
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::projects::{project_for, Need};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/jupyter", get(get_config).put(put_config).delete(delete_config))
        .route("/{id}/jupyter/test", post(test_config))
        .route("/{id}/jupyter/kernels", post(start_kernel))
        .route("/{id}/jupyter/kernels/{kid}", get(kernel_status).delete(stop_kernel))
        .route("/{id}/jupyter/kernels/{kid}/interrupt", post(interrupt_kernel))
        .route("/{id}/jupyter/kernels/{kid}/restart", post(restart_kernel))
        .route("/{id}/jupyter/kernels/{kid}/channels", get(channels))
}

/// Noyau sans aucune connexion depuis ce délai : arrêté sur le serveur Jupyter.
const IDLE_SHUTDOWN: Duration = Duration::from_secs(20 * 60);
/// Noyaux ouverts en même temps par une personne (tous projets confondus).
const MAX_KERNELS_PER_USER: usize = 4;

// ---------------------------------------------------------------------------
// Adresse du serveur
// ---------------------------------------------------------------------------

fn allow_private() -> bool {
    std::env::var("JUPYTER_ALLOW_PRIVATE").as_deref() == Ok("true")
}

/// Normalise l'adresse donnée (celle de JupyterLab, d'un serveur JupyterHub…) en adresse
/// de base du serveur, terminée par `/`. Refuse un jeton dans l'URL : il a son propre champ.
pub(crate) fn parse_base(raw: &str, private_ok: bool) -> ApiResult<Url> {
    let mut url = Url::parse(raw.trim()).map_err(|_| ApiError::bad_request("jupyter_invalid_url"))?;
    match url.scheme() {
        "https" => {}
        "http" if private_ok => {}
        "http" => return Err(ApiError::bad_request("jupyter_https_required")),
        _ => return Err(ApiError::bad_request("jupyter_invalid_url")),
    }
    if url.query_pairs().any(|(k, _)| k == "token") || url.password().is_some() {
        return Err(ApiError::bad_request("jupyter_token_in_url"));
    }
    if !url.username().is_empty() || url.host_str().is_none() {
        return Err(ApiError::bad_request("jupyter_invalid_url"));
    }
    url.set_query(None);
    url.set_fragment(None);
    // Adresse copiée depuis l'interface : …/lab, …/tree, …/lab/tree/x.ipynb → la base.
    let mut segs: Vec<String> = url.path_segments().map(|s| s.filter(|x| !x.is_empty()).map(str::to_string).collect()).unwrap_or_default();
    if let Some(i) = segs.iter().position(|s| matches!(s.as_str(), "lab" | "tree" | "notebooks" | "api")) {
        segs.truncate(i);
    }
    let path = if segs.is_empty() { "/".to_string() } else { format!("/{}/", segs.join("/")) };
    url.set_path(&path);
    Ok(url)
}

/// Vérifie l'adresse puis la résolution DNS (pas d'adresse privée sauf autorisation).
async fn check_base(raw: &str) -> ApiResult<Url> {
    let private_ok = allow_private();
    let url = parse_base(raw, private_ok)?;
    let host = url.host_str().unwrap_or_default().trim_start_matches('[').trim_end_matches(']').to_string();
    let port = url.port_or_known_default().unwrap_or(443);
    let ips: Vec<IpAddr> = match host.parse::<IpAddr>() {
        Ok(ip) => vec![ip],
        Err(_) => tokio::net::lookup_host((host.as_str(), port)).await.map_err(|_| ApiError::bad_request("jupyter_unreachable"))?.map(|a| a.ip()).collect(),
    };
    if ips.is_empty() {
        return Err(ApiError::bad_request("jupyter_unreachable"));
    }
    if !private_ok && ips.iter().any(|ip| crate::caldav::is_private(*ip)) {
        return Err(ApiError::bad_request("jupyter_private_address"));
    }
    Ok(url)
}

fn valid_kernel_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

// ---------------------------------------------------------------------------
// Client HTTP
// ---------------------------------------------------------------------------

fn http() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(8))
            .user_agent("Looty-Jupyter/1.0")
            .build()
            .expect("http client")
    })
}

#[derive(Clone)]
struct Server {
    base: Url,
    token: Option<String>,
}

impl Server {
    /// Appel à l'API du serveur. Les erreurs sont traduites en codes explicites ; ni l'URL
    /// ni le jeton n'apparaissent dans les messages ou les journaux.
    async fn call(&self, method: Method, path: &str, body: Option<Value>, gone: &'static str) -> ApiResult<Value> {
        let url = self.base.join(path).map_err(|_| ApiError::bad_request("jupyter_invalid_url"))?;
        let mut req = http().request(method, url);
        if let Some(t) = &self.token {
            req = req.header(reqwest::header::AUTHORIZATION, format!("token {t}"));
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let res = req.send().await.map_err(|e| ApiError::bad_request(if e.is_timeout() { "jupyter_timeout" } else { "jupyter_unreachable" }))?;
        let status = res.status();
        match status.as_u16() {
            200..=299 => {}
            401 | 403 => return Err(ApiError::bad_request("jupyter_auth_failed")),
            404 => return Err(ApiError::bad_request(gone)),
            300..=399 => return Err(ApiError::bad_request("jupyter_redirect")),
            _ => return Err(ApiError::bad_request("jupyter_unexpected_status")),
        }
        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        let text = res.text().await.map_err(|_| ApiError::bad_request("jupyter_unreachable"))?;
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|_| ApiError::bad_request("jupyter_not_jupyter"))
    }

    /// Noyaux disponibles : `{ default, kernels: [{ name, display_name, language }] }`.
    async fn kernelspecs(&self) -> ApiResult<Value> {
        let v = self.call(Method::GET, "api/kernelspecs", None, "jupyter_not_jupyter").await?;
        let specs = v.get("kernelspecs").and_then(Value::as_object).ok_or_else(|| ApiError::bad_request("jupyter_not_jupyter"))?;
        let mut kernels: Vec<Value> = specs
            .iter()
            .map(|(name, s)| {
                let spec = s.get("spec").cloned().unwrap_or(Value::Null);
                json!({
                    "name": name,
                    "display_name": spec.get("display_name").and_then(Value::as_str).unwrap_or(name),
                    "language": spec.get("language").and_then(Value::as_str).unwrap_or(""),
                })
            })
            .collect();
        kernels.sort_by(|a, b| a["display_name"].as_str().cmp(&b["display_name"].as_str()));
        let version = self.call(Method::GET, "api", None, "jupyter_not_jupyter").await.ok().and_then(|v| v.get("version").and_then(Value::as_str).map(str::to_string));
        Ok(json!({ "default": v.get("default"), "kernels": kernels, "version": version }))
    }
}

// ---------------------------------------------------------------------------
// Configuration du projet
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ConfigRow {
    url: String,
    token_enc: Option<String>,
    kernel: String,
    updated_at: DateTime<Utc>,
}

async fn load(state: &AppState, project: Uuid) -> ApiResult<Option<ConfigRow>> {
    Ok(sqlx::query_as("SELECT url, token_enc, kernel, updated_at FROM project_jupyter WHERE project_id = $1").bind(project).fetch_optional(&state.pool).await?)
}

fn decrypt_token(row: &ConfigRow) -> ApiResult<Option<String>> {
    match &row.token_enc {
        None => Ok(None),
        Some(enc) => crate::crypto::decrypt(enc).map(Some).ok_or_else(|| ApiError::bad_request("jupyter_token_unreadable")),
    }
}

async fn server_for(state: &AppState, project: Uuid) -> ApiResult<(Server, ConfigRow)> {
    let row = load(state, project).await?.ok_or_else(|| ApiError::bad_request("jupyter_not_configured"))?;
    let base = check_base(&row.url).await?;
    let token = decrypt_token(&row)?;
    Ok((Server { base, token }, row))
}

fn config_json(row: Option<&ConfigRow>, can_edit: bool) -> Value {
    match row {
        None => json!({ "configured": false, "can_edit": can_edit, "allow_private": allow_private() }),
        Some(r) => json!({
            "configured": true,
            "url": r.url,
            "kernel": r.kernel,
            "has_token": r.token_enc.is_some(),
            "updated_at": r.updated_at,
            "can_edit": can_edit,
            "allow_private": allow_private(),
        }),
    }
}

async fn can_write(state: &AppState, user: &AuthUser, id: Uuid) -> bool {
    project_for(state, user, id, Need::Write).await.is_ok()
}

async fn get_config(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    project_for(&state, &user, id, Need::Read).await?;
    let row = load(&state, id).await?;
    Ok(Json(config_json(row.as_ref(), can_write(&state, &user, id).await)))
}

#[derive(Deserialize)]
struct ConfigBody {
    url: String,
    /// Absent : jeton inchangé ; chaîne vide : retiré.
    token: Option<String>,
    kernel: Option<String>,
}

async fn put_config(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(b): Json<ConfigBody>) -> ApiResult<Json<Value>> {
    project_for(&state, &user, id, Need::Write).await?;
    let base = check_base(&b.url).await?;
    let kernel = b.kernel.as_deref().map(str::trim).filter(|k| !k.is_empty()).unwrap_or("python3").to_string();
    if !valid_kernel_name(&kernel) {
        return Err(ApiError::bad_request("jupyter_invalid_kernel"));
    }
    let previous = load(&state, id).await?;
    let token_enc = match b.token.as_deref().map(str::trim) {
        Some("") => None,
        Some(t) if t.len() > 4096 => return Err(ApiError::bad_request("invalid_value")),
        Some(t) => Some(crate::crypto::encrypt(t)),
        // Jeton gardé seulement pour le même serveur : changer d'adresse sans le redonner l'efface.
        None => previous.as_ref().filter(|p| p.url == base.as_str()).and_then(|p| p.token_enc.clone()),
    };
    sqlx::query(
        "INSERT INTO project_jupyter (project_id, url, token_enc, kernel, updated_by, updated_at) VALUES ($1, $2, $3, $4, $5, NOW())
         ON CONFLICT (project_id) DO UPDATE SET url = $2, token_enc = $3, kernel = $4, updated_by = $5, updated_at = NOW()",
    )
    .bind(id)
    .bind(base.as_str())
    .bind(&token_enc)
    .bind(&kernel)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    let row = load(&state, id).await?;
    Ok(Json(config_json(row.as_ref(), true)))
}

async fn delete_config(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    project_for(&state, &user, id, Need::Write).await?;
    sqlx::query("DELETE FROM project_jupyter WHERE project_id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, Default)]
struct TestBody {
    url: Option<String>,
    token: Option<String>,
}

/// Teste une adresse (celle du formulaire, sinon celle enregistrée) : version et noyaux.
async fn test_config(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, body: Option<Json<TestBody>>) -> ApiResult<Json<Value>> {
    project_for(&state, &user, id, Need::Write).await?;
    let b = body.map(|Json(b)| b).unwrap_or_default();
    let stored = load(&state, id).await?;
    let server = match b.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        Some(raw) => {
            let base = check_base(raw).await?;
            let token = match b.token.as_deref().map(str::trim) {
                Some("") => None,
                Some(t) => Some(t.to_string()),
                None => match stored.as_ref().filter(|s| s.url == base.as_str()) {
                    Some(s) => decrypt_token(s)?,
                    None => None,
                },
            };
            Server { base, token }
        }
        None => server_for(&state, id).await?.0,
    };
    Ok(Json(server.kernelspecs().await?))
}

// ---------------------------------------------------------------------------
// Noyaux
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Kernel {
    project: Uuid,
    user: Uuid,
    server: Server,
    connections: usize,
    last_seen: Instant,
}

static KERNELS: LazyLock<Mutex<HashMap<String, Kernel>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn kernels() -> std::sync::MutexGuard<'static, HashMap<String, Kernel>> {
    KERNELS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Noyau démarré par cette personne dans ce projet, sinon introuvable.
fn owned(project: Uuid, user: Uuid, kid: &str) -> ApiResult<Kernel> {
    if Uuid::parse_str(kid).is_err() {
        return Err(ApiError::NotFound);
    }
    let mut map = kernels();
    let k = map.get_mut(kid).filter(|k| k.project == project && k.user == user).ok_or(ApiError::NotFound)?;
    k.last_seen = Instant::now();
    Ok(k.clone())
}

fn kernel_json(kid: &str, v: &Value) -> Value {
    json!({
        "id": kid,
        "name": v.get("name"),
        "execution_state": v.get("execution_state").and_then(Value::as_str).unwrap_or("unknown"),
    })
}

#[derive(Deserialize, Default)]
struct StartBody {
    kernel: Option<String>,
}

async fn start_kernel(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, body: Option<Json<StartBody>>) -> ApiResult<Json<Value>> {
    project_for(&state, &user, id, Need::Write).await?;
    let (server, row) = server_for(&state, id).await?;
    let name = body.and_then(|Json(b)| b.kernel).map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).unwrap_or(row.kernel);
    if !valid_kernel_name(&name) {
        return Err(ApiError::bad_request("jupyter_invalid_kernel"));
    }
    if kernels().values().filter(|k| k.user == user.id).count() >= MAX_KERNELS_PER_USER {
        return Err(ApiError::bad_request("jupyter_too_many_kernels"));
    }
    let v = server.call(Method::POST, "api/kernels", Some(json!({ "name": name })), "jupyter_kernel_unknown").await?;
    let kid = v.get("id").and_then(Value::as_str).filter(|k| Uuid::parse_str(k).is_ok()).ok_or_else(|| ApiError::bad_request("jupyter_not_jupyter"))?.to_string();
    kernels().insert(kid.clone(), Kernel { project: id, user: user.id, server, connections: 0, last_seen: Instant::now() });
    Ok(Json(kernel_json(&kid, &v)))
}

/// Retire un noyau disparu du serveur (redémarrage de Jupyter, arrêt par quelqu'un d'autre).
fn forget_if_gone<T>(kid: &str, r: ApiResult<T>) -> ApiResult<T> {
    if matches!(&r, Err(ApiError::BadRequest(code)) if code == "jupyter_kernel_gone") {
        kernels().remove(kid);
    }
    r
}

async fn kernel_status(State(state): State<AppState>, user: AuthUser, Path((id, kid)): Path<(Uuid, String)>) -> ApiResult<Json<Value>> {
    project_for(&state, &user, id, Need::Write).await?;
    let k = owned(id, user.id, &kid)?;
    let v = forget_if_gone(&kid, k.server.call(Method::GET, &format!("api/kernels/{kid}"), None, "jupyter_kernel_gone").await)?;
    Ok(Json(kernel_json(&kid, &v)))
}

async fn interrupt_kernel(State(state): State<AppState>, user: AuthUser, Path((id, kid)): Path<(Uuid, String)>) -> ApiResult<StatusCode> {
    project_for(&state, &user, id, Need::Write).await?;
    let k = owned(id, user.id, &kid)?;
    forget_if_gone(&kid, k.server.call(Method::POST, &format!("api/kernels/{kid}/interrupt"), None, "jupyter_kernel_gone").await)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restart_kernel(State(state): State<AppState>, user: AuthUser, Path((id, kid)): Path<(Uuid, String)>) -> ApiResult<Json<Value>> {
    project_for(&state, &user, id, Need::Write).await?;
    let k = owned(id, user.id, &kid)?;
    let v = forget_if_gone(&kid, k.server.call(Method::POST, &format!("api/kernels/{kid}/restart"), None, "jupyter_kernel_gone").await)?;
    Ok(Json(kernel_json(&kid, &v)))
}

async fn stop_kernel(State(state): State<AppState>, user: AuthUser, Path((id, kid)): Path<(Uuid, String)>) -> ApiResult<StatusCode> {
    // Arrêter son propre noyau reste possible même si l'on a perdu le droit de modifier.
    project_for(&state, &user, id, Need::Read).await?;
    let k = owned(id, user.id, &kid)?;
    kernels().remove(&kid);
    match k.server.call(Method::DELETE, &format!("api/kernels/{kid}"), None, "jupyter_kernel_gone").await {
        Ok(_) | Err(ApiError::BadRequest(_)) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Canaux du noyau (WebSocket relayé)
// ---------------------------------------------------------------------------

type Upstream = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect_upstream(server: &Server, kid: &str) -> ApiResult<Upstream> {
    // Résolution revérifiée à chaque connexion.
    let base = check_base(server.base.as_str()).await?;
    let mut url = base.join(&format!("api/kernels/{kid}/channels")).map_err(|_| ApiError::bad_request("jupyter_invalid_url"))?;
    let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
    url.set_scheme(scheme).map_err(|_| ApiError::bad_request("jupyter_invalid_url"))?;
    url.query_pairs_mut().append_pair("session_id", &Uuid::new_v4().to_string());
    let mut req = url.as_str().into_client_request().map_err(|_| ApiError::bad_request("jupyter_invalid_url"))?;
    if let Some(t) = &server.token {
        let v = format!("token {t}").parse().map_err(|_| ApiError::bad_request("jupyter_auth_failed"))?;
        req.headers_mut().insert("authorization", v);
    }
    let connect = tokio_tungstenite::connect_async(req);
    match tokio::time::timeout(Duration::from_secs(15), connect).await {
        Err(_) => Err(ApiError::bad_request("jupyter_timeout")),
        Ok(Ok((ws, _))) => Ok(ws),
        Ok(Err(tungstenite::Error::Http(res))) => Err(ApiError::bad_request(match res.status().as_u16() {
            401 | 403 => "jupyter_auth_failed",
            404 => "jupyter_kernel_gone",
            _ => "jupyter_unexpected_status",
        })),
        Ok(Err(_)) => Err(ApiError::bad_request("jupyter_unreachable")),
    }
}

async fn channels(ws: WebSocketUpgrade, State(state): State<AppState>, user: AuthUser, Path((id, kid)): Path<(Uuid, String)>) -> ApiResult<Response> {
    project_for(&state, &user, id, Need::Write).await?;
    let k = owned(id, user.id, &kid)?;
    // Connexion au noyau AVANT l'upgrade : une erreur reste une réponse HTTP explicite.
    let upstream = forget_if_gone(&kid, connect_upstream(&k.server, &kid).await)?;
    if let Some(e) = kernels().get_mut(&kid) {
        e.connections += 1;
    }
    Ok(ws.on_upgrade(move |socket| relay(socket, upstream, kid)).into_response())
}

async fn relay(socket: WebSocket, upstream: Upstream, kid: String) {
    let (mut up_tx, mut up_rx) = upstream.split();
    let (mut cl_tx, mut cl_rx) = socket.split();
    let to_kernel = async {
        while let Some(Ok(m)) = cl_rx.next().await {
            let m = match m {
                AxMsg::Text(t) => TMsg::text(t.as_str().to_owned()),
                // Sans sous-protocole v1, Jupyter n'attend que du JSON en texte ; un relais
                // (celui de l'interface) peut transformer les trames texte en binaire.
                AxMsg::Binary(b) => match std::str::from_utf8(&b) {
                    Ok(s) => TMsg::text(s.to_owned()),
                    Err(_) => TMsg::Binary(b),
                },
                AxMsg::Close(_) => break,
                _ => continue,
            };
            if up_tx.send(m).await.is_err() {
                break;
            }
        }
        let _ = up_tx.close().await;
    };
    let to_client = async {
        while let Some(Ok(m)) = up_rx.next().await {
            let m = match m {
                TMsg::Text(t) => AxMsg::Text(t.as_str().to_owned().into()),
                TMsg::Binary(b) => AxMsg::Binary(b),
                TMsg::Close(_) => break,
                _ => continue,
            };
            if cl_tx.send(m).await.is_err() {
                break;
            }
        }
        let _ = cl_tx.close().await;
    };
    tokio::select! {
        _ = to_kernel => {}
        _ = to_client => {}
    }
    if let Some(e) = kernels().get_mut(&kid) {
        e.connections = e.connections.saturating_sub(1);
        e.last_seen = Instant::now();
    }
}

/// Arrête les noyaux abandonnés (onglet fermé sans arrêt, connexion perdue).
pub fn start_reaper() {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let idle: Vec<(String, Server)> = {
                let mut map = kernels();
                let ids: Vec<String> = map.iter().filter(|(_, k)| k.connections == 0 && k.last_seen.elapsed() > IDLE_SHUTDOWN).map(|(id, _)| id.clone()).collect();
                ids.into_iter().filter_map(|id| map.remove(&id).map(|k| (id, k.server))).collect()
            };
            for (kid, server) in idle {
                let _ = server.call(Method::DELETE, &format!("api/kernels/{kid}"), None, "jupyter_kernel_gone").await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::parse_base;

    fn base(raw: &str, private_ok: bool) -> Result<String, String> {
        parse_base(raw, private_ok).map(|u| u.to_string()).map_err(|e| match e {
            crate::error::ApiError::BadRequest(c) => c,
            _ => "other".into(),
        })
    }

    #[test]
    fn normalizes_server_addresses() {
        assert_eq!(base("https://jupyter.example.org", false).unwrap(), "https://jupyter.example.org/");
        assert_eq!(base("https://jupyter.example.org/lab", false).unwrap(), "https://jupyter.example.org/");
        assert_eq!(base("https://hub.example.org/user/alice/lab/tree/a.ipynb#x", false).unwrap(), "https://hub.example.org/user/alice/");
        assert_eq!(base("https://hub.example.org/user/alice", false).unwrap(), "https://hub.example.org/user/alice/");
        assert_eq!(base("http://10.0.0.5:8888/tree", true).unwrap(), "http://10.0.0.5:8888/");
    }

    #[test]
    fn rejects_unsafe_addresses() {
        assert_eq!(base("https://j.example.org/lab?token=abc", false).unwrap_err(), "jupyter_token_in_url");
        assert_eq!(base("https://u:p@j.example.org/", false).unwrap_err(), "jupyter_token_in_url");
        assert_eq!(base("http://j.example.org/", false).unwrap_err(), "jupyter_https_required");
        assert_eq!(base("ftp://j.example.org/", true).unwrap_err(), "jupyter_invalid_url");
        assert_eq!(base("not a url", false).unwrap_err(), "jupyter_invalid_url");
    }

    #[test]
    fn kernel_names() {
        assert!(super::valid_kernel_name("python3"));
        assert!(super::valid_kernel_name("ir-4.3_x.y"));
        assert!(!super::valid_kernel_name("../x"));
        assert!(!super::valid_kernel_name(""));
    }
}
