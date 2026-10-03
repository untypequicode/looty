use axum::extract::FromRef;
use dashmap::DashMap;
use openidconnect::core::CoreClient;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::ws::Hub;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub oidc_clients: Arc<DashMap<Uuid, CoreClient>>,
    pub hub: Arc<Hub>,
    pub base_url: String,
    /// Réveille la synchronisation CalDAV après une modification de tâche.
    pub caldav_notify: Arc<tokio::sync::Notify>,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl AppState {
    /// Diffuse un événement temps réel à tous les membres connectés d'un workspace.
    pub async fn emit(&self, workspace_id: Uuid, event: serde_json::Value) {
        if event["type"].as_str().is_some_and(|t| t.starts_with("TASK_")) {
            self.caldav_notify.notify_one();
        }
        self.hub.broadcast(workspace_id, event).await;
    }
}
