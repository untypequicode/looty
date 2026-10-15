//! Synchronisation bidirectionnelle des tâches avec des calendriers CalDAV (VTODO).
//!
//! Chaque utilisateur connecte ses propres calendriers ; ses tâches assignées y sont
//! poussées, et les modifications faites dans l'application calendrier (titre, dates,
//! priorité, description, terminé) reviennent dans Looty. La détection des changements
//! se fait à trois voies (instantané du dernier état synchronisé) et toutes les
//! écritures sont conditionnelles (ETag) : aucune modification n'est écrasée en silence.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use reqwest::{header, Method, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

use crate::access::AuthUser;
use crate::board::{sync_apply_remote, RemoteChanges};
use crate::error::{ApiError, ApiResult};
use crate::ical::{self, TodoFields, TodoMeta};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Garde-fous réseau (SSRF)
// ---------------------------------------------------------------------------

pub(crate) fn allow_private() -> bool {
    std::env::var("CALDAV_ALLOW_PRIVATE").as_deref() == Ok("true")
}

pub(crate) fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local() || v.is_unspecified() || v.is_broadcast()
            || v.octets()[0] == 100 && (v.octets()[1] & 0xC0) == 64 // CGNAT
            || v.octets()[0] == 169 && v.octets()[1] == 254,
        IpAddr::V6(v) => v.is_loopback() || v.is_unspecified() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80
            || v.to_ipv4_mapped().map(|m| is_private(IpAddr::V4(m))).unwrap_or(false),
    }
}

/// Vérifie qu'une URL peut être contactée par le serveur : https (ou http si réseau privé
/// autorisé) et adresse publique, sauf si `CALDAV_ALLOW_PRIVATE=true`.
pub(crate) async fn check_url(raw: &str) -> ApiResult<Url> {
    let url = Url::parse(raw.trim()).map_err(|_| ApiError::bad_request("caldav_invalid_url"))?;
    let private_ok = allow_private();
    match url.scheme() {
        "https" => {}
        "http" if private_ok => {}
        _ => return Err(ApiError::bad_request("caldav_https_required")),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ApiError::bad_request("caldav_invalid_url"));
    }
    let host = url.host_str().ok_or_else(|| ApiError::bad_request("caldav_invalid_url"))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs = tokio::net::lookup_host((host, port)).await.map_err(|_| ApiError::bad_request("caldav_unreachable"))?;
    let ips: Vec<IpAddr> = addrs.map(|a| a.ip()).collect();
    if ips.is_empty() {
        return Err(ApiError::bad_request("caldav_unreachable"));
    }
    if !private_ok && ips.iter().any(|ip| is_private(*ip)) {
        return Err(ApiError::bad_request("caldav_private_address"));
    }
    Ok(url)
}

fn http() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(8))
            .user_agent("Looty-CalDAV/1.0")
            .build()
            .expect("http client")
    })
}

// ---------------------------------------------------------------------------
// Client WebDAV / CalDAV
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct Dav {
    pub(crate) user: String,
    pub(crate) pass: String,
}

pub(crate) struct DavResponse {
    pub(crate) status: StatusCode,
    pub(crate) etag: Option<String>,
    pub(crate) body: String,
    pub(crate) url: Url,
}

impl Dav {
    /// Requête avec suivi manuel des redirections (chaque cible est revérifiée).
    pub(crate) async fn send(&self, method: &str, url: &Url, depth: Option<&str>, body: Option<String>, extra: &[(header::HeaderName, String)]) -> ApiResult<DavResponse> {
        let mut target = url.clone();
        for _ in 0..4 {
            let m = Method::from_bytes(method.as_bytes()).unwrap();
            let mut req = http().request(m, target.clone()).basic_auth(&self.user, Some(&self.pass));
            if let Some(d) = depth {
                req = req.header("Depth", d);
            }
            for (k, v) in extra {
                req = req.header(k, v);
            }
            if let Some(b) = &body {
                let ct = if method == "PUT" { "text/calendar; charset=utf-8" } else { "application/xml; charset=utf-8" };
                req = req.header(header::CONTENT_TYPE, ct).body(b.clone());
            }
            let resp = req.send().await.map_err(|e| ApiError::BadRequest(format!("caldav_unreachable: {e}")))?;
            let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            if status.is_redirection() {
                if let Some(loc) = resp.headers().get(header::LOCATION).and_then(|l| l.to_str().ok()) {
                    let next = target.join(loc).map_err(|_| ApiError::bad_request("caldav_invalid_url"))?;
                    target = check_url(next.as_str()).await?;
                    continue;
                }
            }
            let etag = resp.headers().get(header::ETAG).and_then(|e| e.to_str().ok()).map(|s| s.to_string());
            let text = resp.text().await.unwrap_or_default();
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(ApiError::bad_request("caldav_auth_failed"));
            }
            return Ok(DavResponse { status, etag, body: text, url: target });
        }
        Err(ApiError::bad_request("caldav_too_many_redirects"))
    }

    /// ETag d'une ressource, quand le serveur ne le renvoie pas après un PUT.
    async fn etag_of(&self, url: &Url, put_etag: Option<String>) -> Option<String> {
        if put_etag.is_some() {
            return put_etag;
        }
        self.propfind(url, "0", "<d:getetag/>").await.ok()?.1.into_iter().next()?.props.get("getetag").cloned()
    }

    async fn propfind(&self, url: &Url, depth: &str, props: &str) -> ApiResult<(Url, Vec<MsResponse>)> {
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/" xmlns:ic="http://apple.com/ns/ical/"><d:prop>{props}</d:prop></d:propfind>"#
        );
        let r = self.send("PROPFIND", url, Some(depth), Some(body), &[]).await?;
        if r.status.as_u16() != 207 {
            return Err(ApiError::BadRequest(format!("caldav_unexpected_status: {}", r.status.as_u16())));
        }
        Ok((r.url, parse_multistatus(&r.body)?))
    }
}

/// Une entrée `<d:response>` d'une réponse multistatus.
#[derive(Debug, Default)]
pub(crate) struct MsResponse {
    pub(crate) href: String,
    /// propriété (nom local) -> texte
    pub(crate) props: HashMap<String, String>,
    /// propriété -> hrefs imbriqués (current-user-principal, calendar-home-set)
    hrefs: HashMap<String, Vec<String>>,
    /// nom local des éléments de resourcetype
    types: Vec<String>,
    /// composants supportés (VTODO, VEVENT...)
    comps: Vec<String>,
}

pub(crate) fn parse_multistatus(xml: &str) -> ApiResult<Vec<MsResponse>> {
    let doc = roxmltree::Document::parse(xml).map_err(|_| ApiError::bad_request("caldav_bad_xml"))?;
    let mut out = Vec::new();
    for resp in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "response") {
        let mut r = MsResponse::default();
        if let Some(h) = resp.children().find(|n| n.tag_name().name() == "href") {
            r.href = h.text().unwrap_or("").trim().to_string();
        }
        for propstat in resp.children().filter(|n| n.tag_name().name() == "propstat") {
            let ok = propstat
                .children()
                .find(|n| n.tag_name().name() == "status")
                .and_then(|s| s.text())
                .map(|s| s.contains(" 200"))
                .unwrap_or(true);
            if !ok {
                continue;
            }
            let Some(prop) = propstat.children().find(|n| n.tag_name().name() == "prop") else { continue };
            for p in prop.children().filter(|n| n.is_element()) {
                let name = p.tag_name().name().to_string();
                match name.as_str() {
                    "resourcetype" => r.types = p.children().filter(|n| n.is_element()).map(|n| n.tag_name().name().to_string()).collect(),
                    "supported-calendar-component-set" => {
                        r.comps = p.children().filter(|n| n.is_element()).filter_map(|n| n.attribute("name").map(|s| s.to_ascii_uppercase())).collect()
                    }
                    _ => {
                        let hs: Vec<String> = p.descendants().filter(|n| n.tag_name().name() == "href").filter_map(|n| n.text()).map(|s| s.trim().to_string()).collect();
                        if !hs.is_empty() {
                            r.hrefs.insert(name.clone(), hs);
                        }
                        let text: String = p.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect::<String>().trim().to_string();
                        r.props.insert(name, text);
                    }
                }
            }
        }
        out.push(r);
    }
    Ok(out)
}

/// Chemin normalisé (décodé, sans barre finale) pour comparer des hrefs.
fn norm_path(base: &Url, href: &str) -> String {
    let full = base.join(href).map(|u| u.path().to_string()).unwrap_or_else(|_| href.to_string());
    let decoded = percent_decode(&full);
    decoded.trim_end_matches('/').to_string()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

// ---------------------------------------------------------------------------
// Découverte des calendriers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct DiscoveredCalendar {
    pub url: String,
    pub name: String,
    pub color: Option<String>,
    pub supports_todo: bool,
}

async fn discover(server_url: &str, dav: &Dav) -> ApiResult<Vec<DiscoveredCalendar>> {
    let start = check_url(server_url).await?;
    let principal_props = "<d:current-user-principal/><d:resourcetype/><c:calendar-home-set/>";

    // 1) L'URL donnée, ou à défaut /.well-known/caldav
    let attempt = dav.propfind(&start, "0", principal_props).await;
    let (base, first) = match attempt {
        Ok(v) => v,
        Err(_) => {
            let wk = start.join("/.well-known/caldav").map_err(|_| ApiError::bad_request("caldav_invalid_url"))?;
            dav.propfind(&wk, "0", principal_props).await?
        }
    };
    let first = first.into_iter().next().unwrap_or_default();

    // L'utilisateur a peut-être donné directement l'URL d'un calendrier
    if first.types.iter().any(|t| t == "calendar") {
        let (_, cal) = dav.propfind(&base, "0", "<d:displayname/><c:supported-calendar-component-set/><ic:calendar-color/><d:resourcetype/>").await?;
        let c = cal.into_iter().next().unwrap_or_default();
        return Ok(vec![DiscoveredCalendar {
            url: base.to_string(),
            name: c.props.get("displayname").cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| "Calendar".into()),
            color: c.props.get("calendar-color").cloned().filter(|s| !s.is_empty()),
            supports_todo: c.comps.is_empty() || c.comps.iter().any(|x| x == "VTODO"),
        }]);
    }

    // 2) calendar-home-set (directement, ou via le principal)
    let mut home = first.hrefs.get("calendar-home-set").and_then(|h| h.first()).cloned();
    if home.is_none() {
        let principal = first.hrefs.get("current-user-principal").and_then(|h| h.first()).cloned().ok_or_else(|| ApiError::bad_request("caldav_no_principal"))?;
        let purl = check_url(base.join(&principal).map_err(|_| ApiError::bad_request("caldav_invalid_url"))?.as_str()).await?;
        let (_, p) = dav.propfind(&purl, "0", "<c:calendar-home-set/>").await?;
        home = p.into_iter().next().and_then(|r| r.hrefs.get("calendar-home-set").and_then(|h| h.first()).cloned());
    }
    let home = home.ok_or_else(|| ApiError::bad_request("caldav_no_calendar_home"))?;
    let home_url = check_url(base.join(&home).map_err(|_| ApiError::bad_request("caldav_invalid_url"))?.as_str()).await?;

    // 3) Liste des calendriers
    let (hbase, list) = dav
        .propfind(&home_url, "1", "<d:displayname/><d:resourcetype/><c:supported-calendar-component-set/><ic:calendar-color/>")
        .await?;
    let mut cals: Vec<DiscoveredCalendar> = list
        .into_iter()
        .filter(|r| r.types.iter().any(|t| t == "calendar"))
        .filter_map(|r| {
            let url = hbase.join(&r.href).ok()?;
            Some(DiscoveredCalendar {
                url: url.to_string(),
                name: r.props.get("displayname").cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| r.href.trim_end_matches('/').rsplit('/').next().unwrap_or("Calendar").to_string()),
                color: r.props.get("calendar-color").cloned().filter(|s| !s.is_empty()).map(|c| c.chars().take(7).collect()),
                supports_todo: r.comps.is_empty() || r.comps.iter().any(|x| x == "VTODO"),
            })
        })
        .collect();
    cals.sort_by(|a, b| b.supports_todo.cmp(&a.supports_todo).then(a.name.cmp(&b.name)));
    Ok(cals)
}

// ---------------------------------------------------------------------------
// Moteur de synchronisation
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow, Clone)]
struct Account {
    id: Uuid,
    user_id: Uuid,
    calendar_url: String,
    username: String,
    password_enc: String,
    board_ids: Option<Vec<Uuid>>,
    include_completed: bool,
}

#[derive(sqlx::FromRow)]
struct Link {
    task_id: Uuid,
    uid: String,
    href: String,
    etag: Option<String>,
    pushed_hash: Option<String>,
    snapshot: Option<serde_json::Value>,
    remote_deleted: bool,
}

#[derive(sqlx::FromRow)]
struct SyncTask {
    id: Uuid,
    title: String,
    description: Option<String>,
    due_date: Option<DateTime<Utc>>,
    start_date: Option<DateTime<Utc>>,
    priority: i32,
    is_completed: bool,
    is_per_person: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    workspace_id: Uuid,
    my_validation: bool,
}

/// Une tâche « par personne » est terminée pour moi dès que je l'ai validée.
fn fields_of(t: &SyncTask) -> TodoFields {
    TodoFields {
        summary: t.title.clone(),
        description: ical::html_to_text(t.description.as_deref().unwrap_or("")),
        due: t.due_date,
        start: t.start_date,
        priority: t.priority,
        completed: if t.is_per_person { t.my_validation || t.is_completed } else { t.is_completed },
    }
}

fn hash(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// Champs modifiés dans le calendrier depuis le dernier état synchronisé.
fn remote_changes(remote: &TodoFields, snap: &TodoFields, local: &TodoFields) -> RemoteChanges {
    let mut c = RemoteChanges::default();
    if remote.summary != snap.summary && !remote.summary.trim().is_empty() && remote.summary != local.summary {
        c.title = Some(remote.summary.trim().to_string());
    }
    if remote.description != snap.description && remote.description != local.description {
        let html = ical::text_to_html(&remote.description);
        c.description = Some(if html.is_empty() { None } else { Some(html) });
    }
    if remote.due != snap.due && remote.due != local.due {
        c.due = Some(remote.due);
    }
    if remote.start != snap.start && remote.start != local.start {
        c.start = Some(remote.start);
    }
    if remote.priority != snap.priority && remote.priority != local.priority {
        c.priority = Some(remote.priority);
    }
    if remote.completed != snap.completed && remote.completed != local.completed {
        c.completed = Some(remote.completed);
    }
    c
}

/// Un seul passage de synchronisation à la fois par compte.
fn running() -> &'static Mutex<HashSet<Uuid>> {
    static R: OnceLock<Mutex<HashSet<Uuid>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashSet::new()))
}

#[derive(Serialize, Default)]
pub struct SyncReport {
    pub pushed: u32,
    pub pulled: u32,
    pub deleted: u32,
}

pub async fn sync_account(state: &AppState, account_id: Uuid) -> ApiResult<SyncReport> {
    {
        let mut r = running().lock().await;
        if !r.insert(account_id) {
            return Ok(SyncReport::default());
        }
    }
    let result = sync_account_inner(state, account_id).await;
    running().lock().await.remove(&account_id);
    let pool = &state.pool;
    match &result {
        Ok(_) => {
            sqlx::query("UPDATE caldav_accounts SET last_sync_at = NOW(), last_error = NULL WHERE id = $1").bind(account_id).execute(pool).await.ok();
        }
        Err(e) => {
            let msg = match e {
                ApiError::BadRequest(m) | ApiError::Conflict(m) | ApiError::Internal(m) => m.clone(),
                other => format!("{other:?}"),
            };
            sqlx::query("UPDATE caldav_accounts SET last_sync_at = NOW(), last_error = $2 WHERE id = $1").bind(account_id).bind(msg.chars().take(500).collect::<String>()).execute(pool).await.ok();
        }
    }
    result
}

async fn sync_account_inner(state: &AppState, account_id: Uuid) -> ApiResult<SyncReport> {
    let pool = &state.pool;
    let acc: Account = sqlx::query_as(
        "SELECT id, user_id, calendar_url, username, password_enc, board_ids, include_completed FROM caldav_accounts WHERE id = $1 AND enabled",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    let pass = crate::crypto::decrypt(&acc.password_enc).ok_or_else(|| ApiError::bad_request("caldav_secret_unreadable"))?;
    let dav = Dav { user: acc.username.clone(), pass };
    let cal = check_url(&acc.calendar_url).await?;
    let mut report = SyncReport::default();

    // 1) Suppressions en attente (tâches supprimées dans Looty)
    let pending: Vec<(String, Option<String>)> = sqlx::query_as("SELECT href, etag FROM caldav_pending_deletes WHERE account_id = $1").bind(acc.id).fetch_all(pool).await?;
    for (href, _etag) in pending {
        if let Ok(u) = cal.join(&href) {
            let r = dav.send("DELETE", &u, None, None, &[]).await?;
            if r.status.is_success() || r.status == StatusCode::NOT_FOUND || r.status == StatusCode::GONE {
                report.deleted += 1;
            }
        }
        sqlx::query("DELETE FROM caldav_pending_deletes WHERE account_id = $1 AND href = $2").bind(acc.id).bind(&href).execute(pool).await?;
    }

    // 2) ETags distants
    let (cbase, listing) = dav.propfind(&cal, "1", "<d:getetag/><d:resourcetype/>").await?;
    let remote: HashMap<String, String> = listing
        .into_iter()
        .filter(|r| !r.types.iter().any(|t| t == "collection" || t == "calendar"))
        .map(|r| (norm_path(&cbase, &r.href), r.props.get("getetag").cloned().unwrap_or_default()))
        .collect();

    // 3) État local
    let links: Vec<Link> = sqlx::query_as(
        "SELECT task_id, uid, href, etag, pushed_hash, snapshot, remote_deleted FROM caldav_task_links WHERE account_id = $1",
    )
    .bind(acc.id)
    .fetch_all(pool)
    .await?;
    let mut links: HashMap<Uuid, Link> = links.into_iter().map(|l| (l.task_id, l)).collect();

    let me = acc.user_id.to_string();
    let load_tasks = || async {
        sqlx::query_as::<_, SyncTask>(
            "SELECT t.id, t.title, t.description, t.due_date, t.start_date, t.priority, t.is_completed, t.is_per_person,
                    t.created_at, t.updated_at, b.workspace_id,
                    EXISTS(SELECT 1 FROM task_validations v WHERE v.task_id = t.id AND v.user_id = $1) AS my_validation
             FROM tasks t
             JOIN task_assignees ta ON ta.task_id = t.id AND ta.user_id = $1
             JOIN board_columns c ON c.id = t.column_id
             JOIN boards b ON b.id = c.board_id
             JOIN workspace_members wm ON wm.workspace_id = b.workspace_id AND wm.user_id = $1
             WHERE ($2::uuid[] IS NULL OR b.id = ANY($2))",
        )
        .bind(&me)
        .bind(&acc.board_ids)
        .fetch_all(pool)
        .await
    };

    // 4) Tirage : ressources modifiées dans le calendrier
    for link in links.values_mut() {
        if link.remote_deleted {
            continue;
        }
        let path = norm_path(&cbase, &link.href);
        match remote.get(&path) {
            None => {
                // Supprimée côté calendrier : on arrête de la renvoyer (la tâche reste dans Looty).
                sqlx::query("UPDATE caldav_task_links SET remote_deleted = TRUE WHERE account_id = $1 AND task_id = $2").bind(acc.id).bind(link.task_id).execute(pool).await?;
                link.remote_deleted = true;
            }
            Some(etag) if Some(etag) != link.etag.as_ref() => {
                let url = cal.join(&link.href).map_err(|_| ApiError::bad_request("caldav_invalid_url"))?;
                let r = dav.send("GET", &url, None, None, &[]).await?;
                if !r.status.is_success() {
                    continue;
                }
                let Some(parsed) = ical::parse_vtodo(&r.body) else { continue };
                // Sécurité : la ressource doit toujours décrire la même tâche.
                if parsed.uid.as_deref().is_some_and(|u| u != link.uid) {
                    continue;
                }
                let snap: TodoFields = link.snapshot.clone().and_then(|s| serde_json::from_value(s).ok()).unwrap_or_default();
                let current: Option<SyncTask> = load_tasks().await?.into_iter().find(|t| t.id == link.task_id);
                if let Some(cur) = current {
                    let changes = remote_changes(&parsed.fields, &snap, &fields_of(&cur));
                    if !changes.is_empty() {
                        match sync_apply_remote(state, link.task_id, acc.user_id, changes).await {
                            Ok(()) => report.pulled += 1,
                            Err(e) => tracing::warn!("caldav: cannot apply remote change on {}: {:?}", link.task_id, e),
                        }
                    }
                }
                // Nouvel état de référence : la version distante telle que lue.
                link.etag = r.etag.or_else(|| Some(etag.clone()));
                link.snapshot = serde_json::to_value(&parsed.fields).ok();
                // Force le renvoi (normalisation du contenu) au passage de poussée.
                link.pushed_hash = None;
                sqlx::query("UPDATE caldav_task_links SET etag = $3, snapshot = $4, pushed_hash = NULL, last_synced_at = NOW() WHERE account_id = $1 AND task_id = $2")
                    .bind(acc.id).bind(link.task_id).bind(&link.etag).bind(&link.snapshot).execute(pool).await?;
            }
            _ => {}
        }
    }

    // 5) Poussée : création / mise à jour des tâches assignées
    let tasks = load_tasks().await?;
    let host = Url::parse(&state.base_url).ok().and_then(|u| u.host_str().map(|s| s.to_string())).unwrap_or_else(|| "looty".into());
    let mut wanted: HashSet<Uuid> = HashSet::new();
    for t in &tasks {
        let linked = links.contains_key(&t.id);
        if t.is_completed && !acc.include_completed && !linked {
            continue;
        }
        wanted.insert(t.id);
        if links.get(&t.id).map(|l| l.remote_deleted).unwrap_or(false) {
            continue;
        }
        let categories: Vec<String> = sqlx::query_scalar("SELECT l.name FROM labels l JOIN task_labels tl ON tl.label_id = l.id WHERE tl.task_id = $1 ORDER BY l.name")
            .bind(t.id).fetch_all(pool).await?;
        let fields = fields_of(t);
        let uid = links.get(&t.id).map(|l| l.uid.clone()).unwrap_or_else(|| format!("looty-{}@{}", t.id, host));
        let url = format!("{}/w/{}/task/{}", state.base_url.trim_end_matches('/'), t.workspace_id, t.id);
        let body = ical::build_vtodo(&fields, &TodoMeta { uid: &uid, url: &url, categories: &categories, last_modified: t.updated_at, created: t.created_at });
        let h = hash(&body);

        match links.get(&t.id) {
            Some(l) if l.pushed_hash.as_deref() == Some(h.as_str()) => {}
            Some(l) => {
                let target = cal.join(&l.href).map_err(|_| ApiError::bad_request("caldav_invalid_url"))?;
                let mut hdr = vec![];
                if let Some(e) = &l.etag {
                    hdr.push((header::IF_MATCH, e.clone()));
                }
                let r = dav.send("PUT", &target, None, Some(body), &hdr).await?;
                if r.status == StatusCode::PRECONDITION_FAILED {
                    // Modifiée à distance entre-temps : sera tirée au prochain passage.
                    continue;
                }
                if !r.status.is_success() {
                    return Err(ApiError::BadRequest(format!("caldav_put_failed: {}", r.status.as_u16())));
                }
                sqlx::query("UPDATE caldav_task_links SET etag = $3, pushed_hash = $4, snapshot = $5, last_synced_at = NOW() WHERE account_id = $1 AND task_id = $2")
                    .bind(acc.id).bind(t.id).bind(dav.etag_of(&target, r.etag.clone()).await).bind(&h).bind(serde_json::to_value(&fields).ok()).execute(pool).await?;
                report.pushed += 1;
            }
            None => {
                let href = format!("looty-{}.ics", t.id);
                let target = cal.join(&href).map_err(|_| ApiError::bad_request("caldav_invalid_url"))?;
                let r = dav.send("PUT", &target, None, Some(body), &[(header::IF_NONE_MATCH, "*".into())]).await?;
                if !(r.status.is_success() || r.status == StatusCode::PRECONDITION_FAILED) {
                    return Err(ApiError::BadRequest(format!("caldav_put_failed: {}", r.status.as_u16())));
                }
                sqlx::query(
                    "INSERT INTO caldav_task_links (account_id, task_id, uid, href, etag, pushed_hash, snapshot)
                     VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (account_id, task_id) DO NOTHING",
                )
                .bind(acc.id).bind(t.id).bind(&uid).bind(target.path()).bind(dav.etag_of(&target, r.etag.clone()).await).bind(&h).bind(serde_json::to_value(&fields).ok())
                .execute(pool).await?;
                report.pushed += 1;
            }
        }
    }

    // 6) Tâches qui ne doivent plus figurer dans ce calendrier (désassignées, filtrées)
    for (task_id, l) in links.iter() {
        if wanted.contains(task_id) {
            continue;
        }
        if !l.remote_deleted {
            if let Ok(u) = cal.join(&l.href) {
                let mut hdr = vec![];
                if let Some(e) = &l.etag {
                    hdr.push((header::IF_MATCH, e.clone()));
                }
                let _ = dav.send("DELETE", &u, None, None, &hdr).await;
                report.deleted += 1;
            }
        }
        // Suppression interne : la ressource distante est déjà traitée, pas de file d'attente.
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT set_config('looty.caldav_internal', 'on', true)").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM caldav_task_links WHERE account_id = $1 AND task_id = $2").bind(acc.id).bind(task_id).execute(&mut *tx).await?;
        tx.commit().await?;
    }

    Ok(report)
}

/// Boucle de fond : toutes les minutes, ou quelques secondes après une modification de tâche.
pub fn start_sync_loop(state: AppState, notify: Arc<Notify>) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                _ = notify.notified() => {
                    // Regroupe les rafales de modifications
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
            let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM caldav_accounts WHERE enabled ORDER BY last_sync_at NULLS FIRST")
                .fetch_all(&state.pool)
                .await
                .unwrap_or_default();
            for id in ids {
                if let Err(e) = sync_account(&state, id).await {
                    tracing::debug!("caldav sync {} failed: {:?}", id, e);
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_accounts).post(create_account))
        .route("/discover", post(discover_handler))
        .route("/{id}", put(update_account).delete(delete_account))
        .route("/{id}/sync", post(sync_now))
}

#[derive(Serialize, sqlx::FromRow)]
struct AccountDto {
    id: Uuid,
    name: String,
    server_url: String,
    calendar_url: String,
    calendar_name: Option<String>,
    calendar_color: Option<String>,
    username: String,
    board_ids: Option<Vec<Uuid>>,
    include_completed: bool,
    enabled: bool,
    last_sync_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    task_count: i64,
}

const ACCOUNT_SELECT: &str = "SELECT a.id, a.name, a.server_url, a.calendar_url, a.calendar_name, a.calendar_color, a.username,
    a.board_ids, a.include_completed, a.enabled, a.last_sync_at, a.last_error,
    (SELECT count(*) FROM caldav_task_links l WHERE l.account_id = a.id AND NOT l.remote_deleted) AS task_count
    FROM caldav_accounts a";

async fn list_accounts(State(pool): State<PgPool>, user: AuthUser) -> ApiResult<Json<Vec<AccountDto>>> {
    Ok(Json(sqlx::query_as(sqlx::AssertSqlSafe(format!("{ACCOUNT_SELECT} WHERE a.user_id = $1 ORDER BY a.created_at"))).bind(user.id).fetch_all(&pool).await?))
}

#[derive(Deserialize)]
struct Credentials {
    server_url: String,
    username: String,
    password: String,
}

async fn discover_handler(_user: AuthUser, Json(c): Json<Credentials>) -> ApiResult<Json<Vec<DiscoveredCalendar>>> {
    let dav = Dav { user: c.username.trim().to_string(), pass: c.password };
    Ok(Json(discover(&c.server_url, &dav).await?))
}

#[derive(Deserialize)]
struct CreateAccount {
    name: String,
    server_url: String,
    calendar_url: String,
    calendar_name: Option<String>,
    calendar_color: Option<String>,
    username: String,
    password: String,
    board_ids: Option<Vec<Uuid>>,
    include_completed: Option<bool>,
}

fn clean_color(c: Option<String>) -> Option<String> {
    c.filter(|c| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit()))
}

async fn create_account(State(state): State<AppState>, user: AuthUser, Json(p): Json<CreateAccount>) -> ApiResult<Json<AccountDto>> {
    let name = p.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(ApiError::bad_request("invalid_name"));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM caldav_accounts WHERE user_id = $1").bind(user.id).fetch_one(&state.pool).await?;
    if count >= 10 {
        return Err(ApiError::bad_request("caldav_too_many"));
    }
    // Vérifie l'accès au calendrier choisi avant d'enregistrer.
    let dav = Dav { user: p.username.trim().to_string(), pass: p.password.clone() };
    let cal = check_url(&p.calendar_url).await?;
    dav.propfind(&cal, "0", "<d:resourcetype/>").await?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO caldav_accounts (user_id, name, server_url, calendar_url, calendar_name, calendar_color, username, password_enc, board_ids, include_completed)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
    )
    .bind(user.id)
    .bind(name)
    .bind(p.server_url.trim())
    .bind(cal.as_str())
    .bind(p.calendar_name.map(|s| s.chars().take(255).collect::<String>()))
    .bind(clean_color(p.calendar_color))
    .bind(p.username.trim())
    .bind(crate::crypto::encrypt(&p.password))
    .bind(p.board_ids.filter(|b| !b.is_empty()))
    .bind(p.include_completed.unwrap_or(true))
    .fetch_one(&state.pool)
    .await?;

    // Première synchronisation en arrière-plan
    let st = state.clone();
    tokio::spawn(async move { let _ = sync_account(&st, id).await; });
    Ok(Json(sqlx::query_as(sqlx::AssertSqlSafe(format!("{ACCOUNT_SELECT} WHERE a.id = $1"))).bind(id).fetch_one(&state.pool).await?))
}

#[derive(Deserialize)]
struct UpdateAccount {
    name: Option<String>,
    password: Option<String>,
    enabled: Option<bool>,
    include_completed: Option<bool>,
    /// `null` = tous les tableaux
    #[serde(default, deserialize_with = "double_option")]
    board_ids: Option<Option<Vec<Uuid>>>,
}

fn double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(de).map(Some)
}

async fn update_account(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<UpdateAccount>) -> ApiResult<Json<AccountDto>> {
    let name = p.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let pass = p.password.filter(|s| !s.is_empty()).map(|s| crate::crypto::encrypt(&s));
    let n = sqlx::query(
        "UPDATE caldav_accounts SET name = COALESCE($3, name), password_enc = COALESCE($4, password_enc),
            enabled = COALESCE($5, enabled), include_completed = COALESCE($6, include_completed),
            board_ids = CASE WHEN $7 THEN $8 ELSE board_ids END, last_error = NULL
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .bind(name)
    .bind(pass)
    .bind(p.enabled)
    .bind(p.include_completed)
    .bind(p.board_ids.is_some())
    .bind(p.board_ids.flatten().filter(|b| !b.is_empty()))
    .execute(&state.pool)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    let st = state.clone();
    tokio::spawn(async move { let _ = sync_account(&st, id).await; });
    Ok(Json(sqlx::query_as(sqlx::AssertSqlSafe(format!("{ACCOUNT_SELECT} WHERE a.id = $1"))).bind(id).fetch_one(&state.pool).await?))
}

#[derive(Deserialize)]
struct DeleteQuery {
    /// Supprime aussi les tâches créées dans le calendrier (par défaut : oui).
    keep_remote: Option<bool>,
}

async fn delete_account(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<DeleteQuery>) -> ApiResult<StatusCode> {
    let acc: Account = sqlx::query_as(
        "SELECT id, user_id, calendar_url, username, password_enc, board_ids, include_completed FROM caldav_accounts WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    if !q.keep_remote.unwrap_or(false) {
        if let (Some(pass), Ok(cal)) = (crate::crypto::decrypt(&acc.password_enc), check_url(&acc.calendar_url).await) {
            let dav = Dav { user: acc.username.clone(), pass };
            let hrefs: Vec<String> = sqlx::query_scalar("SELECT href FROM caldav_task_links WHERE account_id = $1 AND NOT remote_deleted").bind(id).fetch_all(&state.pool).await?;
            for h in hrefs {
                if let Ok(u) = cal.join(&h) {
                    let _ = dav.send("DELETE", &u, None, None, &[]).await;
                }
            }
        }
    }
    sqlx::query("DELETE FROM caldav_accounts WHERE id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn sync_now(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM caldav_accounts WHERE id = $1").bind(id).fetch_optional(&state.pool).await?;
    if owner != Some(user.id) {
        return Err(ApiError::NotFound);
    }
    let report = sync_account(&state, id).await?;
    let acc: AccountDto = sqlx::query_as(sqlx::AssertSqlSafe(format!("{ACCOUNT_SELECT} WHERE a.id = $1"))).bind(id).fetch_one(&state.pool).await?;
    Ok(Json(serde_json::json!({ "report": report, "account": acc })))
}

/// Calendriers de l'utilisateur dans lesquels une tâche est synchronisée.
pub async fn task_links(State(pool): State<PgPool>, user: AuthUser, Path(task_id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    let ctx = crate::access::task_ctx(&pool, task_id).await?;
    crate::access::require_member(&pool, &user, ctx.workspace_id).await?;
    let rows: Vec<(Uuid, String, Option<String>, DateTime<Utc>, bool)> = sqlx::query_as(
        "SELECT a.id, a.name, a.calendar_color, l.last_synced_at, l.remote_deleted
         FROM caldav_task_links l JOIN caldav_accounts a ON a.id = l.account_id
         WHERE l.task_id = $1 AND a.user_id = $2",
    )
    .bind(task_id)
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    Ok(Json(serde_json::json!(rows
        .into_iter()
        .map(|(id, name, color, at, del)| serde_json::json!({ "account_id": id, "name": name, "color": color, "last_synced_at": at, "remote_deleted": del }))
        .collect::<Vec<_>>())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ranges() {
        for ip in ["127.0.0.1", "10.1.2.3", "192.168.1.10", "172.16.0.1", "169.254.169.254", "100.64.0.1", "::1", "fd00::1", "fe80::1", "::ffff:10.0.0.1", "0.0.0.0"] {
            assert!(is_private(ip.parse().unwrap()), "{ip} should be private");
        }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(!is_private(ip.parse().unwrap()), "{ip} should be public");
        }
    }

    #[tokio::test]
    async fn rejects_unsafe_urls() {
        std::env::remove_var("CALDAV_ALLOW_PRIVATE");
        assert!(check_url("http://example.com/dav").await.is_err(), "plain http refused");
        assert!(check_url("https://127.0.0.1/dav").await.is_err(), "loopback refused");
        assert!(check_url("https://169.254.169.254/latest").await.is_err(), "metadata refused");
        assert!(check_url("https://user:pw@example.com/").await.is_err(), "credentials in URL refused");
        assert!(check_url("ftp://example.com/").await.is_err());
    }

    #[test]
    fn three_way_merge() {
        let snap = TodoFields { summary: "A".into(), priority: 2, ..Default::default() };
        let local = TodoFields { summary: "A".into(), priority: 1, ..Default::default() }; // modifiée dans Looty
        let remote = TodoFields { summary: "B".into(), priority: 2, completed: true, ..Default::default() }; // titre + terminé à distance
        let c = remote_changes(&remote, &snap, &local);
        assert_eq!(c.title.as_deref(), Some("B"));
        assert_eq!(c.completed, Some(true));
        assert!(c.priority.is_none(), "local priority change must not be overwritten");
    }
}
