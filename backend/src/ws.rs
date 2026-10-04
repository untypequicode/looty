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

use crate::access::{require_member, AuthUser};
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
                    UserPresence { user_id: user_id.to_string(), location: "home".into(), updated_at: now, status: active(), since: now }
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

    async fn set_location(&self, workspace_id: Uuid, user_id: Uuid, location: String, status: String) {
        let now = chrono::Utc::now().timestamp_millis();
        let presence = {
            let mut all = self.presence.write().await;
            let room = all.entry(workspace_id).or_default();
            // « Depuis » ne change que si l'endroit change (pas pour actif / inactif).
            let since = room.get(&user_id).filter(|p| p.location == location).map(|p| p.since).unwrap_or(now);
            let p = UserPresence { user_id: user_id.to_string(), location, updated_at: now, status, since };
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
    require_member(&state.pool, &user, query.workspace_id).await?;
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, query.workspace_id, user.id)))
}

fn sanitize_location(raw: &str) -> String {
    // Format attendu : un écran ("home", "calendar"…), "hidden", ou "<kind>:<uuid>"
    const SCREENS: &[&str] = &["home", "hidden", "calendar", "mail", "inbox", "my-tasks", "docs", "trash", "settings", "activity"];
    let valid = SCREENS.contains(&raw)
        || raw
            .split_once(':')
            .map(|(kind, id)| {
                matches!(kind, "page" | "board" | "task" | "settings") && Uuid::parse_str(id).is_ok()
            })
            .unwrap_or(false);
    if valid { raw.to_string() } else { "home".into() }
}

async fn handle_socket(socket: WebSocket, state: AppState, workspace_id: Uuid, user_id: Uuid) {
    let hub = state.hub.clone();
    let (mut sender, mut receiver) = socket.split();
    let mut rx = hub.room(workspace_id).await.subscribe();

    hub.connected(workspace_id, user_id).await;
    let snapshot = serde_json::json!({ "type": "PRESENCE_SYNC", "presence": hub.presence_list(workspace_id).await });
    let _ = sender.send(Message::Text(snapshot.to_string())).await;

    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(env) => {
                    if env.to.is_some_and(|to| to != user_id) {
                        continue;
                    }
                    if sender.send(Message::Text(env.payload.to_string())).await.is_err() {
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
                    hub_rx.set_location(workspace_id, user_id, loc, status.into()).await;
                }
                Some("USER_MENTIONED") => {
                    // L'auteur est imposé par le serveur ; la cible doit être membre.
                    let Some(target) = json["target_user_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) else {
                        continue;
                    };
                    if target == user_id {
                        continue;
                    }
                    let ok = crate::access::is_member(&pool, workspace_id, &target.to_string())
                        .await
                        .unwrap_or(false);
                    if !ok {
                        continue;
                    }
                    let loc = sanitize_location(json["location"].as_str().unwrap_or("home"));
                    crate::notifications::notify(
                        &pool,
                        &hub_rx,
                        workspace_id,
                        target,
                        Some(user_id),
                        "mention",
                        &loc,
                        serde_json::json!({}),
                    )
                    .await;
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
