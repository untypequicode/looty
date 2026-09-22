use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

// État global pour les WebSockets
pub struct AppState {
    // Un channel broadcast pour envoyer des messages à tout le monde
    pub tx: broadcast::Sender<String>,
    // Les utilisateurs connectés (juste pour tester)
    pub connected_users: Mutex<usize>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(100);
        Self {
            tx,
            connected_users: Mutex::new(0),
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    println!(">>> NOUVELLE CONNEXION WEBSOCKET REÇUE !");
    let (mut sender, mut receiver) = socket.split();

    // S'inscrire au channel broadcast
    let mut rx = state.tx.subscribe();

    {
        let mut users = state.connected_users.lock().await;
        *users += 1;
        let _ = state.tx.send(format!("Un utilisateur a rejoint. Total: {}", *users));
    }

    // Tâche pour envoyer les messages reçus du broadcast vers ce client WS
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Tâche pour recevoir les messages du client WS et les envoyer au broadcast
    let tx = state.tx.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let _ = tx.send(format!("Message: {}", text));
        }
    });

    // On attend qu'une des deux tâches se termine (client déconnecté ou erreur)
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    // Client déconnecté
    {
        let mut users = state.connected_users.lock().await;
        *users -= 1;
        let _ = state.tx.send(format!("Un utilisateur a quitté. Total: {}", *users));
    }
}
