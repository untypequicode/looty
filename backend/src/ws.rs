use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::access::{
    effective_page_access, level, member_perms, nearest_page_override, require_member_or_guest, visible_pages, AuthUser, PageAccess, Perms, Role,
};
use sqlx::PgPool;
use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct WsQuery {
    pub workspace_id: Uuid,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UserPresence {
    pub user_id: String,
    pub location: String, // "page:<uuid>", "board:<uuid>", "task:<uuid>", "home", "calendar", "hidden"...
    pub updated_at: i64,
    /// « active » ou « idle » (onglet en arrière-plan / inactif).
    #[serde(default = "active")]
    pub status: String,
    /// Depuis quand la personne est à cet endroit (ms).
    #[serde(default)]
    pub since: i64,
    /// Membre que la personne suit (son écran suit le sien) : affiché chez la personne suivie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub following: Option<String>,
}

fn active() -> String {
    "active".into()
}

/// Message diffusé dans une room. `to` restreint la livraison à un seul utilisateur.
#[derive(Clone, Debug)]
pub struct Envelope {
    pub to: Option<Uuid>,
    pub payload: Arc<str>,
}

/// Hub temps réel : une room par workspace.
pub struct Hub {
    rooms: RwLock<HashMap<Uuid, broadcast::Sender<Envelope>>>,
    presence: RwLock<HashMap<Uuid, HashMap<Uuid, UserPresence>>>,
    connections: RwLock<HashMap<Uuid, HashMap<Uuid, usize>>>,
}

impl Hub {
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
            presence: RwLock::new(HashMap::new()),
            connections: RwLock::new(HashMap::new()),
        }
    }

    async fn room(&self, workspace_id: Uuid) -> broadcast::Sender<Envelope> {
        if let Some(tx) = self.rooms.read().await.get(&workspace_id) {
            return tx.clone();
        }
        let mut rooms = self.rooms.write().await;
        rooms
            .entry(workspace_id)
            .or_insert_with(|| broadcast::channel(512).0)
            .clone()
    }

    pub async fn broadcast(&self, workspace_id: Uuid, event: serde_json::Value) {
        let tx = self.room(workspace_id).await;
        let _ = tx.send(Envelope { to: None, payload: event.to_string().into() });
    }

    pub async fn send_to(&self, workspace_id: Uuid, user_id: Uuid, event: serde_json::Value) {
        let tx = self.room(workspace_id).await;
        let _ = tx.send(Envelope { to: Some(user_id), payload: event.to_string().into() });
    }

    async fn presence_list(&self, workspace_id: Uuid) -> Vec<UserPresence> {
        self.presence
            .read()
            .await
            .get(&workspace_id)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    async fn connected(&self, workspace_id: Uuid, user_id: Uuid) {
        *self
            .connections
            .write()
            .await
            .entry(workspace_id)
            .or_default()
            .entry(user_id)
            .or_insert(0) += 1;

        let presence = {
            let mut p = self.presence.write().await;
            p.entry(workspace_id)
                .or_default()
                .entry(user_id)
                .or_insert_with(|| {
                    let now = chrono::Utc::now().timestamp_millis();
                    UserPresence { user_id: user_id.to_string(), location: "home".into(), updated_at: now, status: active(), since: now, following: None }
                })
                .clone()
        };
        self.broadcast(workspace_id, serde_json::json!({ "type": "USER_PRESENCE_UPDATE", "presence": presence }))
            .await;
    }

    async fn disconnected(&self, workspace_id: Uuid, user_id: Uuid) {
        let is_last = {
            let mut conns = self.connections.write().await;
            let m = conns.entry(workspace_id).or_default();
            let remaining = m.get_mut(&user_id).map(|c| {
                *c = c.saturating_sub(1);
                *c
            });
            if remaining.unwrap_or(0) == 0 {
                m.remove(&user_id);
                true
            } else {
                false
            }
        };
        if is_last {
            if let Some(m) = self.presence.write().await.get_mut(&workspace_id) {
                m.remove(&user_id);
            }
            self.broadcast(workspace_id, serde_json::json!({ "type": "USER_OFFLINE", "user_id": user_id }))
                .await;
        }
    }

    async fn set_location(&self, workspace_id: Uuid, user_id: Uuid, location: String, status: String, following: Option<Uuid>) {
        let now = chrono::Utc::now().timestamp_millis();
        let presence = {
            let mut all = self.presence.write().await;
            let room = all.entry(workspace_id).or_default();
            // « Depuis » ne change que si l'endroit change (pas pour actif / inactif).
            let since = room.get(&user_id).filter(|p| p.location == location).map(|p| p.since).unwrap_or(now);
            // On ne suit que quelqu'un de connecté à cet espace (et jamais soi-même).
            let following = following.filter(|f| *f != user_id && room.contains_key(f)).map(|f| f.to_string());
            let p = UserPresence { user_id: user_id.to_string(), location, updated_at: now, status, since, following };
            room.insert(user_id, p.clone());
            p
        };
        self.broadcast(workspace_id, serde_json::json!({ "type": "USER_PRESENCE_UPDATE", "presence": presence }))
            .await;
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<WsQuery>,
) -> ApiResult<impl IntoResponse> {
    // Vérifie l'appartenance AVANT l'upgrade : impossible d'écouter un workspace étranger.
    // Invité (pages partagées) : seulement la présence, limitée à ses pages.
    let guest_pages = if require_member_or_guest(&state.pool, &user, query.workspace_id).await? == Role::Guest {
        let pages = visible_pages(&state.pool, &user, query.workspace_id, Role::Guest).await?;
        Some(std::sync::Arc::new(pages.into_iter().filter(|(_, a)| *a != PageAccess::None).map(|(id, _)| id).collect::<std::collections::HashSet<_>>()))
    } else {
        None
    };
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, query.workspace_id, user.id, guest_pages)))
}

fn sanitize_location(raw: &str) -> String {
    // Format attendu : un écran ("home", "calendar"…), "hidden", ou "<kind>:<uuid>"
    const SCREENS: &[&str] = &["home", "hidden", "calendar", "mail", "moodle", "projects", "drive", "inbox", "my-tasks", "docs", "trash", "settings", "activity", "graph"];
    let valid = SCREENS.contains(&raw)
        || raw
            .split_once(':')
            .map(|(kind, id)| {
                matches!(kind, "page" | "board" | "task" | "settings") && Uuid::parse_str(id).is_ok()
            })
            .unwrap_or(false);
    if valid { raw.to_string() } else { "home".into() }
}

/// Ce qu'un invité reçoit d'un message de l'espace : la présence seulement, sans montrer où
/// sont les membres hors de ses pages partagées. `None` : message non transmis.
fn guest_view(payload: &str, pages: &std::collections::HashSet<Uuid>, guest: Uuid) -> Option<String> {
    let mut v: serde_json::Value = serde_json::from_str(payload).ok()?;
    let clean = |p: &mut serde_json::Value| {
        let loc = p["location"].as_str().unwrap_or("home").to_string();
        let visible = match loc.split_once(':') {
            Some(("page", id)) => Uuid::parse_str(id).is_ok_and(|id| pages.contains(&id)),
            Some(_) => false,
            None => matches!(loc.as_str(), "home" | "docs" | "hidden"),
        };
        if !visible {
            p["location"] = "hidden".into();
        }
        if p["following"].as_str() != Some(guest.to_string().as_str()) {
            if let Some(o) = p.as_object_mut() {
                o.remove("following");
            }
        }
    };
    match v["type"].as_str()? {
        "USER_PRESENCE_UPDATE" => clean(&mut v["presence"]),
        "PRESENCE_SYNC" => {
            for p in v["presence"].as_array_mut()? {
                clean(p);
            }
        }
        "USER_OFFLINE" => {}
        _ => return None,
    }
    Some(v.to_string())
}

/// Droits d'un membre connecté, pour filtrer ce que la room lui transmet. Recalculés quand
/// ses droits changent ; `None` : il n'est plus membre (la connexion est fermée).
struct MemberView {
    perms: Perms,
    /// Accès aux pages (vide pour un admin : il voit tout).
    pages: HashMap<Uuid, PageAccess>,
}

async fn member_view(pool: &PgPool, user_id: Uuid, workspace_id: Uuid) -> Option<MemberView> {
    let (_, perms) = member_perms(pool, user_id, workspace_id).await.ok()??;
    let user = AuthUser { id: user_id, is_super_admin: false };
    let pages = if perms.admin { HashMap::new() } else { visible_pages(pool, &user, workspace_id, Role::Member).await.ok()? };
    Some(MemberView { perms, pages })
}

impl MemberView {
    fn page(&self, id: Uuid) -> PageAccess {
        if self.perms.admin { PageAccess::Write } else { self.pages.get(&id).copied().unwrap_or(PageAccess::None) }
    }
}

/// Message transmis à un membre : rien sur les tableaux sans accès aux tableaux, rien sur
/// les pages qu'il ne peut pas lire, et le lien public d'une page seulement s'il peut la modifier.
fn member_allows(view: &MemberView, v: &mut serde_json::Value) -> bool {
    let kind = v["type"].as_str().unwrap_or_default();
    let boards = kind.starts_with("TASK_")
        || kind.starts_with("COLUMN_")
        || kind.starts_with("BOARD_")
        || kind.starts_with("LABEL_")
        || matches!(kind, "COMMENT_CREATED" | "COMMENT_UPDATED" | "COMMENT_DELETED");
    if boards {
        return view.perms.boards > level::NONE;
    }
    if kind == "CALENDAR_EVENTS_UPDATED" {
        return view.perms.calendar > level::NONE;
    }
    if matches!(kind, "PAGE_CREATED" | "PAGE_UPDATED") {
        let Some(id) = v["page"]["id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) else { return false };
        return match view.page(id) {
            PageAccess::None => false,
            PageAccess::Read => {
                if let Some(p) = v["page"].as_object_mut() {
                    p.remove("share_token");
                }
                true
            }
            PageAccess::Write => true,
        };
    }
    true
}

/// Compte actif, et toujours membre ou invité de l'espace.
async fn still_allowed(pool: &PgPool, user_id: Uuid, workspace_id: Uuid) -> bool {
    let active: Option<bool> = sqlx::query_scalar("SELECT NOT is_suspended FROM users WHERE id = $1").bind(user_id).fetch_optional(pool).await.ok().flatten();
    if active != Some(true) {
        return false;
    }
    let user = AuthUser { id: user_id, is_super_admin: false };
    require_member_or_guest(pool, &user, workspace_id).await.is_ok()
}

async fn handle_socket(socket: WebSocket, state: AppState, workspace_id: Uuid, user_id: Uuid, guest_pages: Option<std::sync::Arc<std::collections::HashSet<Uuid>>>) {
    let hub = state.hub.clone();
    let (mut sender, mut receiver) = socket.split();
    let mut rx = hub.room(workspace_id).await.subscribe();
    let mut view = match guest_pages {
        Some(_) => None,
        None => match member_view(&state.pool, user_id, workspace_id).await {
            Some(v) => Some(v),
            None => return,
        },
    };
    let send_pool = state.pool.clone();

    hub.connected(workspace_id, user_id).await;
    crate::analytics::track(&state.pool, workspace_id, user_id, "session", None).await;
    let snapshot = serde_json::json!({ "type": "PRESENCE_SYNC", "presence": hub.presence_list(workspace_id).await }).to_string();
    let snapshot = match &guest_pages {
        Some(pages) => guest_view(&snapshot, pages, user_id).unwrap_or_default(),
        None => snapshot,
    };
    let _ = sender.send(Message::Text(snapshot.into())).await;

    let mut send_task = tokio::spawn(async move {
        // Vérification régulière : compte suspendu, accès à l'espace perdu (invité dont les partages
        // ont été retirés) : la connexion est fermée.
        let mut check = tokio::time::interval(std::time::Duration::from_secs(120));
        check.tick().await;
        loop {
            let received = tokio::select! {
                r = rx.recv() => r,
                _ = check.tick() => {
                    if !still_allowed(&send_pool, user_id, workspace_id).await {
                        break;
                    }
                    if let Some(mv) = view.as_mut() {
                        match member_view(&send_pool, user_id, workspace_id).await {
                            Some(fresh) => *mv = fresh,
                            None => break,
                        }
                    }
                    continue;
                }
            };
            match received {
                Ok(env) => {
                    if env.to.is_some_and(|to| to != user_id) {
                        continue;
                    }
                    let text = match (&guest_pages, view.as_mut()) {
                        // Invité : messages qui lui sont adressés, et la présence filtrée.
                        (Some(pages), _) if env.to.is_none() => match guest_view(&env.payload, pages, user_id) {
                            Some(t) => t,
                            None => continue,
                        },
                        (None, Some(mv)) if env.to.is_none() => {
                            let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&env.payload) else { continue };
                            match v["type"].as_str().unwrap_or_default() {
                                // Rôle ou membres modifiés : droits recalculés ; retiré de l'espace : fin.
                                "MEMBERS_UPDATED" => {
                                    if v["removed"] == true && v["user_id"].as_str() == Some(user_id.to_string().as_str()) {
                                        let _ = sender.send(Message::Text(env.payload.to_string().into())).await;
                                        break;
                                    }
                                    match member_view(&send_pool, user_id, workspace_id).await {
                                        Some(fresh) => *mv = fresh,
                                        None => break,
                                    }
                                }
                                "PAGE_PERMISSIONS_UPDATED" if !mv.perms.admin => {
                                    if let Some(fresh) = member_view(&send_pool, user_id, workspace_id).await {
                                        *mv = fresh;
                                    }
                                }
                                // Page nouvelle ou déplacée : son accès (hérité du parent) peut changer.
                                "PAGE_CREATED" | "PAGE_UPDATED" if !mv.perms.admin => {
                                    if let Some(id) = v["page"]["id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) {
                                        let ov = nearest_page_override(&send_pool, id, user_id).await.ok().flatten();
                                        mv.pages.insert(id, effective_page_access(&mv.perms, ov.as_deref()));
                                    }
                                }
                                _ => {}
                            }
                            if !member_allows(mv, &mut v) {
                                continue;
                            }
                            v.to_string()
                        }
                        _ => env.payload.to_string(),
                    };
                    if sender.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    let hub_rx = hub.clone();
    let pool = state.pool.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            let text = match msg {
                Ok(Message::Text(t)) => t,
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => continue,
            };
            if text.len() > 4096 {
                continue;
            }
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            match json["type"].as_str() {
                Some("USER_LOCATION") => {
                    let loc = sanitize_location(json["location"].as_str().unwrap_or("home"));
                    let status = if json["status"].as_str() == Some("idle") { "idle" } else { "active" };
                    let following = json["following"].as_str().and_then(|s| Uuid::parse_str(s).ok());
                    hub_rx.set_location(workspace_id, user_id, loc, status.into(), following).await;
                }
                Some("USER_MENTIONED") => {
                    // L'auteur est imposé par le serveur ; la cible doit être membre.
                    let Some(target) = json["target_user_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) else {
                        continue;
                    };
                    // Un membre, ou un groupe de l'espace : chacun de ses membres est notifié.
                    let people = crate::groups::mentioned_people(&pool, workspace_id, &[target]).await.unwrap_or_default();
                    let loc = sanitize_location(json["location"].as_str().unwrap_or("home"));
                    for person in people {
                        crate::notifications::notify(
                            &pool,
                            &hub_rx,
                            workspace_id,
                            person,
                            Some(user_id),
                            "mention",
                            &loc,
                            serde_json::json!({}),
                        )
                        .await;
                    }
                }
                _ => {} // Tout autre message client est ignoré (pas de relais arbitraire).
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
    hub.disconnected(workspace_id, user_id).await;
}
