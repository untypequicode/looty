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
//! - Réseau : http et https acceptés ; une adresse privée (réseau local, localhost) n'est
//!   jointe que si `JUPYTER_ALLOW_PRIVATE=true`.
//! - JupyterHub : le jeton d'API du Hub (page `/hub/token`) sert d'identifiant. L'adresse
//!   peut être celle du Hub (`/hub/login…`, `/hub/`) : le nom d'utilisateur est lu avec le
//!   jeton et l'adresse devient `/user/<nom>/`. Un serveur utilisateur arrêté est démarré
//!   via l'API du Hub avant la connexion.

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
pub(crate) fn parse_base(raw: &str) -> ApiResult<Url> {
    let mut url = Url::parse(raw.trim()).map_err(|_| ApiError::bad_request("jupyter_invalid_url"))?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(ApiError::bad_request("jupyter_invalid_url"));
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
    // `/user/<nom>/…` : un nom d'utilisateur JupyterHub peut s'appeler « lab » ou « hub ».
    let marker = |i: usize, s: &str| matches!(s, "lab" | "tree" | "notebooks" | "api" | "hub") && !(i > 0 && segs[i - 1] == "user");
    if let Some(i) = segs.iter().enumerate().position(|(i, s)| marker(i, s)) {
        segs.truncate(i);
    }
    let path = if segs.is_empty() { "/".to_string() } else { format!("/{}/", segs.join("/")) };
    url.set_path(&path);
    Ok(url)
}

/// Vérifie l'adresse puis la résolution DNS (pas d'adresse privée sauf autorisation).
async fn check_base(raw: &str) -> ApiResult<Url> {
    let private_ok = allow_private();
    let url = parse_base(raw)?;
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
    hub: Option<Hub>,
    /// Serveur atteint à travers le tunnel d'un PC (`tunnel.rs`) : `base` n'est alors qu'un point de départ pour les chemins.
    tunnel: Option<Route>,
}

/// Tunnel (jeton) et service déclaré par l'agent.
#[derive(Clone)]
struct Route {
    id: Uuid,
    service: String,
}

fn tunnel_error(e: crate::tunnel_relay::TunnelError) -> ApiError {
    use crate::tunnel_relay::TunnelError as E;
    ApiError::bad_request(match e {
        E::Offline | E::Closed => "jupyter_tunnel_offline",
        E::NoService => "jupyter_tunnel_service",
        E::TooManyStreams => "jupyter_tunnel_busy",
        E::Timeout => "jupyter_timeout",
        E::TooLarge => "jupyter_tunnel_too_large",
        E::Status(401 | 403) => "jupyter_auth_failed",
        E::Status(404) => "jupyter_kernel_gone",
        E::BadRequest | E::Status(_) | E::Agent(_) => "jupyter_unreachable",
    })
}

/// JupyterHub derrière le serveur : racine du Hub (préfixe de `hub/api`, `user/…`) et nom.
#[derive(Clone)]
struct Hub {
    root: Url,
    user: String,
}

fn valid_hub_user(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@'))
}

impl Server {
    /// Serveur d'une adresse normalisée ; reconnaît un JupyterHub (voir l'en-tête du module).
    async fn new(base: Url, token: Option<String>) -> Server {
        let mut s = Server { base, token, hub: None, tunnel: None };
        s.detect_hub().await;
        s
    }

    fn tunneled(route: Route, token: Option<String>) -> Server {
        Server { base: Url::parse("http://tunnel.invalid/").expect("static url"), token, hub: None, tunnel: Some(route) }
    }

    async fn detect_hub(&mut self) {
        if self.tunnel.is_some() {
            return;
        }
        let segs: Vec<String> = self.base.path_segments().map(|s| s.filter(|x| !x.is_empty()).map(str::to_string).collect()).unwrap_or_default();
        let n = segs.len();
        if n >= 2 && segs[n - 2] == "user" {
            let mut root = self.base.clone();
            root.set_path(&if n == 2 { "/".to_string() } else { format!("/{}/", segs[..n - 2].join("/")) });
            self.hub = Some(Hub { root, user: segs[n - 1].clone() });
            return;
        }
        // Adresse du Hub lui-même : le jeton dit à qui il appartient.
        if self.token.is_none() {
            return;
        }
        let Ok(v) = self.call(Method::GET, "hub/api/user", None, "jupyter_not_jupyter").await else { return };
        let Some(name) = v.get("name").and_then(Value::as_str).filter(|n| valid_hub_user(n)).map(str::to_string) else { return };
        let root = self.base.clone();
        let Ok(base) = root.join(&format!("user/{name}/")) else { return };
        self.base = base;
        self.hub = Some(Hub { root, user: name });
    }

    /// Démarre le serveur utilisateur JupyterHub s'il est arrêté (le Hub redirige alors vers
    /// sa page d'attente) et attend qu'il réponde. Sans Hub : ne fait rien.
    async fn ensure_up(&self) -> ApiResult<()> {
        let Some(hub) = &self.hub else { return Ok(()) };
        match self.call(Method::GET, "api", None, "jupyter_not_jupyter").await {
            Ok(_) => return Ok(()),
            Err(ApiError::BadRequest(c)) if matches!(c.as_str(), "jupyter_redirect" | "jupyter_unexpected_status" | "jupyter_not_jupyter") => {}
            Err(e) => return Err(e),
        }
        // 201/202 : démarrage ; 400 : déjà en cours (erreur ignorée).
        let _ = self.call_at(&hub.root, Method::POST, &format!("hub/api/users/{}/server", hub.user), None, "jupyter_not_jupyter").await;
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if self.call(Method::GET, "api", None, "jupyter_not_jupyter").await.is_ok() {
                return Ok(());
            }
        }
        Err(ApiError::bad_request("jupyter_hub_start_timeout"))
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>, gone: &'static str) -> ApiResult<Value> {
        self.call_at(&self.base, method, path, body, gone).await
    }

    /// Appel à l'API du serveur. Les erreurs sont traduites en codes explicites ; ni l'URL
    /// ni le jeton n'apparaissent dans les messages ou les journaux.
    async fn call_at(&self, base: &Url, method: Method, path: &str, body: Option<Value>, gone: &'static str) -> ApiResult<Value> {
        let url = base.join(path).map_err(|_| ApiError::bad_request("jupyter_invalid_url"))?;
        let (status, text) = if let Some(route) = &self.tunnel {
            let session = crate::tunnel_relay::connected(route.id).ok_or_else(|| ApiError::bad_request("jupyter_tunnel_offline"))?;
            let path = format!("{}{}", url.path(), url.query().map(|q| format!("?{q}")).unwrap_or_default());
            let mut headers = vec![("accept".to_string(), "application/json".to_string())];
            if let Some(t) = &self.token {
                headers.push(("authorization".into(), format!("token {t}")));
            }
            let body = body.map(|b| serde_json::to_vec(&b).unwrap_or_default());
            if body.is_some() {
                headers.push(("content-type".into(), "application/json".into()));
            }
            let r = crate::tunnel_relay::http(&session, &route.service, "jupyter", method.as_str(), &path, headers, body).await.map_err(tunnel_error)?;
            (r.status, String::from_utf8_lossy(&r.body).into_owned())
        } else {
            let mut req = http().request(method, url);
            if let Some(t) = &self.token {
                req = req.header(reqwest::header::AUTHORIZATION, format!("token {t}"));
            }
            if let Some(b) = body {
                req = req.json(&b);
            }
            let res = req.send().await.map_err(|e| ApiError::bad_request(if e.is_timeout() { "jupyter_timeout" } else { "jupyter_unreachable" }))?;
            let status = res.status().as_u16();
            let text = if (200..300).contains(&status) && status != 204 { res.text().await.map_err(|_| ApiError::bad_request("jupyter_unreachable"))? } else { String::new() };
            (status, text)
        };
        match status {
            200..=299 => {}
            401 | 403 => return Err(ApiError::bad_request("jupyter_auth_failed")),
            404 => return Err(ApiError::bad_request(gone)),
            300..=399 => return Err(ApiError::bad_request("jupyter_redirect")),
            _ => return Err(ApiError::bad_request("jupyter_unexpected_status")),
        }
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
    tunnel_id: Option<Uuid>,
    tunnel_service: Option<String>,
}

async fn load(state: &AppState, project: Uuid) -> ApiResult<Option<ConfigRow>> {
    Ok(sqlx::query_as("SELECT url, token_enc, kernel, updated_at, tunnel_id, tunnel_service FROM project_jupyter WHERE project_id = $1").bind(project).fetch_optional(&state.pool).await?)
}

fn decrypt_token(row: &ConfigRow) -> ApiResult<Option<String>> {
    match &row.token_enc {
        None => Ok(None),
        Some(enc) => crate::crypto::decrypt(enc).map(Some).ok_or_else(|| ApiError::bad_request("jupyter_token_unreadable")),
    }
}

async fn server_for(state: &AppState, project: Uuid) -> ApiResult<(Server, ConfigRow)> {
    let row = load(state, project).await?.ok_or_else(|| ApiError::bad_request("jupyter_not_configured"))?;
    let token = decrypt_token(&row)?;
    if let (Some(id), Some(service)) = (row.tunnel_id, row.tunnel_service.clone()) {
        // Le tunnel doit exister, ne pas être révoqué ni expiré, et la fonction doit être activée.
        let usable = crate::tunnel::enabled() && crate::tunnel::token_row(state, id).await?.is_some_and(|t| t.usable());
        if !usable {
            return Err(ApiError::bad_request("jupyter_tunnel_offline"));
        }
        return Ok((Server::tunneled(Route { id, service }, token), row));
    }
    let base = check_base(&row.url).await?;
    Ok((Server::new(base, token).await, row))
}

/// Tunnel du projet pour l'interface : propriétaire, état en direct (sans rien de secret).
async fn tunnel_json(state: &AppState, row: &ConfigRow, viewer: Uuid) -> Value {
    let Some(id) = row.tunnel_id else { return Value::Null };
    let t = crate::tunnel::token_row(state, id).await.ok().flatten();
    let mut live = crate::tunnel::live_json(id);
    live["id"] = json!(id);
    live["service"] = json!(row.tunnel_service);
    live["name"] = json!(t.as_ref().map(|t| t.name.clone()));
    live["mine"] = json!(t.as_ref().is_some_and(|t| t.user_id == viewer));
    live["usable"] = json!(t.as_ref().is_some_and(|t| t.usable()));
    live
}

fn config_json(row: Option<&ConfigRow>, can_edit: bool, tunnel: Value) -> Value {
    match row {
        None => json!({ "configured": false, "can_edit": can_edit, "allow_private": allow_private(), "tunnels_enabled": crate::tunnel::enabled() }),
        Some(r) => json!({
            "configured": true,
            "tunnel": tunnel,
            "tunnels_enabled": crate::tunnel::enabled(),
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
    let tunnel = match &row {
        Some(r) => tunnel_json(&state, r, user.id).await,
        None => Value::Null,
    };
    Ok(Json(config_json(row.as_ref(), can_write(&state, &user, id).await, tunnel)))
}

#[derive(Deserialize)]
struct ConfigBody {
    #[serde(default)]
    url: String,
    /// Tunnel d'un PC (jeton de tunnel de la personne) au lieu d'une adresse ; `service` : nom déclaré par l'agent.
    tunnel_id: Option<Uuid>,
    tunnel_service: Option<String>,
    /// Absent : jeton inchangé ; chaîne vide : retiré.
    token: Option<String>,
    kernel: Option<String>,
}

async fn put_config(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(b): Json<ConfigBody>) -> ApiResult<Json<Value>> {
    let (project, _) = project_for(&state, &user, id, Need::Write).await?;
    let kernel = b.kernel.as_deref().map(str::trim).filter(|k| !k.is_empty()).unwrap_or("python3").to_string();
    if !valid_kernel_name(&kernel) {
        return Err(ApiError::bad_request("jupyter_invalid_kernel"));
    }
    let previous = load(&state, id).await?;
    let (url, tunnel_id, tunnel_service) = match b.tunnel_id {
        Some(tid) => {
            if !crate::tunnel::enabled() {
                return Err(ApiError::bad_request("tunnels_disabled"));
            }
            let same = previous.as_ref().is_some_and(|p| p.tunnel_id == Some(tid));
            let t = crate::tunnel::token_row(&state, tid).await?.filter(|t| t.usable()).ok_or_else(|| ApiError::bad_request("jupyter_tunnel_unknown"))?;
            // Seul le propriétaire attache son tunnel ; un autre membre peut seulement garder celui déjà attaché.
            if t.user_id != user.id && !same {
                return Err(ApiError::Forbidden);
            }
            if t.workspace_id != project.workspace_id || t.project_id.is_some_and(|p| p != id) {
                return Err(ApiError::bad_request("tunnel_wrong_workspace"));
            }
            let service = b
                .tunnel_service
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| crate::tunnel_relay::connected(tid).and_then(|s| s.services().into_iter().find(|s| s.kind == "jupyter").map(|s| s.name)))
                .or_else(|| previous.as_ref().filter(|p| p.tunnel_id == Some(tid)).and_then(|p| p.tunnel_service.clone()))
                .unwrap_or_else(|| "jupyter".to_string());
            if !crate::tunnel_relay::valid_service_name(&service) {
                return Err(ApiError::bad_request("jupyter_tunnel_service"));
            }
            (String::new(), Some(tid), Some(service))
        }
        None => (check_base(&b.url).await?.to_string(), None, None),
    };
    let token_enc = match b.token.as_deref().map(str::trim) {
        Some("") => None,
        Some(t) if t.len() > 4096 => return Err(ApiError::bad_request("invalid_value")),
        Some(t) => Some(crate::crypto::encrypt(t)),
        // Jeton gardé seulement pour le même serveur : changer d'adresse sans le redonner l'efface.
        None => previous.as_ref().filter(|p| p.url == url && p.tunnel_id == tunnel_id).and_then(|p| p.token_enc.clone()),
    };
    sqlx::query(
        "INSERT INTO project_jupyter (project_id, url, token_enc, kernel, updated_by, updated_at, tunnel_id, tunnel_service) VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7)
         ON CONFLICT (project_id) DO UPDATE SET url = $2, token_enc = $3, kernel = $4, updated_by = $5, updated_at = NOW(), tunnel_id = $6, tunnel_service = $7",
    )
    .bind(id)
    .bind(&url)
    .bind(&token_enc)
    .bind(&kernel)
    .bind(user.id)
    .bind(tunnel_id)
    .bind(&tunnel_service)
    .execute(&state.pool)
    .await?;
    let row = load(&state, id).await?;
    let tunnel = match &row {
        Some(r) => tunnel_json(&state, r, user.id).await,
        None => Value::Null,
    };
    Ok(Json(config_json(row.as_ref(), true, tunnel)))
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
            Server::new(base, token).await
        }
        None => server_for(&state, id).await?.0,
    };
    server.ensure_up().await?;
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
    server.ensure_up().await?;
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

/// Côté serveur Jupyter d'un canal : connexion directe, ou flux d'un tunnel.
struct Upstream {
    tx: std::pin::Pin<Box<dyn futures::Sink<TMsg, Error = String> + Send>>,
    rx: std::pin::Pin<Box<dyn futures::Stream<Item = Result<TMsg, String>> + Send>>,
}

async fn connect_tunnel(route: &Route, token: Option<&str>, kid: &str) -> ApiResult<Upstream> {
    let session = crate::tunnel_relay::connected(route.id).ok_or_else(|| ApiError::bad_request("jupyter_tunnel_offline"))?;
    let path = format!("/api/kernels/{kid}/channels?session_id={}", Uuid::new_v4());
    let headers = token.map(|t| vec![("authorization".to_string(), format!("token {t}"))]).unwrap_or_default();
    let s = crate::tunnel_relay::ws_open(&session, &route.service, "jupyter", &path, headers).await.map_err(tunnel_error)?;
    let tx = futures::sink::unfold(s.tx, |tx, m: TMsg| async move {
        match m {
            TMsg::Text(t) => tx.send_message(true, t.as_bytes()).await,
            TMsg::Binary(b) => tx.send_message(false, &b).await,
            TMsg::Close(_) => tx.end().await,
            _ => Ok(()),
        }
        .map_err(|e| e.code())?;
        Ok::<_, String>(tx)
    });
    let rx = futures::stream::unfold(s.rx, |mut rx| async move {
        match rx.recv_message().await {
            None => None,
            Some(Err(e)) => Some((Err(e.code()), rx)),
            Some(Ok(m)) => Some((Ok(if m.text { TMsg::text(String::from_utf8_lossy(&m.data).into_owned()) } else { TMsg::Binary(m.data.into()) }), rx)),
        }
    });
    Ok(Upstream { tx: Box::pin(tx), rx: Box::pin(rx) })
}

async fn connect_upstream(server: &Server, kid: &str) -> ApiResult<Upstream> {
    if let Some(route) = &server.tunnel {
        return connect_tunnel(route, server.token.as_deref(), kid).await;
    }
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
        Ok(Ok((ws, _))) => {
            let (tx, rx) = ws.split();
            Ok(Upstream { tx: Box::pin(tx.sink_map_err(|e| e.to_string())), rx: Box::pin(rx.map(|r| r.map_err(|e| e.to_string()))) })
        }
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
    let (mut up_tx, mut up_rx) = (upstream.tx, upstream.rx);
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

    fn base(raw: &str) -> Result<String, String> {
        parse_base(raw).map(|u| u.to_string()).map_err(|e| match e {
            crate::error::ApiError::BadRequest(c) => c,
            _ => "other".into(),
        })
    }

    #[test]
    fn normalizes_server_addresses() {
        assert_eq!(base("https://jupyter.example.org").unwrap(), "https://jupyter.example.org/");
        assert_eq!(base("https://jupyter.example.org/lab").unwrap(), "https://jupyter.example.org/");
        assert_eq!(base("https://hub.example.org/user/alice/lab/tree/a.ipynb#x").unwrap(), "https://hub.example.org/user/alice/");
        assert_eq!(base("https://hub.example.org/user/alice").unwrap(), "https://hub.example.org/user/alice/");
        assert_eq!(base("http://10.0.0.5:8888/tree").unwrap(), "http://10.0.0.5:8888/");
        assert_eq!(base("https://jupyterhub.emi.u-bordeaux.fr/hub/login?next=%2Fhub%2F").unwrap(), "https://jupyterhub.emi.u-bordeaux.fr/");
        assert_eq!(base("https://jupyterhub.emi.u-bordeaux.fr/user/etfacca/lab").unwrap(), "https://jupyterhub.emi.u-bordeaux.fr/user/etfacca/");
        assert_eq!(base("https://h.example.org/user/lab/lab").unwrap(), "https://h.example.org/user/lab/");
    }

    #[test]
    fn rejects_unsafe_addresses() {
        assert_eq!(base("https://j.example.org/lab?token=abc").unwrap_err(), "jupyter_token_in_url");
        assert_eq!(base("https://u:p@j.example.org/").unwrap_err(), "jupyter_token_in_url");
        assert_eq!(base("http://j.example.org:8888/lab").unwrap(), "http://j.example.org:8888/");
        assert_eq!(base("ftp://j.example.org/").unwrap_err(), "jupyter_invalid_url");
        assert_eq!(base("not a url").unwrap_err(), "jupyter_invalid_url");
    }


    /// Un projet qui cible un tunnel : les appels REST et le canal du noyau passent par l'agent (faux ici).
    #[tokio::test]
    async fn a_tunneled_server_reaches_jupyter_through_the_agent() {
        use crate::tunnel_relay::{self as relay, Ctl, Out, ServiceDecl, Session};
        use futures::{SinkExt, StreamExt};
        let (tx, mut out) = tokio::sync::mpsc::channel(64);
        let id = uuid::Uuid::new_v4();
        let decl = vec![ServiceDecl { name: "pc".into(), kind: "jupyter".into() }];
        let session = Session::new(id, uuid::Uuid::new_v4(), "t".into(), chrono::Utc::now() + chrono::Duration::days(1), decl, tx);
        relay::register(session.clone());
        let agent = session.clone();
        tokio::spawn(async move {
            let say = |c: Ctl| serde_json::to_string(&c).unwrap();
            while let Some(o) = out.recv().await {
                let Out::Text(t) = o else { continue };
                let Ok(Ctl::Open { id, path, headers, proto, .. }) = serde_json::from_str::<Ctl>(&t) else { continue };
                assert!(headers.iter().any(|(k, v)| k == "authorization" && v == "token secret"));
                if proto == relay::Proto::Http {
                    assert!(path.starts_with("/api"));
                    agent.on_text(&say(Ctl::Head { id, status: 200, headers: vec![] })).await.unwrap();
                    let mut f = id.to_be_bytes().to_vec();
                    f.push(0);
                    f.extend_from_slice(br#"{"default":"python3","kernelspecs":{"python3":{"spec":{"display_name":"Python 3","language":"python"}}}}"#);
                    agent.on_binary(&f).unwrap();
                } else {
                    assert!(path.starts_with("/api/kernels/k1/channels?session_id="));
                    agent.on_text(&say(Ctl::Head { id, status: 101, headers: vec![] })).await.unwrap();
                    let mut f = id.to_be_bytes().to_vec();
                    f.extend_from_slice(&[relay::FLAG_TEXT]);
                    f.extend_from_slice(b"hello");
                    agent.on_binary(&f).unwrap();
                }
                if proto == relay::Proto::Http {
                    agent.on_text(&say(Ctl::End { id })).await.unwrap();
                }
            }
        });
        let server = super::Server::tunneled(super::Route { id, service: "pc".into() }, Some("secret".into()));
        let specs = server.kernelspecs().await.unwrap();
        assert_eq!(specs["kernels"][0]["name"], "python3");
        let mut up = super::connect_upstream(&server, "k1").await.unwrap();
        let m = up.rx.next().await.unwrap().unwrap();
        assert_eq!(m.into_text().unwrap().as_str(), "hello");
        up.tx.send(super::TMsg::text("x")).await.unwrap();
        // Hors ligne : erreur explicite, pas de repli silencieux.
        relay::disconnect(id, 4001, "revoked");
        let e = server.kernelspecs().await.err().unwrap();
        assert!(matches!(e, crate::error::ApiError::BadRequest(c) if c == "jupyter_tunnel_offline"));
    }
    #[test]
    fn kernel_names() {
        assert!(super::valid_kernel_name("python3"));
        assert!(super::valid_kernel_name("ir-4.3_x.y"));
        assert!(!super::valid_kernel_name("../x"));
        assert!(!super::valid_kernel_name(""));
    }
}
