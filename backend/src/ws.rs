use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Query},
    response::IntoResponse,
    Extension,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};

#[derive(Deserialize)]
pub struct WsQuery {
    pub workspace_id: String,
}

// Global state for WebSockets, organized by workspace rooms
pub struct AppState {
    pub rooms: RwLock<HashMap<String, broadcast::Sender<String>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_or_create_room(&self, workspace_id: &str) -> broadcast::Sender<String> {
        let rooms = self.rooms.read().await;
        if let Some(tx) = rooms.get(workspace_id) {
            return tx.clone();
        }
        drop(rooms);

        let mut rooms = self.rooms.write().await;
        // Check again in case it was created while waiting for write lock
        if let Some(tx) = rooms.get(workspace_id) {
            return tx.clone();
        }
        let (tx, _rx) = broadcast::channel(100);
        rooms.insert(workspace_id.to_string(), tx.clone());
        tx
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, query.workspace_id, state))
}

async fn handle_socket(socket: WebSocket, workspace_id: String, state: Arc<AppState>) {
    println!(">>> NOUVELLE CONNEXION WS POUR WORKSPACE {} !", workspace_id);
    let (mut sender, mut receiver) = socket.split();

    let tx = state.get_or_create_room(&workspace_id).await;
    let mut rx = tx.subscribe();

    // Send messages from broadcast channel to this WS client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Receive messages from this WS client and send to broadcast channel
    let tx_clone = tx.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let _ = tx_clone.send(text);
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
    println!("<<< CONNEXION WS FERMÉE POUR WORKSPACE {}", workspace_id);
}
