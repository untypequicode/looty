//! Compilation LaTeX par le TeX Live d'un PC connecté par tunnel (issue #275, ADR 0020).
//!
//! Un projet LaTeX peut être rattaché à un service tunnelé de type `latex` (service `looty-latex-service`
//! de `bridge/`). La compilation envoie les fichiers du projet en zip à `POST /compile?async=1`, interroge
//! `GET /jobs/{id}?wait=20` jusqu'au résultat (chaque requête reste courte : le tunnel attend 30 s au plus un
//! premier octet) et lit le PDF et le journal dans le zip rendu. Annuler (client parti) supprime le travail
//! côté PC (`DELETE /jobs/{id}`). Les règles de rattachement sont celles de Jupyter : seule la personne
//! propriétaire du jeton rattache son tunnel ; rien n'est jamais redirigé en silence vers le serveur.
//!
//! Codes d'erreur : `latex_tunnel_offline`, `latex_tunnel_service`, `latex_tunnel_busy`,
//! `latex_tunnel_too_large`, `latex_tunnel_unreachable`, et ceux du service (`latex_timeout`, `latex_busy`,
//! `latex_no_main`, `latex_unavailable`, `latex_too_large`).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use dashmap::DashMap;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::projects::{all_files, file_bytes, project_for, Need};
use crate::state::AppState;
use crate::tunnel_relay::{self as relay, Headers, Session, TunnelError, MAX_REQUEST_BODY};

pub const KIND: &str = "latex";
/// Temps au plus d'une compilation côté instance (le service borne aussi la sienne à 300 s).
const TOTAL: Duration = Duration::from_secs(330);
/// Compilations en même temps par tunnel.
const PER_TUNNEL: usize = 2;

pub fn routes() -> Router<AppState> {
    Router::new().route("/{id}/latex/tunnel", get(get_config).put(put_config).delete(delete_config))
}

/// Tunnel et service d'un projet.
#[derive(Clone, Debug)]
pub struct Route {
    pub tunnel_id: Uuid,
    pub service: String,
}

#[derive(sqlx::FromRow)]
struct Row {
    tunnel_id: Uuid,
    tunnel_service: String,
}

async fn load(state: &AppState, project: Uuid) -> ApiResult<Option<Row>> {
    Ok(sqlx::query_as("SELECT tunnel_id, tunnel_service FROM project_latex_tunnel WHERE project_id = $1").bind(project).fetch_optional(&state.pool).await?)
}

/// Tunnel du projet s'il y en a un. Un tunnel révoqué, expiré ou désactivé donne `latex_tunnel_offline`.
pub async fn route(state: &AppState, project: Uuid) -> ApiResult<Option<Route>> {
    let Some(row) = load(state, project).await? else { return Ok(None) };
    let usable = crate::tunnel::policy(&state.pool).await?.enabled && crate::tunnel::token_row(state, row.tunnel_id).await?.is_some_and(|t| t.usable());
    if !usable {
        return Err(ApiError::bad_request("latex_tunnel_offline"));
    }
    Ok(Some(Route { tunnel_id: row.tunnel_id, service: row.tunnel_service }))
}

async fn config_json(state: &AppState, row: Option<&Row>, viewer: Uuid) -> Value {
    let enabled = crate::tunnel::policy(&state.pool).await.map(|p| p.enabled).unwrap_or(false);
    let Some(r) = row else { return json!({ "attached": false, "tunnels_enabled": enabled }) };
    let t = crate::tunnel::token_row(state, r.tunnel_id).await.ok().flatten();
    let mut live = crate::tunnel::live_json(r.tunnel_id);
    live["id"] = json!(r.tunnel_id);
    live["service"] = json!(r.tunnel_service);
    live["name"] = json!(t.as_ref().map(|t| t.name.clone()));
    live["usable"] = json!(t.as_ref().is_some_and(|t| t.usable()));
    live["mine"] = json!(t.as_ref().is_some_and(|t| t.user_id == viewer));
    json!({ "attached": true, "service": r.tunnel_service, "tunnel": live, "tunnels_enabled": enabled })
}

async fn get_config(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    project_for(&state, &user, id, Need::Read).await?;
    let row = load(&state, id).await?;
    Ok(Json(config_json(&state, row.as_ref(), user.id).await))
}

#[derive(Deserialize)]
struct PutBody {
    tunnel_id: Uuid,
    /// Nom du service `latex` déclaré par l'agent (par défaut le premier).
    tunnel_service: Option<String>,
}

async fn put_config(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(b): Json<PutBody>) -> ApiResult<Json<Value>> {
    let (project, _) = project_for(&state, &user, id, Need::Write).await?;
    if state.demo {
        return Err(ApiError::DemoUnavailable);
    }
    if project.kind != "latex" {
        return Err(ApiError::bad_request("invalid_value"));
    }
    if !crate::tunnel::policy(&state.pool).await?.enabled {
        return Err(ApiError::bad_request("tunnels_disabled"));
    }
    let previous = load(&state, id).await?;
    let same = previous.as_ref().is_some_and(|p| p.tunnel_id == b.tunnel_id);
    let t = crate::tunnel::token_row(&state, b.tunnel_id).await?.filter(|t| t.usable()).ok_or_else(|| ApiError::bad_request("latex_tunnel_unknown"))?;
    // Seul le propriétaire attache son tunnel ; un autre membre peut seulement garder celui déjà attaché.
    if t.user_id != user.id && !same {
        return Err(ApiError::Forbidden);
    }
    if t.workspace_id != project.workspace_id || t.project_id.is_some_and(|p| p != id) {
        return Err(ApiError::bad_request("tunnel_wrong_workspace"));
    }
    let live = relay::connected(b.tunnel_id).map(|s| s.services());
    let service = b
        .tunnel_service
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| live.as_ref().and_then(|l| l.iter().find(|s| s.kind == KIND).map(|s| s.name.clone())))
        .or_else(|| previous.as_ref().filter(|p| p.tunnel_id == b.tunnel_id).map(|p| p.tunnel_service.clone()))
        .unwrap_or_else(|| "latex".to_string());
    if !relay::valid_service_name(&service) {
        return Err(ApiError::bad_request("latex_tunnel_service"));
    }
    // Quand l'agent est connecté, le service doit être déclaré, de type latex.
    if let Some(l) = &live {
        if !l.iter().any(|s| s.name == service && s.kind == KIND) {
            return Err(ApiError::bad_request("latex_tunnel_service"));
        }
    }
    sqlx::query(
        "INSERT INTO project_latex_tunnel (project_id, tunnel_id, tunnel_service, updated_by, updated_at) VALUES ($1, $2, $3, $4, NOW())
         ON CONFLICT (project_id) DO UPDATE SET tunnel_id = $2, tunnel_service = $3, updated_by = $4, updated_at = NOW()",
    )
    .bind(id)
    .bind(b.tunnel_id)
    .bind(&service)
    .bind(user.id)
    .execute(&state.pool)
    .await?;
    let row = load(&state, id).await?;
    Ok(Json(config_json(&state, row.as_ref(), user.id).await))
}

async fn delete_config(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    project_for(&state, &user, id, Need::Write).await?;
    sqlx::query("DELETE FROM project_latex_tunnel WHERE project_id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Compilation
// ---------------------------------------------------------------------------

fn tunnel_error(e: TunnelError) -> ApiError {
    ApiError::bad_request(match e {
        TunnelError::Offline | TunnelError::Closed => "latex_tunnel_offline",
        TunnelError::NoService => "latex_tunnel_service",
        TunnelError::TooManyStreams => "latex_tunnel_busy",
        TunnelError::KindNotAllowed => "tunnel_kind_not_allowed",
        TunnelError::OwnerOnly => "tunnel_owner_only",
        TunnelError::Timeout => "latex_timeout",
        TunnelError::TooLarge => "latex_tunnel_too_large",
        TunnelError::BadRequest | TunnelError::Status(_) | TunnelError::Agent(_) => "latex_tunnel_unreachable",
    })
}

/// Code d'erreur du service (`{"error": "latex_..."}`), seulement parmi ceux qu'il documente.
fn service_error(body: &[u8]) -> ApiError {
    let code = serde_json::from_slice::<Value>(body).ok().and_then(|v| v["error"].as_str().map(str::to_string));
    ApiError::bad_request(match code.as_deref() {
        Some("latex_timeout") => "latex_timeout",
        Some("latex_busy") => "latex_busy",
        Some("latex_no_main") => "latex_no_main",
        Some("latex_engine_missing") => "latex_unavailable",
        Some("latex_too_large") => "latex_tunnel_too_large",
        Some("latex_bad_request") => "invalid_value",
        _ => "latex_tunnel_unreachable",
    })
}

static SLOTS: LazyLock<DashMap<Uuid, Arc<Semaphore>>> = LazyLock::new(DashMap::new);

fn zip_project(files: Vec<(String, Vec<u8>)>) -> ApiResult<Vec<u8>> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    for (path, bytes) in files {
        w.start_file(path, opts).map_err(|e| ApiError::Internal(e.to_string()))?;
        w.write_all(&bytes).map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    Ok(w.finish().map_err(|e| ApiError::Internal(e.to_string()))?.into_inner())
}

/// Lit `output.pdf` (absent si la compilation n'en a pas produit) et `compile.log`.
fn unzip_result(bytes: &[u8]) -> ApiResult<(Option<Vec<u8>>, String)> {
    let bad = || ApiError::bad_request("latex_tunnel_unreachable");
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| bad())?;
    let mut pdf = None;
    if let Ok(mut f) = z.by_name("output.pdf") {
        let mut v = Vec::new();
        std::io::Read::by_ref(&mut f).take(64 * 1024 * 1024).read_to_end(&mut v).map_err(|_| bad())?;
        pdf = Some(v);
    }
    let mut log = String::new();
    if let Ok(mut f) = z.by_name("compile.log") {
        let mut v = Vec::new();
        std::io::Read::by_ref(&mut f).take(2 * 1024 * 1024).read_to_end(&mut v).map_err(|_| bad())?;
        log = String::from_utf8_lossy(&v).into_owned();
    }
    Ok((pdf, log))
}

/// Supprime le travail côté PC quand l'appelant s'en va avant la fin (client déconnecté, délai).
struct Cancel {
    session: Arc<Session>,
    service: String,
    id: Option<String>,
}

impl Cancel {
    async fn done(&mut self) {
        if let Some(id) = self.id.take() {
            let _ = relay::http(&self.session, &self.service, KIND, "DELETE", &format!("/jobs/{id}"), vec![], None).await;
        }
    }
}

impl Drop for Cancel {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            let (session, service) = (self.session.clone(), self.service.clone());
            if let Ok(h) = tokio::runtime::Handle::try_current() {
                h.spawn(async move {
                    let _ = relay::http(&session, &service, KIND, "DELETE", &format!("/jobs/{id}"), vec![], None).await;
                });
            }
        }
    }
}

/// Envoie le projet (zip) au service et attend le résultat : PDF éventuel et journal.
pub async fn exchange(session: &Arc<Session>, service: &str, zip: Vec<u8>, main: &str, engine: &str) -> ApiResult<(Option<Vec<u8>>, String)> {
    if zip.len() > MAX_REQUEST_BODY {
        return Err(ApiError::bad_request("latex_tunnel_too_large"));
    }
    let mut u = reqwest::Url::parse("http://tunnel.invalid/compile").expect("static url");
    u.query_pairs_mut().append_pair("async", "1").append_pair("main", main).append_pair("engine", engine);
    let q = u.query().unwrap_or_default().to_string();
    let headers: Headers = vec![("content-type".into(), "application/zip".into()), ("accept".into(), "application/json".into())];
    let r = relay::http(session, service, KIND, "POST", &format!("/compile?{q}"), headers, Some(zip)).await.map_err(tunnel_error)?;
    if r.status != 202 {
        return Err(service_error(&r.body));
    }
    let id = serde_json::from_slice::<Value>(&r.body)
        .ok()
        .and_then(|v| v["id"].as_str().map(str::to_string))
        .filter(|i| !i.is_empty() && i.len() <= 64 && i.chars().all(|c| c.is_ascii_alphanumeric()))
        .ok_or_else(|| ApiError::bad_request("latex_tunnel_unreachable"))?;
    let mut cancel = Cancel { session: session.clone(), service: service.to_string(), id: Some(id.clone()) };
    let deadline = Instant::now() + TOTAL;
    loop {
        if Instant::now() > deadline {
            return Err(ApiError::bad_request("latex_timeout"));
        }
        let r = relay::http(session, service, KIND, "GET", &format!("/jobs/{id}?wait=20"), vec![], None).await.map_err(tunnel_error)?;
        match r.status {
            202 => continue,
            200 => {
                cancel.done().await;
                return unzip_result(&r.body);
            }
            _ => {
                cancel.id = None;
                return Err(service_error(&r.body));
            }
        }
    }
}

/// Compile le projet sur le PC. Même réponse que `latex::compile`, avec `via: "tunnel"`.
pub async fn compile(state: &AppState, route: &Route, caller: Uuid, project: Uuid, main: &str, engine: &str, overrides: &HashMap<Uuid, String>) -> ApiResult<Value> {
    let session = relay::connected_for(route.tunnel_id, caller).map_err(tunnel_error)?;
    if !session.services().iter().any(|s| s.name == route.service && s.kind == KIND) {
        return Err(ApiError::bad_request("latex_tunnel_service"));
    }
    let slots = SLOTS.entry(route.tunnel_id).or_insert_with(|| Arc::new(Semaphore::new(PER_TUNNEL))).clone();
    let _slot = tokio::time::timeout(Duration::from_secs(120), slots.acquire_owned()).await.map_err(|_| ApiError::bad_request("latex_busy"))?.map_err(|_| ApiError::bad_request("latex_busy"))?;
    let started = Instant::now();
    let files = all_files(state, project).await?;
    if !files.iter().any(|f| f.path == main && !f.is_dir) {
        return Err(ApiError::bad_request("latex_no_main"));
    }
    let mut entries = Vec::new();
    let mut total = 0usize;
    for f in files.iter().filter(|f| !f.is_dir) {
        let bytes = match overrides.get(&f.id) {
            Some(text) if f.blob_path.is_none() => text.clone().into_bytes(),
            _ => file_bytes(f).await.unwrap_or_default(),
        };
        total += bytes.len();
        if total > 4 * MAX_REQUEST_BODY {
            return Err(ApiError::bad_request("latex_tunnel_too_large"));
        }
        entries.push((f.path.clone(), bytes));
    }
    let zip = tokio::task::spawn_blocking(move || zip_project(entries)).await.map_err(|e| ApiError::Internal(e.to_string()))??;
    let (pdf, log) = exchange(&session, &route.service, zip, main, engine).await?;
    let produced = pdf.as_ref().is_some_and(|p| !p.is_empty());
    let issues = crate::latex::parse_log(&log);
    let ok = produced && !issues.iter().any(|i| i.is_error());
    if let Some(pdf) = pdf.filter(|p| !p.is_empty()) {
        let out = crate::latex::output_path(project);
        if let Some(parent) = out.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        }
        tokio::fs::write(&out, pdf).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    sqlx::query("UPDATE projects SET last_build_at = NOW(), last_build_ok = $2 WHERE id = $1").bind(project).bind(ok).execute(&state.pool).await?;
    let log = if log.len() > 200_000 {
        let mut cut = log.len() - 200_000;
        while !log.is_char_boundary(cut) {
            cut += 1;
        }
        log[cut..].to_string()
    } else {
        log
    };
    Ok(json!({
        "ok": ok,
        "pdf": produced,
        "engine": engine,
        "duration_ms": started.elapsed().as_millis() as u64,
        "issues": issues,
        "log": log,
        "via": "tunnel",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tunnel_relay::{Ctl, Out, Proto, ServiceDecl};
    use tokio::sync::mpsc;

    type Seen = Arc<std::sync::Mutex<Vec<(String, String, Vec<u8>)>>>;

    fn frame(id: u32, data: &[u8]) -> Vec<u8> {
        let mut b = id.to_be_bytes().to_vec();
        b.push(0);
        b.extend_from_slice(data);
        b
    }

    fn result_zip(pdf: Option<&[u8]>, log: &str) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        if let Some(p) = pdf {
            w.start_file("output.pdf", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(p).unwrap();
        }
        w.start_file("compile.log", zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(log.as_bytes()).unwrap();
        w.finish().unwrap().into_inner()
    }

    /// Faux agent : chaque requête reçoit la réponse de `answer(méthode, chemin, nombre d'appels GET déjà faits)`.
    fn fake_agent(services: &[(&str, &str)], answer: impl Fn(&str, &str, usize) -> (u16, Vec<u8>) + Send + 'static) -> (Arc<Session>, Seen) {
        let (tx, mut out) = mpsc::channel(256);
        let decl = services.iter().map(|(n, k)| ServiceDecl { name: n.to_string(), kind: k.to_string() }).collect();
        let session = Session::new(Uuid::new_v4(), Uuid::new_v4(), "test".into(), chrono::Utc::now() + chrono::Duration::days(1), decl, tx);
        let seen: Seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (s, sn) = (session.clone(), seen.clone());
        tokio::spawn(async move {
            let mut open: HashMap<u32, (String, String, Vec<u8>)> = HashMap::new();
            let mut gets = 0usize;
            while let Some(m) = out.recv().await {
                match m {
                    Out::Text(t) => match serde_json::from_str::<Ctl>(&t).unwrap() {
                        Ctl::Open { id, method, path, proto, .. } => {
                            assert_eq!(proto, Proto::Http);
                            open.insert(id, (method, path, Vec::new()));
                        }
                        Ctl::End { id } => {
                            let Some((method, path, body)) = open.remove(&id) else { continue };
                            if method == "GET" {
                                gets += 1;
                            }
                            let (status, reply) = answer(&method, &path, gets);
                            sn.lock().unwrap().push((method, path, body));
                            s.on_text(&serde_json::to_string(&Ctl::Head { id, status, headers: vec![] }).unwrap()).await.unwrap();
                            if !reply.is_empty() {
                                s.on_binary(&frame(id, &reply)).unwrap();
                            }
                            s.on_text(&serde_json::to_string(&Ctl::End { id }).unwrap()).await.unwrap();
                        }
                        _ => {}
                    },
                    Out::Binary(b) => {
                        let id = u32::from_be_bytes(b[..4].try_into().unwrap());
                        if let Some(e) = open.get_mut(&id) {
                            e.2.extend_from_slice(&b[5..]);
                        }
                    }
                    Out::Close(..) => {}
                }
            }
        });
        (session, seen)
    }

    fn code(e: ApiError) -> String {
        match e {
            ApiError::BadRequest(c) => c,
            other => format!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn compiles_through_the_latex_service() {
        let reply = result_zip(Some(b"%PDF-1.5 fake"), "./main.tex:3: Undefined control sequence.\n");
        let (session, seen) = fake_agent(&[("tex", "latex")], move |m, p, gets| match (m, p) {
            ("POST", _) => (202, br#"{"id":"abc123","status":"running"}"#.to_vec()),
            ("GET", _) if gets < 2 => (202, br#"{"status":"running"}"#.to_vec()),
            ("GET", _) => (200, reply.clone()),
            _ => (204, vec![]),
        });
        let zip_in = zip_project(vec![("main.tex".into(), b"hi".to_vec())]).unwrap();
        let (pdf, log) = exchange(&session, "tex", zip_in, "chap one/main.tex", "xelatex").await.unwrap();
        assert_eq!(pdf.as_deref(), Some(b"%PDF-1.5 fake".as_slice()));
        assert!(log.contains("Undefined control sequence"));
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0].0, "POST");
        assert_eq!(seen[0].1, "/compile?async=1&main=chap+one%2Fmain.tex&engine=xelatex");
        let mut z = zip::ZipArchive::new(Cursor::new(seen[0].2.as_slice())).unwrap();
        assert!(z.by_name("main.tex").is_ok());
        assert_eq!(seen.iter().filter(|s| s.1.starts_with("/jobs/abc123?wait=")).count(), 2);
        assert_eq!((seen.last().unwrap().0.as_str(), seen.last().unwrap().1.as_str()), ("DELETE", "/jobs/abc123"));
        // Le journal sert à `parse_log`, comme pour une compilation locale.
        assert!(crate::latex::parse_log(&log).iter().any(|i| i.is_error()));
    }

    #[tokio::test]
    async fn only_declared_latex_services_and_clear_errors() {
        let (jup, _) = fake_agent(&[("tex", "jupyter")], |_, _, _| (202, vec![]));
        assert_eq!(code(exchange(&jup, "tex", vec![], "main.tex", "pdflatex").await.unwrap_err()), "latex_tunnel_service");
        let (other, _) = fake_agent(&[("tex", "latex")], |_, _, _| (202, vec![]));
        assert_eq!(code(exchange(&other, "pc", vec![], "main.tex", "pdflatex").await.unwrap_err()), "latex_tunnel_service");
        assert_eq!(code(exchange(&other, "tex", vec![], "main.tex", "pdflatex").await.unwrap_err()), "latex_tunnel_unreachable");
        let (busy, _) = fake_agent(&[("tex", "latex")], |_, _, _| (503, br#"{"error":"latex_busy"}"#.to_vec()));
        assert_eq!(code(exchange(&busy, "tex", vec![], "main.tex", "pdflatex").await.unwrap_err()), "latex_busy");
        let (nomain, _) = fake_agent(&[("tex", "latex")], |_, _, _| (422, br#"{"error":"latex_no_main"}"#.to_vec()));
        assert_eq!(code(exchange(&nomain, "tex", vec![], "main.tex", "pdflatex").await.unwrap_err()), "latex_no_main");
        let (huge, _) = fake_agent(&[("tex", "latex")], |_, _, _| (202, vec![]));
        let big = vec![0u8; MAX_REQUEST_BODY + 1];
        assert_eq!(code(exchange(&huge, "tex", big, "main.tex", "pdflatex").await.unwrap_err()), "latex_tunnel_too_large");
        // Tunnel fermé : hors ligne.
        let (gone, _) = fake_agent(&[("tex", "latex")], |_, _, _| (202, vec![]));
        gone.shutdown();
        assert_eq!(code(exchange(&gone, "tex", vec![], "main.tex", "pdflatex").await.unwrap_err()), "latex_tunnel_offline");
    }

    #[tokio::test]
    async fn dropping_the_compile_cancels_the_job() {
        let (session, seen) = fake_agent(&[("tex", "latex")], |m, _, _| match m {
            "POST" => (202, br#"{"id":"job9"}"#.to_vec()),
            "GET" => (202, br#"{"status":"running"}"#.to_vec()),
            _ => (204, vec![]),
        });
        let s = session.clone();
        let task = tokio::spawn(async move { exchange(&s, "tex", vec![], "main.tex", "pdflatex").await });
        tokio::time::sleep(Duration::from_millis(300)).await;
        task.abort();
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(seen.lock().unwrap().iter().any(|s| s.0 == "DELETE" && s.1 == "/jobs/job9"));
    }
}
