//! Serveur CalDAV intégré (RFC 4791) : les applications de calendrier / tâches
//! (Thunderbird, DAVx⁵ + jtx Board / Tasks.org, Apple Rappels, Evolution…) se
//! connectent à Looty et y lisent / modifient les tâches sous forme de VTODO.
//!
//! - Authentification HTTP Basic : e-mail + mot de passe d'application (`api_tokens`
//!   de portée `caldav`, inutilisable sur l'API) ; jamais de cookie de session.
//! - Calendriers par utilisateur : « Assignées à moi » (toutes les tâches assignées),
//!   un calendrier de tâches par tableau accessible, et le calendrier d'événements
//!   partagé de chaque espace (VEVENT, cf. events.rs).
//! - Les écritures passent par les mêmes règles que l'interface (droits, historique,
//!   validation par personne, automatisations, temps réel).
//!
//! Arborescence :
//!   /dav/                                   racine (découverte)
//!   /dav/principals/<user>/                 principal
//!   /dav/calendars/<user>/                  calendar-home-set
//!   /dav/calendars/<user>/<calendrier>/     calendrier (`assigned`, `board-<id>`, `events-<espace>` : calendrier
//!                                           par défaut de l'espace, `cal-<id>` : autre calendrier Looty, dont ceux des
//!                                           tableaux, avec leurs étiquettes en CATEGORIES)
//!   /dav/calendars/<user>/<calendrier>/<x>.ics

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use base64::Engine;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::access::{hash_token, Role};
use crate::board::{dav_create_task, dav_unassign, remove_task, sync_apply_remote, RemoteChanges};
use crate::error::ApiError;
use crate::ical::{self, TodoFields, TodoMeta};
use crate::state::AppState;

const NS: &str = r#"xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/" xmlns:ical="http://apple.com/ns/ical/""#;
const DAV_NS: &str = "DAV:";
const CALDAV_NS: &str = "urn:ietf:params:xml:ns:caldav";
const CS_NS: &str = "http://calendarserver.org/ns/";
const ICAL_NS: &str = "http://apple.com/ns/ical/";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/.well-known/caldav", any(well_known))
        .route("/dav", any(handle))
        .route("/dav/", any(handle))
        .route("/dav/{*path}", any(handle))
}

async fn well_known() -> Response {
    Response::builder().status(StatusCode::MOVED_PERMANENTLY).header(header::LOCATION, "/dav/").body(Body::empty()).unwrap()
}

// ---------------------------------------------------------------------------
// Authentification
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct DavUser {
    id: Uuid,
    email: String,
    name: String,
    french: bool,
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Option<DavUser> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Basic ")?;
    let decoded = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(raw.trim()).ok()?).ok()?;
    let (login, password) = decoded.split_once(':')?;
    let row: Option<(Uuid, String, String, Option<String>)> = sqlx::query_as(
        "UPDATE api_tokens t SET last_used_at = NOW()
         FROM users u
         WHERE t.token_hash = $1 AND t.scope = 'caldav' AND u.id = t.user_id AND NOT u.is_suspended
           AND (lower(u.email) = lower($2) OR u.id::text = $2)
         RETURNING u.id, u.email, u.name, u.locale",
    )
    .bind(hash_token(password.trim()))
    .bind(login.trim())
    .fetch_optional(&state.pool)
    .await
    .ok()?;
    row.map(|(id, email, name, locale)| DavUser { id, email, name, french: locale.as_deref() == Some("fr") })
}

fn unauthorized() -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::WWW_AUTHENTICATE, r#"Basic realm="Looty CalDAV", charset="UTF-8""#)
        .body(Body::from("Authentication required"))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Modèle
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum CalKind {
    Assigned,
    Board(Uuid),
    /// Événements du calendrier par défaut d'un espace.
    Events(Uuid),
    /// Événements d'un autre calendrier Looty (calendrier d'un tableau…).
    Cal(Uuid),
}

impl CalKind {
    fn parse(s: &str) -> Option<Self> {
        if s == "assigned" {
            return Some(CalKind::Assigned);
        }
        if let Some(ws) = s.strip_prefix("events-").and_then(|id| Uuid::parse_str(id).ok()) {
            return Some(CalKind::Events(ws));
        }
        if let Some(c) = s.strip_prefix("cal-").and_then(|id| Uuid::parse_str(id).ok()) {
            return Some(CalKind::Cal(c));
        }
        s.strip_prefix("board-").and_then(|id| Uuid::parse_str(id).ok()).map(CalKind::Board)
    }
    fn component(&self) -> &'static str {
        match self {
            CalKind::Events(_) | CalKind::Cal(_) => "VEVENT",
            _ => "VTODO",
        }
    }
    fn key(&self) -> String {
        match self {
            CalKind::Assigned => "assigned".into(),
            CalKind::Board(id) => format!("board-{id}"),
            CalKind::Events(ws) => format!("events-{ws}"),
            CalKind::Cal(c) => format!("cal-{c}"),
        }
    }
}

struct Calendar {
    kind: CalKind,
    name: String,
    description: String,
    writable: bool,
    order: usize,
}

#[derive(sqlx::FromRow)]
struct Item {
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
    categories: Vec<String>,
    dav_uid: Option<String>,
    dav_name: Option<String>,
}

impl Item {
    fn name(&self) -> String {
        self.dav_name.clone().unwrap_or_else(|| format!("{}.ics", self.id))
    }
    fn fields(&self) -> TodoFields {
        TodoFields {
            summary: self.title.clone(),
            description: ical::html_to_text(self.description.as_deref().unwrap_or("")),
            due: self.due_date,
            start: self.start_date,
            priority: self.priority,
            completed: if self.is_per_person { self.my_validation || self.is_completed } else { self.is_completed },
        }
    }
    fn ics(&self, state: &AppState) -> String {
        let host = reqwest::Url::parse(&state.base_url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_else(|| "looty".into());
        let uid = self.dav_uid.clone().unwrap_or_else(|| format!("looty-{}@{}", self.id, host));
        let url = format!("{}/w/{}/task/{}", state.base_url.trim_end_matches('/'), self.workspace_id, self.id);
        ical::build_vtodo(&self.fields(), &TodoMeta { uid: &uid, url: &url, categories: &self.categories, last_modified: self.updated_at, created: self.created_at })
    }
}

/// Ressource d'un calendrier : tâche (VTODO) ou événement partagé (VEVENT).
enum Obj {
    Task(Item),
    Event(crate::events::EventRow),
}

impl Obj {
    fn name(&self) -> String {
        match self {
            Obj::Task(t) => t.name(),
            Obj::Event(e) => e.name(),
        }
    }
    fn ics(&self, state: &AppState) -> String {
        match self {
            Obj::Task(t) => t.ics(state),
            Obj::Event(e) => e.dav_ics(),
        }
    }
}

fn etag_of(body: &str) -> String {
    format!("\"{}\"", &hex::encode(Sha256::digest(body.as_bytes()))[..32])
}

async fn calendars(state: &AppState, user: &DavUser) -> Result<Vec<Calendar>, ApiError> {
    let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT b.id, b.name, w.name, wm.role FROM boards b
         JOIN workspaces w ON w.id = b.workspace_id
         JOIN workspace_members wm ON wm.workspace_id = w.id AND wm.user_id = $1
         ORDER BY w.name, b.created_at",
    )
    .bind(user.id.to_string())
    .fetch_all(&state.pool)
    .await?;
    let mut out = vec![Calendar {
        kind: CalKind::Assigned,
        name: if user.french { "Looty · Mes tâches" } else { "Looty · Assigned to me" }.into(),
        description: if user.french { "Tâches qui me sont assignées" } else { "Tasks assigned to me" }.into(),
        writable: true,
        order: 0,
    }];
    let offset = rows.len() + 1;
    for (i, (id, board, ws, role)) in rows.into_iter().enumerate() {
        out.push(Calendar {
            kind: CalKind::Board(id),
            name: format!("{ws} · {board}"),
            description: format!("{ws} / {board}"),
            writable: Role::parse(&role).map(|r| r.can_write()).unwrap_or(false),
            order: i + 1,
        });
    }
    let spaces: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT w.id, w.name, wm.role FROM workspaces w JOIN workspace_members wm ON wm.workspace_id = w.id AND wm.user_id = $1 ORDER BY w.name",
    )
    .bind(user.id.to_string())
    .fetch_all(&state.pool)
    .await?;
    for (i, (id, ws, role)) in spaces.into_iter().enumerate() {
        out.push(Calendar {
            kind: CalKind::Events(id),
            name: format!("{ws} · {}", if user.french { "Calendrier" } else { "Calendar" }),
            description: if user.french { format!("Événements partagés de l'espace {ws}") } else { format!("Shared events of the {ws} workspace") },
            writable: Role::parse(&role).map(|r| r.can_write()).unwrap_or(false),
            order: offset + i,
        });
    }
    // Autres calendriers Looty (dont un par tableau) : une collection chacun.
    let cals: Vec<(Uuid, String, String, String, bool)> = sqlx::query_as(
        "SELECT c.id, c.name, w.name, wm.role, c.board_id IS NOT NULL FROM looty_calendars c
         JOIN workspaces w ON w.id = c.workspace_id
         JOIN workspace_members wm ON wm.workspace_id = w.id AND wm.user_id = $1
         WHERE NOT c.is_default ORDER BY w.name, c.position, c.created_at",
    )
    .bind(user.id.to_string())
    .fetch_all(&state.pool)
    .await?;
    let offset = out.len();
    for (i, (id, name, ws, role, board)) in cals.into_iter().enumerate() {
        out.push(Calendar {
            kind: CalKind::Cal(id),
            name: format!("{ws} · {name}"),
            description: match (board, user.french) {
                (true, true) => format!("Événements du tableau {name} ({ws}), avec ses étiquettes"),
                (true, false) => format!("Events of the {name} board ({ws}), with its labels"),
                (false, true) => format!("Calendrier {name} de l'espace {ws}"),
                (false, false) => format!("{name} calendar of the {ws} workspace"),
            },
            writable: Role::parse(&role).map(|r| r.can_write()).unwrap_or(false),
            order: offset + i,
        });
    }
    Ok(out)
}

/// Collection d'événements : espace et calendrier Looty (`None` : calendrier par défaut).
async fn event_scope(state: &AppState, user: &DavUser, kind: CalKind) -> Result<Option<(Uuid, Option<Uuid>, Role)>, ApiError> {
    match kind {
        CalKind::Events(ws) => Ok(Some((ws, None, events_role(state, user, ws).await?))),
        CalKind::Cal(c) => {
            let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM looty_calendars WHERE id = $1 AND NOT is_default")
                .bind(c)
                .fetch_optional(&state.pool)
                .await?
                .ok_or(ApiError::NotFound)?;
            Ok(Some((ws, Some(c), events_role(state, user, ws).await?)))
        }
        _ => Ok(None),
    }
}

async fn items(state: &AppState, user: &DavUser, kind: CalKind, only: Option<Uuid>) -> Result<Vec<Item>, ApiError> {
    let board = match kind {
        CalKind::Assigned => None,
        CalKind::Board(id) => Some(id),
        CalKind::Events(_) | CalKind::Cal(_) => return Ok(vec![]),
    };
    Ok(sqlx::query_as::<_, Item>(
        "SELECT t.id, t.title, t.description, t.due_date, t.start_date, t.priority, t.is_completed, t.is_per_person,
                t.created_at, t.updated_at, b.workspace_id,
                EXISTS(SELECT 1 FROM task_validations v WHERE v.task_id = t.id AND v.user_id = $1) AS my_validation,
                COALESCE((SELECT array_agg(l.name::text ORDER BY l.name) FROM labels l JOIN task_labels tl ON tl.label_id = l.id WHERE tl.task_id = t.id), '{}') AS categories,
                r.uid AS dav_uid, r.name AS dav_name
         FROM tasks t
         JOIN board_columns c ON c.id = t.column_id
         JOIN boards b ON b.id = c.board_id
         JOIN workspace_members wm ON wm.workspace_id = b.workspace_id AND wm.user_id = $1
         LEFT JOIN dav_task_resources r ON r.task_id = t.id
         WHERE (($2::uuid IS NULL AND EXISTS(SELECT 1 FROM task_assignees ta WHERE ta.task_id = t.id AND ta.user_id = $1)) OR b.id = $2)
           AND ($3::uuid IS NULL OR t.id = $3)
         ORDER BY t.created_at",
    )
    .bind(user.id.to_string())
    .bind(board)
    .bind(only)
    .fetch_all(&state.pool)
    .await?)
}

/// Membre de l'espace d'un calendrier d'événements (sinon : introuvable).
async fn events_role(state: &AppState, user: &DavUser, ws: Uuid) -> Result<Role, ApiError> {
    crate::access::workspace_role(&state.pool, user.id, ws).await?.ok_or(ApiError::NotFound)
}

/// Ressources d'un calendrier.
async fn objects(state: &AppState, user: &DavUser, kind: CalKind) -> Result<Vec<Obj>, ApiError> {
    if let Some((ws, cal, _)) = event_scope(state, user, kind).await? {
        return Ok(crate::events::list(&state.pool, ws, cal).await?.into_iter().map(Obj::Event).collect());
    }
    Ok(items(state, user, kind, None).await?.into_iter().map(Obj::Task).collect())
}

async fn find_obj(state: &AppState, user: &DavUser, kind: CalKind, name: &str) -> Result<Option<Obj>, ApiError> {
    if let Some((ws, cal, _)) = event_scope(state, user, kind).await? {
        return Ok(crate::events::find_by_name(&state.pool, ws, name, cal).await?.map(Obj::Event));
    }
    Ok(find_item(state, user, kind, name).await?.map(Obj::Task))
}

/// Retrouve une ressource d'un calendrier par son nom de fichier.
async fn find_item(state: &AppState, user: &DavUser, kind: CalKind, name: &str) -> Result<Option<Item>, ApiError> {
    let mapped: Option<Uuid> = sqlx::query_scalar("SELECT task_id FROM dav_task_resources WHERE name = $1").bind(name).fetch_optional(&state.pool).await?;
    let id = mapped.or_else(|| name.strip_suffix(".ics").and_then(|s| Uuid::parse_str(s).ok()));
    let Some(id) = id else { return Ok(None) };
    Ok(items(state, user, kind, Some(id)).await?.into_iter().find(|i| i.name() == name))
}

// ---------------------------------------------------------------------------
// Chemins
// ---------------------------------------------------------------------------

enum Target {
    Root,
    Principal,
    Home,
    Calendar(CalKind),
    Object(CalKind, String),
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|c| if c.is_ascii_alphanumeric() || b"-._~@+".contains(&c) { (c as char).to_string() } else { format!("%{c:02X}") })
        .collect()
}

/// Analyse un chemin (`/dav/...`) ; `None` = hors de l'espace de l'utilisateur.
fn parse_target(path: &str, user: &DavUser) -> Option<Target> {
    let rest = path.strip_prefix("/dav").unwrap_or(path);
    let parts: Vec<String> = rest.split('/').filter(|p| !p.is_empty()).map(decode).collect();
    let me = user.id.to_string();
    match parts.as_slice() {
        [] => Some(Target::Root),
        [p, u] if p == "principals" && *u == me => Some(Target::Principal),
        [c, u] if c == "calendars" && *u == me => Some(Target::Home),
        [c, u, cal] if c == "calendars" && *u == me => CalKind::parse(cal).map(Target::Calendar),
        [c, u, cal, name] if c == "calendars" && *u == me => CalKind::parse(cal).map(|k| Target::Object(k, name.clone())),
        _ => None,
    }
}

fn principal_href(u: &DavUser) -> String {
    format!("/dav/principals/{}/", u.id)
}
fn home_href(u: &DavUser) -> String {
    format!("/dav/calendars/{}/", u.id)
}
fn cal_href(u: &DavUser, k: CalKind) -> String {
    format!("/dav/calendars/{}/{}/", u.id, k.key())
}
fn obj_href(u: &DavUser, k: CalKind, name: &str) -> String {
    format!("{}{}", cal_href(u, k), encode(name))
}

// ---------------------------------------------------------------------------
// XML
// ---------------------------------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn status_line(code: StatusCode) -> String {
    format!("HTTP/1.1 {} {}", code.as_u16(), code.canonical_reason().unwrap_or(""))
}

/// Propriété demandée : (espace de noms, nom local).
type PropName = (String, String);

enum PropRequest {
    All,
    Names(Vec<PropName>),
}

fn parse_prop_request(body: &str) -> PropRequest {
    let Ok(doc) = roxmltree::Document::parse(body) else { return PropRequest::All };
    if doc.descendants().any(|n| n.tag_name().name() == "allprop" && n.tag_name().namespace() == Some(DAV_NS)) {
        return PropRequest::All;
    }
    let Some(prop) = doc.descendants().find(|n| n.tag_name().name() == "prop" && n.tag_name().namespace() == Some(DAV_NS)) else {
        return PropRequest::All;
    };
    PropRequest::Names(
        prop.children()
            .filter(|n| n.is_element())
            .map(|n| (n.tag_name().namespace().unwrap_or("").to_string(), n.tag_name().name().to_string()))
            .collect(),
    )
}

fn prefixed(ns: &str, name: &str, inner: Option<&str>) -> String {
    let (open, close) = match ns {
        DAV_NS => (format!("d:{name}"), format!("d:{name}")),
        CALDAV_NS => (format!("c:{name}"), format!("c:{name}")),
        CS_NS => (format!("cs:{name}"), format!("cs:{name}")),
        ICAL_NS => (format!("ical:{name}"), format!("ical:{name}")),
        other => (format!("x:{name} xmlns:x=\"{}\"", esc(other)), format!("x:{name}")),
    };
    match inner {
        Some(i) if !i.is_empty() => format!("<{open}>{i}</{close}>"),
        _ => format!("<{open}/>"),
    }
}

/// Ressource décrite dans une réponse multistatus.
enum Res<'a> {
    Root,
    Principal,
    Home,
    Calendar(&'a Calendar, &'a str),
    Object { etag: String, data: Option<String>, comp: &'static str },
}

fn prop_value(res: &Res, user: &DavUser, ns: &str, name: &str) -> Option<String> {
    let href = |h: String| format!("<d:href>{}</d:href>", esc(&h));
    let principal = href(principal_href(user));
    Some(match (ns, name) {
        (DAV_NS, "current-user-principal") => principal,
        (DAV_NS, "principal-URL") if !matches!(res, Res::Object { .. }) => principal,
        (DAV_NS, "owner") => principal,
        (CALDAV_NS, "calendar-home-set") if !matches!(res, Res::Object { .. }) => href(home_href(user)),
        (CALDAV_NS, "calendar-user-address-set") => format!("{}{}", href(format!("mailto:{}", user.email)), principal),
        (DAV_NS, "principal-collection-set") => href("/dav/principals/".into()),
        (DAV_NS, "displayname") => match res {
            Res::Root => "Looty".into(),
            Res::Principal => esc(&user.name),
            Res::Home => "Calendars".into(),
            Res::Calendar(c, _) => esc(&c.name),
            Res::Object { .. } => return None,
        },
        (DAV_NS, "resourcetype") => match res {
            Res::Root | Res::Home => "<d:collection/>".into(),
            Res::Principal => "<d:collection/><d:principal/>".into(),
            Res::Calendar(..) => "<d:collection/><c:calendar/>".into(),
            Res::Object { .. } => String::new(),
        },
        (DAV_NS, "current-user-privilege-set") => {
            let write = match res {
                Res::Calendar(c, _) => c.writable,
                Res::Object { .. } => true,
                _ => false,
            };
            let mut p = String::from("<d:privilege><d:read/></d:privilege><d:privilege><c:read-free-busy/></d:privilege>");
            if write {
                p.push_str("<d:privilege><d:write/></d:privilege><d:privilege><d:write-content/></d:privilege><d:privilege><d:write-properties/></d:privilege><d:privilege><d:bind/></d:privilege><d:privilege><d:unbind/></d:privilege>");
            }
            p
        }
        (DAV_NS, "supported-report-set") if matches!(res, Res::Calendar(..)) => {
            "<d:supported-report><d:report><c:calendar-multiget/></d:report></d:supported-report><d:supported-report><d:report><c:calendar-query/></d:report></d:supported-report>".into()
        }
        (CALDAV_NS, "supported-calendar-component-set") => match res {
            Res::Calendar(c, _) => format!(r#"<c:comp name="{}"/>"#, c.kind.component()),
            _ => return None,
        },
        (CALDAV_NS, "supported-calendar-data") if matches!(res, Res::Calendar(..)) => r#"<c:calendar-data content-type="text/calendar" version="2.0"/>"#.into(),
        (CALDAV_NS, "calendar-description") => match res {
            Res::Calendar(c, _) => esc(&c.description),
            _ => return None,
        },
        (CS_NS, "getctag") => match res {
            Res::Calendar(_, ctag) => esc(ctag),
            _ => return None,
        },
        (ICAL_NS, "calendar-color") if matches!(res, Res::Calendar(..)) => "#7170FFFF".into(),
        (ICAL_NS, "calendar-order") => match res {
            Res::Calendar(c, _) => c.order.to_string(),
            _ => return None,
        },
        (DAV_NS, "getetag") => match res {
            Res::Object { etag, .. } => esc(etag),
            _ => return None,
        },
        (DAV_NS, "getcontenttype") => match res {
            Res::Object { comp, .. } => format!("text/calendar; charset=utf-8; component={comp}"),
            _ => return None,
        },
        (CALDAV_NS, "calendar-data") => match res {
            Res::Object { data: Some(d), .. } => esc(d),
            _ => return None,
        },
        _ => return None,
    })
}

const ALL_PROPS: &[(&str, &str)] = &[
    (DAV_NS, "displayname"),
    (DAV_NS, "resourcetype"),
    (DAV_NS, "current-user-principal"),
    (DAV_NS, "owner"),
    (DAV_NS, "getetag"),
    (DAV_NS, "getcontenttype"),
    (CALDAV_NS, "calendar-home-set"),
    (CALDAV_NS, "supported-calendar-component-set"),
    (CALDAV_NS, "calendar-description"),
    (CS_NS, "getctag"),
];

fn response_xml(href: &str, res: &Res, user: &DavUser, req: &PropRequest) -> String {
    let mut ok = String::new();
    let mut missing = String::new();
    match req {
        PropRequest::All => {
            for (ns, name) in ALL_PROPS {
                if let Some(v) = prop_value(res, user, ns, name) {
                    ok.push_str(&prefixed(ns, name, Some(&v)));
                }
            }
        }
        PropRequest::Names(names) => {
            for (ns, name) in names {
                match prop_value(res, user, ns, name) {
                    Some(v) => ok.push_str(&prefixed(ns, name, Some(&v))),
                    None => missing.push_str(&prefixed(ns, name, None)),
                }
            }
        }
    }
    let mut out = format!("<d:response><d:href>{}</d:href>", esc(href));
    if !ok.is_empty() || missing.is_empty() {
        out.push_str(&format!("<d:propstat><d:prop>{ok}</d:prop><d:status>{}</d:status></d:propstat>", status_line(StatusCode::OK)));
    }
    if !missing.is_empty() {
        out.push_str(&format!("<d:propstat><d:prop>{missing}</d:prop><d:status>{}</d:status></d:propstat>", status_line(StatusCode::NOT_FOUND)));
    }
    out.push_str("</d:response>");
    out
}

fn multistatus(responses: Vec<String>) -> Response {
    let body = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<d:multistatus {NS}>{}</d:multistatus>", responses.concat());
    Response::builder()
        .status(StatusCode::MULTI_STATUS)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(Body::from(body))
        .unwrap()
}

fn status(code: StatusCode) -> Response {
    (code, code.canonical_reason().unwrap_or("")).into_response()
}

fn error_response(e: ApiError) -> Response {
    match e {
        ApiError::Forbidden => status(StatusCode::FORBIDDEN),
        ApiError::NotFound => status(StatusCode::NOT_FOUND),
        ApiError::Unauthorized => unauthorized(),
        ApiError::BadRequest(_) => status(StatusCode::BAD_REQUEST),
        other => {
            tracing::warn!("caldav server: {other:?}");
            status(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ---------------------------------------------------------------------------
// Gestionnaire
// ---------------------------------------------------------------------------

async fn handle(State(state): State<AppState>, method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
    if method == Method::OPTIONS {
        return dav_headers(Response::builder().status(StatusCode::OK).body(Body::empty()).unwrap());
    }
    let Some(user) = authenticate(&state, &headers).await else { return unauthorized() };
    let body = String::from_utf8_lossy(&body).into_owned();
    let res = match parse_target(uri.path(), &user) {
        None => Ok(status(StatusCode::NOT_FOUND)),
        Some(target) => match method.as_str() {
            "PROPFIND" => propfind(&state, &user, target, &headers, &body).await,
            "REPORT" => report(&state, &user, target, &body).await,
            "GET" | "HEAD" => get(&state, &user, target, method == Method::HEAD).await,
            "PUT" => put(&state, &user, target, &headers, &body).await,
            "DELETE" => delete(&state, &user, target, &headers).await,
            "PROPPATCH" => Ok(proppatch(uri.path(), &body)),
            "MKCALENDAR" | "MKCOL" | "MOVE" | "COPY" | "LOCK" | "UNLOCK" => Ok(status(StatusCode::FORBIDDEN)),
            _ => Ok(status(StatusCode::METHOD_NOT_ALLOWED)),
        },
    };
    dav_headers(res.unwrap_or_else(error_response))
}

fn dav_headers(mut r: Response) -> Response {
    let h = r.headers_mut();
    h.insert("DAV", HeaderValue::from_static("1, 3, calendar-access"));
    h.insert(header::ALLOW, HeaderValue::from_static("OPTIONS, GET, HEAD, PUT, DELETE, PROPFIND, PROPPATCH, REPORT"));
    r
}

async fn ctag(state: &AppState, user: &DavUser, kind: CalKind) -> Result<(String, Vec<(Obj, String, String)>), ApiError> {
    let list = objects(state, user, kind).await?;
    let mut h = Sha256::new();
    let mut out = Vec::with_capacity(list.len());
    for it in list {
        let ics = it.ics(state);
        let etag = etag_of(&ics);
        h.update(it.name().as_bytes());
        h.update(etag.as_bytes());
        out.push((it, ics, etag));
    }
    Ok((format!("\"{}\"", &hex::encode(h.finalize())[..32]), out))
}

async fn propfind(state: &AppState, user: &DavUser, target: Target, headers: &HeaderMap, body: &str) -> Result<Response, ApiError> {
    let depth1 = headers.get("Depth").and_then(|v| v.to_str().ok()).map(|d| d != "0").unwrap_or(false);
    let req = if body.trim().is_empty() { PropRequest::All } else { parse_prop_request(body) };
    let mut out = Vec::new();
    match target {
        Target::Root => {
            out.push(response_xml("/dav/", &Res::Root, user, &req));
            if depth1 {
                out.push(response_xml(&principal_href(user), &Res::Principal, user, &req));
            }
        }
        Target::Principal => out.push(response_xml(&principal_href(user), &Res::Principal, user, &req)),
        Target::Home => {
            out.push(response_xml(&home_href(user), &Res::Home, user, &req));
            if depth1 {
                for cal in calendars(state, user).await? {
                    let (tag, _) = ctag(state, user, cal.kind).await?;
                    out.push(response_xml(&cal_href(user, cal.kind), &Res::Calendar(&cal, &tag), user, &req));
                }
            }
        }
        Target::Calendar(kind) => {
            let cals = calendars(state, user).await?;
            let Some(cal) = cals.iter().find(|c| c.kind == kind) else { return Ok(status(StatusCode::NOT_FOUND)) };
            let (tag, list) = ctag(state, user, kind).await?;
            out.push(response_xml(&cal_href(user, kind), &Res::Calendar(cal, &tag), user, &req));
            if depth1 {
                for (it, _ics, etag) in list {
                    out.push(response_xml(&obj_href(user, kind, &it.name()), &Res::Object { etag, data: None, comp: kind.component() }, user, &req));
                }
            }
        }
        Target::Object(kind, name) => {
            let Some(it) = find_obj(state, user, kind, &name).await? else { return Ok(status(StatusCode::NOT_FOUND)) };
            let etag = etag_of(&it.ics(state));
            out.push(response_xml(&obj_href(user, kind, &name), &Res::Object { etag, data: None, comp: kind.component() }, user, &req));
        }
    }
    Ok(multistatus(out))
}

async fn report(state: &AppState, user: &DavUser, target: Target, body: &str) -> Result<Response, ApiError> {
    let kind = match target {
        Target::Calendar(k) | Target::Object(k, _) => k,
        _ => return Ok(status(StatusCode::FORBIDDEN)),
    };
    let Ok(doc) = roxmltree::Document::parse(body) else { return Ok(status(StatusCode::BAD_REQUEST)) };
    let root = doc.root_element();
    let req = parse_prop_request(body);
    let (_, list) = ctag(state, user, kind).await?;
    let with_data = |etag: &str, ics: &str| Res::Object { etag: etag.to_string(), data: Some(ics.to_string()), comp: kind.component() };
    match (root.tag_name().namespace(), root.tag_name().name()) {
        (Some(CALDAV_NS), "calendar-multiget") => {
            let mut out = Vec::new();
            for h in root.descendants().filter(|n| n.tag_name().name() == "href" && n.tag_name().namespace() == Some(DAV_NS)) {
                let raw = h.text().unwrap_or("").trim();
                // Les clients peuvent envoyer une URL absolue.
                let path = raw.find("/dav/").map(|i| &raw[i..]).unwrap_or(raw);
                let name = decode(path.rsplit('/').next().unwrap_or(""));
                match list.iter().find(|(it, _, _)| it.name() == name) {
                    Some((_, ics, etag)) => out.push(response_xml(path, &with_data(etag, ics), user, &req)),
                    None => out.push(format!("<d:response><d:href>{}</d:href><d:status>{}</d:status></d:response>", esc(path), status_line(StatusCode::NOT_FOUND))),
                }
            }
            Ok(multistatus(out))
        }
        (Some(CALDAV_NS), "calendar-query") => {
            // Seul le filtre de composant est pris en compte : un calendrier ne contient qu'un
            // type de composant (VTODO pour les tâches, VEVENT pour les événements).
            let own = kind.component();
            let comps: Vec<String> = root
                .descendants()
                .filter(|n| n.tag_name().name() == "comp-filter")
                .filter_map(|n| n.attribute("name").map(|s| s.to_ascii_uppercase()))
                .collect();
            if comps.iter().any(|c| c != "VCALENDAR" && c != own) && !comps.iter().any(|c| c == own) {
                return Ok(multistatus(vec![]));
            }
            Ok(multistatus(list.iter().map(|(it, ics, etag)| response_xml(&obj_href(user, kind, &it.name()), &with_data(etag, ics), user, &req)).collect()))
        }
        _ => Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(Body::from(format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><d:error {NS}><d:supported-report/></d:error>")))
            .unwrap()),
    }
}

async fn get(state: &AppState, user: &DavUser, target: Target, head: bool) -> Result<Response, ApiError> {
    let Target::Object(kind, name) = target else {
        return Ok(status(StatusCode::METHOD_NOT_ALLOWED));
    };
    let Some(it) = find_obj(state, user, kind, &name).await? else { return Ok(status(StatusCode::NOT_FOUND)) };
    let ics = it.ics(state);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .header(header::ETAG, etag_of(&ics))
        .body(if head { Body::empty() } else { Body::from(ics) })
        .unwrap())
}

fn precondition(headers: &HeaderMap, current: Option<&str>) -> bool {
    if let Some(m) = headers.get(header::IF_MATCH).and_then(|v| v.to_str().ok()) {
        let ok = match current {
            Some(e) => m.trim() == "*" || m.split(',').any(|x| x.trim() == e),
            None => false,
        };
        if !ok {
            return false;
        }
    }
    if let Some(m) = headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) {
        if current.is_some() && (m.trim() == "*" || current.is_some_and(|e| m.split(',').any(|x| x.trim() == e))) {
            return false;
        }
    }
    true
}

/// Champs modifiés par le client par rapport à l'état actuel de la tâche.
fn changes(remote: &TodoFields, local: &TodoFields) -> RemoteChanges {
    let mut c = RemoteChanges::default();
    if remote.summary.trim() != local.summary && !remote.summary.trim().is_empty() {
        c.title = Some(remote.summary.trim().to_string());
    }
    if remote.description.trim() != local.description.trim() {
        let html = ical::text_to_html(&remote.description);
        c.description = Some(if html.is_empty() { None } else { Some(html) });
    }
    if remote.due != local.due {
        c.due = Some(remote.due);
    }
    if remote.start != local.start {
        c.start = Some(remote.start);
    }
    if remote.priority != local.priority {
        c.priority = Some(remote.priority);
    }
    if remote.completed != local.completed {
        c.completed = Some(remote.completed);
    }
    c
}

async fn put(state: &AppState, user: &DavUser, target: Target, headers: &HeaderMap, body: &str) -> Result<Response, ApiError> {
    let Target::Object(kind, name) = target else {
        return Ok(status(StatusCode::METHOD_NOT_ALLOWED));
    };
    if name.len() > 255 || name.contains('/') {
        return Ok(status(StatusCode::BAD_REQUEST));
    }
    if let Some((ws, cal, role)) = event_scope(state, user, kind).await? {
        return put_event(state, role, user.id, ws, cal, &name, headers, body).await;
    }
    let Some(parsed) = ical::parse_vtodo(body) else {
        // Seuls les VTODO sont acceptés (supported-calendar-component-set).
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(Body::from(format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><d:error {NS}><c:supported-calendar-component/></d:error>")))
            .unwrap());
    };
    let existing = find_item(state, user, kind, &name).await?;
    let current = existing.as_ref().map(|it| etag_of(&it.ics(state)));
    if !precondition(headers, current.as_deref()) {
        return Ok(status(StatusCode::PRECONDITION_FAILED));
    }
    match existing {
        Some(it) => {
            let ch = changes(&parsed.fields, &it.fields());
            sync_apply_remote(state, it.id, user.id, ch).await?;
            // Contenu normalisé par le serveur : pas d'ETag, le client relira la ressource.
            Ok(status(StatusCode::NO_CONTENT))
        }
        None => {
            let CalKind::Board(board_id) = kind else {
                // « Assignées à moi » regroupe plusieurs tableaux : on ne sait pas où créer.
                return Ok(status(StatusCode::FORBIDDEN));
            };
            if sqlx::query_scalar::<_, i64>("SELECT count(*) FROM dav_task_resources WHERE name = $1").bind(&name).fetch_one(&state.pool).await? > 0 {
                return Ok(status(StatusCode::CONFLICT));
            }
            let task_id = dav_create_task(state, user.id, board_id, &parsed.fields).await?;
            let uid = parsed.uid.filter(|u| !u.trim().is_empty() && u.len() <= 255).unwrap_or_else(|| Uuid::new_v4().to_string());
            sqlx::query("INSERT INTO dav_task_resources (task_id, uid, name) VALUES ($1, $2, $3)").bind(task_id).bind(&uid).bind(&name).execute(&state.pool).await?;
            Ok(status(StatusCode::CREATED))
        }
    }
}

async fn delete(state: &AppState, user: &DavUser, target: Target, headers: &HeaderMap) -> Result<Response, ApiError> {
    let Target::Object(kind, name) = target else {
        return Ok(status(StatusCode::FORBIDDEN));
    };
    if let Some((ws, cal, role)) = event_scope(state, user, kind).await? {
        if !role.can_write() {
            return Err(ApiError::Forbidden);
        }
        let Some(e) = crate::events::find_by_name(&state.pool, ws, &name, cal).await? else { return Ok(status(StatusCode::NOT_FOUND)) };
        if !precondition(headers, Some(&etag_of(&e.dav_ics()))) {
            return Ok(status(StatusCode::PRECONDITION_FAILED));
        }
        crate::events::delete_row(state, &e).await?;
        return Ok(status(StatusCode::NO_CONTENT));
    }
    let Some(it) = find_item(state, user, kind, &name).await? else { return Ok(status(StatusCode::NOT_FOUND)) };
    if !precondition(headers, Some(&etag_of(&it.ics(state)))) {
        return Ok(status(StatusCode::PRECONDITION_FAILED));
    }
    match kind {
        CalKind::Events(_) | CalKind::Cal(_) => unreachable!(),
        // Dans « Assignées à moi », supprimer = ne plus être assigné ; la tâche reste.
        CalKind::Assigned => dav_unassign(state, it.id, user.id).await?,
        CalKind::Board(_) => {
            let role = crate::access::workspace_role(&state.pool, user.id, it.workspace_id).await?.ok_or(ApiError::Forbidden)?;
            if !role.can_write() {
                return Err(ApiError::Forbidden);
            }
            remove_task(state, it.id).await?;
        }
    }
    Ok(status(StatusCode::NO_CONTENT))
}

/// Événement envoyé par un client : l'objet complet remplace l'existant.
async fn put_event(state: &AppState, role: Role, user_id: Uuid, ws: Uuid, cal: Option<Uuid>, name: &str, headers: &HeaderMap, body: &str) -> Result<Response, ApiError> {
    if !role.can_write() {
        return Err(ApiError::Forbidden);
    }
    if body.len() > 512 * 1024 || !body.to_ascii_uppercase().contains("BEGIN:VEVENT") || crate::calendar::master_event(body).is_none() {
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(Body::from(format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><d:error {NS}><c:supported-calendar-component/></d:error>")))
            .unwrap());
    }
    let existing = crate::events::find_by_name(&state.pool, ws, name, cal).await?;
    if !precondition(headers, existing.as_ref().map(|e| etag_of(&e.dav_ics())).as_deref()) {
        return Ok(status(StatusCode::PRECONDITION_FAILED));
    }
    let created = existing.is_none();
    match crate::events::save_from_dav(state, ws, user_id, name, body, existing.as_ref(), cal).await {
        Err(ApiError::Conflict(_)) => return Ok(status(StatusCode::CONFLICT)),
        other => other?,
    }
    // Contenu normalisé (lignes repliées) : pas d'ETag, le client relira la ressource.
    Ok(status(if created { StatusCode::CREATED } else { StatusCode::NO_CONTENT }))
}

/// Les propriétés des calendriers (couleur, nom…) sont gérées dans Looty.
fn proppatch(path: &str, body: &str) -> Response {
    let mut props = String::new();
    if let Ok(doc) = roxmltree::Document::parse(body) {
        for p in doc.descendants().filter(|n| n.tag_name().name() == "prop" && n.tag_name().namespace() == Some(DAV_NS)) {
            for c in p.children().filter(|n| n.is_element()) {
                props.push_str(&prefixed(c.tag_name().namespace().unwrap_or(""), c.tag_name().name(), None));
            }
        }
    }
    multistatus(vec![format!(
        "<d:response><d:href>{}</d:href><d:propstat><d:prop>{props}</d:prop><d:status>{}</d:status></d:propstat></d:response>",
        esc(path),
        status_line(StatusCode::FORBIDDEN)
    )])
}
