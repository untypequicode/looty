//! Service de fond : fonctionne même fenêtre fermée, avec la session détenue par Rust.
//!
//! - une connexion temps réel par espace de travail (`/api/ws`) ;
//! - notifications système pour les notifications Looty (selon les préférences
//!   « Push » de l'utilisateur), sauf quand la fenêtre est au premier plan ;
//! - état de la barre système : non lues, tâches en retard et du jour.

use chrono::{DateTime, Local, Utc};
use futures_util::StreamExt;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;
use url::Url;

use crate::proxy::Shared;

#[derive(Clone, Debug, Default)]
pub struct TrayState {
    pub logged_in: bool,
    pub unread: i64,
    pub overdue: Vec<TaskItem>,
    pub today: Vec<TaskItem>,
    pub multi_workspace: bool,
    /// Espace de travail par défaut (boîte de réception, mes tâches).
    pub workspace: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    pub board_id: String,
    pub board_name: String,
    pub workspace_id: String,
    pub workspace_name: String,
}

impl TaskItem {
    pub fn path(&self) -> String {
        format!("/w/{}/board/{}?task={}", self.workspace_id, self.board_id, self.id)
    }
}

/// Notification système à afficher.
pub struct OsNotification {
    pub title: String,
    pub body: String,
    pub path: String,
}

/// Actions de l'interface (fournies par main.rs).
pub struct Ui {
    pub tray: Box<dyn Fn(TrayState) + Send + Sync>,
    pub notify: Box<dyn Fn(OsNotification) + Send + Sync>,
    /// La fenêtre est-elle visible et au premier plan ?
    pub window_focused: Box<dyn Fn() -> bool + Send + Sync>,
}

pub struct Service {
    proxy: Shared,
    ui: Ui,
    refresh: Notify,
    /// Préférences push par type d'événement (rechargées périodiquement).
    push_prefs: Mutex<HashMap<String, bool>>,
    pub notifications_enabled: Mutex<bool>,
}

#[derive(Deserialize)]
struct Workspace {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct MyTask {
    id: String,
    title: String,
    due_date: Option<DateTime<Utc>>,
    board_id: String,
    board_name: String,
}

impl Service {
    pub fn start(proxy: Shared, ui: Ui, notifications_enabled: bool) -> Arc<Service> {
        let svc = Arc::new(Service {
            proxy,
            ui,
            refresh: Notify::new(),
            push_prefs: Mutex::new(HashMap::new()),
            notifications_enabled: Mutex::new(notifications_enabled),
        });
        let s = svc.clone();
        tauri::async_runtime::spawn(async move { s.run().await });
        svc
    }

    /// Demande une mise à jour de la barre système (après une action).
    pub fn refresh_soon(&self) {
        self.refresh.notify_one();
    }

    async fn get_json(&self, path: &str) -> Option<serde_json::Value> {
        let server = self.proxy.server()?;
        let res = self.proxy.http().get(format!("{}{}", server.origin().ascii_serialization(), path)).timeout(Duration::from_secs(15)).send().await.ok()?;
        if !res.status().is_success() {
            return None;
        }
        res.json().await.ok()
    }

    pub async fn put_json(&self, path: &str, body: serde_json::Value) -> bool {
        let Some(server) = self.proxy.server() else { return false };
        self.proxy
            .http()
            .put(format!("{}{}", server.origin().ascii_serialization(), path))
            .json(&body)
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    async fn workspaces(&self) -> Option<Vec<Workspace>> {
        let v = self.get_json("/api/workspaces").await?;
        serde_json::from_value(v["workspaces"].clone()).ok()
    }

    async fn run(self: Arc<Self>) {
        let mut listeners: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();
        let mut server_key = String::new();
        loop {
            let me = self.get_json("/api/auth/me").await;
            let logged_in = me.as_ref().is_some_and(|m| m["authenticated"] == true);
            let key = self.proxy.server().map(|s| s.to_string()).unwrap_or_default();
            // Serveur changé ou déconnexion : on ferme les connexions temps réel.
            if key != server_key || !logged_in {
                for (_, h) in listeners.drain() {
                    h.abort();
                }
                server_key = key;
            }
            if logged_in {
                self.load_prefs().await;
                if let Some(ws) = self.workspaces().await {
                    let ids: HashSet<String> = ws.iter().map(|w| w.id.clone()).collect();
                    listeners.retain(|id, h| {
                        let keep = ids.contains(id) && !h.is_finished();
                        if !keep {
                            h.abort();
                        }
                        keep
                    });
                    for w in &ws {
                        if !listeners.contains_key(&w.id) {
                            let s = self.clone();
                            let id = w.id.clone();
                            listeners.insert(id.clone(), tokio::spawn(async move { s.listen(id).await }));
                        }
                    }
                    let state = self.tray_state(&ws).await;
                    (self.ui.tray)(state);
                }
            } else {
                (self.ui.tray)(TrayState::default());
            }
            // Toutes les minutes, ou plus tôt après un événement / une action.
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                _ = self.refresh.notified() => { tokio::time::sleep(Duration::from_millis(800)).await; }
            }
        }
    }

    async fn load_prefs(&self) {
        if let Some(v) = self.get_json("/api/user/notifications").await {
            let mut map = HashMap::new();
            for e in v["events"].as_array().into_iter().flatten() {
                if let (Some(ev), Some(push)) = (e["event"].as_str(), e["push"].as_bool()) {
                    map.insert(ev.to_string(), push);
                }
            }
            *self.push_prefs.lock().unwrap() = map;
        }
    }

    async fn tray_state(&self, ws: &[Workspace]) -> TrayState {
        let mut state = TrayState { logged_in: true, multi_workspace: ws.len() > 1, workspace: ws.first().map(|w| w.id.clone()), ..Default::default() };
        let today = Local::now().date_naive();
        for w in ws {
            if let Some(v) = self.get_json(&format!("/api/notifications?workspace_id={}", w.id)).await {
                state.unread += v.as_array().map(|a| a.iter().filter(|n| n["read_at"].is_null()).count() as i64).unwrap_or(0);
            }
            let Some(v) = self.get_json(&format!("/api/workspaces/{}/my_tasks", w.id)).await else { continue };
            let tasks: Vec<MyTask> = serde_json::from_value(v["tasks"].clone()).unwrap_or_default();
            for t in tasks {
                let Some(due) = t.due_date else { continue };
                let day = due.with_timezone(&Local).date_naive();
                let item = TaskItem { id: t.id, title: t.title, board_id: t.board_id, board_name: t.board_name, workspace_id: w.id.clone(), workspace_name: w.name.clone() };
                if day < today {
                    state.overdue.push(item);
                } else if day == today {
                    state.today.push(item);
                }
            }
        }
        state
    }

    /// Connexion temps réel d'un espace (reconnexion avec attente progressive).
    async fn listen(self: Arc<Self>, workspace_id: String) {
        let mut backoff = 2u64;
        loop {
            let Some(server) = self.proxy.server() else { return };
            match self.connect(&server, &workspace_id).await {
                Ok(mut ws) => {
                    backoff = 2;
                    while let Some(Ok(msg)) = ws.next().await {
                        if let tokio_tungstenite::tungstenite::Message::Text(t) = msg {
                            if let Ok(ev) = serde_json::from_str::<serde_json::Value>(&t) {
                                self.on_event(&workspace_id, ev);
                            }
                        }
                    }
                }
                Err(e) => tracing::debug!("realtime {workspace_id}: {e}"),
            }
            tokio::time::sleep(Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(120);
        }
    }

    async fn connect(
        &self,
        server: &Url,
        workspace_id: &str,
    ) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, String> {
        let scheme = if server.scheme() == "https" { "wss" } else { "ws" };
        let port = server.port().map(|p| format!(":{p}")).unwrap_or_default();
        let url = format!("{scheme}://{}{port}/api/ws?workspace_id={workspace_id}", server.host_str().unwrap_or(""));
        let mut req = tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(url.as_str()).map_err(|e| e.to_string())?;
        let cookie_url = Url::parse(&format!("{}/api/ws", server.origin().ascii_serialization())).map_err(|e| e.to_string())?;
        let cookies = self.proxy.cookie_header(&cookie_url);
        if !cookies.is_empty() {
            req.headers_mut().insert("cookie", cookies.parse().map_err(|_| "cookie")?);
        }
        req.headers_mut().insert("origin", server.origin().ascii_serialization().parse().map_err(|_| "origin")?);
        let (ws, _) = tokio_tungstenite::connect_async(req).await.map_err(|e| e.to_string())?;
        Ok(ws)
    }

    fn on_event(&self, workspace_id: &str, ev: serde_json::Value) {
        match ev["type"].as_str() {
            Some("NOTIFICATION") => {
                self.refresh_soon();
                self.maybe_notify(workspace_id, &ev["notification"]);
            }
            Some(t) if t.starts_with("TASK_") => self.refresh_soon(),
            _ => {}
        }
    }

    fn maybe_notify(&self, workspace_id: &str, n: &serde_json::Value) {
        if !*self.notifications_enabled.lock().unwrap() || (self.ui.window_focused)() {
            return;
        }
        let kind = n["kind"].as_str().unwrap_or("");
        let target = n["target"].as_str().unwrap_or("");
        let event = event_of(kind, target);
        // Même règle que les notifications push : le choix « Push » de l'utilisateur.
        if !self.push_prefs.lock().unwrap().get(event).copied().unwrap_or(true) {
            return;
        }
        let actor = n["actor_name"].as_str().unwrap_or("");
        let title = n["data"]["title"].as_str().unwrap_or("");
        (self.ui.notify)(OsNotification { title: "Looty".into(), body: describe(kind, target, actor, title), path: target_path(workspace_id, target, &n["data"]) });
    }
}

/// Type d'événement (comme côté serveur, `delivery::event_of`).
fn event_of(kind: &str, target: &str) -> &'static str {
    match kind {
        "assigned" => "task_assigned",
        "comment" => "task_comment",
        "mention" if target.starts_with("task:") => "task_mention",
        "mention" => "doc_mention",
        "due_soon" => "task_due",
        "completed" => "task_completed",
        _ => "other",
    }
}

fn target_path(ws: &str, target: &str, data: &serde_json::Value) -> String {
    let (kind, id) = target.split_once(':').unwrap_or(("", ""));
    match kind {
        "task" => match data["board_id"].as_str() {
            Some(b) => format!("/w/{ws}/board/{b}?task={id}"),
            None => format!("/w/{ws}/task/{id}"),
        },
        "page" => format!("/w/{ws}/doc/{id}"),
        "board" => format!("/w/{ws}/board/{id}"),
        _ => format!("/w/{ws}/inbox"),
    }
}

pub fn french() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty())).is_some_and(|v| v.starts_with("fr"))
}

fn describe(kind: &str, target: &str, actor: &str, title: &str) -> String {
    let fr = french();
    let actor = if actor.is_empty() { if fr { "Quelqu’un" } else { "Someone" } } else { actor };
    let q = |s: &str| if s.is_empty() { String::new() } else if fr { format!(" « {s} »") } else { format!(" “{s}”") };
    match (event_of(kind, target), fr) {
        ("task_assigned", false) => format!("{actor} assigned you{}", q(title)),
        ("task_assigned", true) => format!("{actor} vous a assigné{}", q(title)),
        ("task_comment", false) => format!("{actor} commented on{}", q(title)),
        ("task_comment", true) => format!("{actor} a commenté{}", q(title)),
        ("task_mention" | "doc_mention", false) => format!("{actor} mentioned you{}", if title.is_empty() { String::new() } else { format!(" in{}", q(title)) }),
        ("task_mention" | "doc_mention", true) => format!("{actor} vous a mentionné{}", if title.is_empty() { String::new() } else { format!(" dans{}", q(title)) }),
        ("task_due", false) => format!("Due soon:{}", q(title)),
        ("task_due", true) => format!("Échéance proche :{}", q(title)),
        ("task_completed", false) => format!("{actor} completed{}", q(title)),
        ("task_completed", true) => format!("{actor} a terminé{}", q(title)),
        (_, false) => format!("New activity from {actor}"),
        (_, true) => format!("Nouvelle activité de {actor}"),
    }
}
