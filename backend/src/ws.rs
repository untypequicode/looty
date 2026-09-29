use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Query},
    response::IntoResponse,
    Extension,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use tower_sessions::Session;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct WsQuery {
    pub workspace_id: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UserPresence {
    pub user_id: String,
    pub location: String, // e.g., "page:uuid", "board:uuid", "home"
    pub updated_at: i64,
}

// Global state for WebSockets
pub struct AppState {
    pub rooms: RwLock<HashMap<String, broadcast::Sender<String>>>,
    // workspace_id -> user_id -> UserPresence
    pub presence: RwLock<HashMap<String, HashMap<String, UserPresence>>>,
    // workspace_id -> user_id -> nombre de connexions ouvertes (onglets / reconnexions)
    pub connections: RwLock<HashMap<String, HashMap<String, usize>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
            presence: RwLock::new(HashMap::new()),
            connections: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_or_create_room(&self, workspace_id: &str) -> broadcast::Sender<String> {
        let rooms = self.rooms.read().await;
        if let Some(tx) = rooms.get(workspace_id) {
            return tx.clone();
        }
        drop(rooms);

        let mut rooms = self.rooms.write().await;
        if let Some(tx) = rooms.get(workspace_id) {
            return tx.clone();
        }
        let (tx, _rx) = broadcast::channel(256);
        rooms.insert(workspace_id.to_string(), tx.clone());
        tx
    }

    async fn presence_list(&self, workspace_id: &str) -> Vec<UserPresence> {
        let presence = self.presence.read().await;
        presence
            .get(workspace_id)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Enregistre une connexion et publie immédiatement la présence (location "home" par défaut).
    async fn user_connected(&self, workspace_id: &str, user_id: &str, tx: &broadcast::Sender<String>) {
        {
            let mut conns = self.connections.write().await;
            *conns
                .entry(workspace_id.to_string())
                .or_default()
                .entry(user_id.to_string())
                .or_insert(0) += 1;
        }

        let presence = {
            let mut p = self.presence.write().await;
            p.entry(workspace_id.to_string())
                .or_default()
                .entry(user_id.to_string())
                .or_insert_with(|| UserPresence {
                    user_id: user_id.to_string(),
                    location: "home".to_string(),
                    updated_at: chrono::Utc::now().timestamp_millis(),
                })
                .clone()
        };

        let _ = tx.send(
            serde_json::json!({ "type": "USER_PRESENCE_UPDATE", "presence": presence }).to_string(),
        );
    }

    /// Ne passe l'utilisateur hors ligne que lorsque sa dernière connexion se ferme.
    async fn user_disconnected(&self, workspace_id: &str, user_id: &str, tx: &broadcast::Sender<String>) {
        let is_last = {
            let mut conns = self.connections.write().await;
            let remaining = match conns.get_mut(workspace_id).and_then(|m| m.get_mut(user_id)) {
                Some(count) => {
                    *count = count.saturating_sub(1);
                    *count
                }
                None => 0,
            };
            if remaining == 0 {
                if let Some(m) = conns.get_mut(workspace_id) {
                    m.remove(user_id);
                }
            }
            remaining == 0
        };

        if is_last {
            {
                let mut p = self.presence.write().await;
                if let Some(ws_p) = p.get_mut(workspace_id) {
                    ws_p.remove(user_id);
                }
            }
            let _ = tx.send(
                serde_json::json!({ "type": "USER_OFFLINE", "user_id": user_id }).to_string(),
            );
        }
    }

    async fn set_location(
        &self,
        workspace_id: &str,
        user_id: &str,
        location: String,
        tx: &broadcast::Sender<String>,
    ) {
        let presence = UserPresence {
            user_id: user_id.to_string(),
            location,
            updated_at: chrono::Utc::now().timestamp_millis(),
        };
        {
            let mut p = self.presence.write().await;
            p.entry(workspace_id.to_string())
                .or_default()
                .insert(user_id.to_string(), presence.clone());
        }
        let _ = tx.send(
            serde_json::json!({ "type": "USER_PRESENCE_UPDATE", "presence": presence }).to_string(),
        );
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    Extension(state): Extension<Arc<AppState>>,
    session: Session,
) -> impl IntoResponse {
    // On lit la session AVANT l'upgrade (la requête HTTP est encore "vivante")
    let user_id = session
        .get::<Uuid>("user_id")
        .await
        .ok()
        .flatten()
        .map(|u| u.to_string())
        .unwrap_or_default();

    ws.on_upgrade(move |socket| handle_socket(socket, query.workspace_id, state, user_id))
}

async fn handle_socket(socket: WebSocket, workspace_id: String, state: Arc<AppState>, user_id: String) {
    println!(">>> NOUVELLE CONNEXION WS POUR WORKSPACE {} (User: '{}')", workspace_id, user_id);
    let (mut sender, mut receiver) = socket.split();

    let tx = state.get_or_create_room(&workspace_id).await;
    let mut rx = tx.subscribe();

    if !user_id.is_empty() {
        state.user_connected(&workspace_id, &user_id, &tx).await;

        // Snapshot complet (incluant soi-même) envoyé uniquement au nouvel arrivant
        let presence_list = state.presence_list(&workspace_id).await;
        let msg = serde_json::json!({
            "type": "PRESENCE_SYNC",
            "presence": presence_list
        })
        .to_string();
        let _ = sender.send(Message::Text(msg)).await;
    }

    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if sender.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    let tx_clone = tx.clone();
    let state_clone = state.clone();
    let workspace_id_clone = workspace_id.clone();
    let user_id_clone = user_id.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            let text = match msg {
                Ok(Message::Text(t)) => t,
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => continue, // ping / pong / binaire : on ignore sans couper la connexion
            };

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if json["type"] == "USER_LOCATION" {
                    if !user_id_clone.is_empty() {
                        let location = json["location"].as_str().unwrap_or("home").to_string();
                        state_clone
                            .set_location(&workspace_id_clone, &user_id_clone, location, &tx_clone)
                            .await;
                    }
                    continue;
                }
            }
            let _ = tx_clone.send(text);
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    if !user_id.is_empty() {
        state.user_disconnected(&workspace_id, &user_id, &tx).await;
    }

    println!("<<< CONNEXION WS FERMÉE POUR WORKSPACE {}", workspace_id);
}