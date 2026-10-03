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
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl AppState {
    /// Diffuse un événement temps réel à tous les membres connectés d'un workspace.
    pub async fn emit(&self, workspace_id: Uuid, event: serde_json::Value) {
        self.hub.broadcast(workspace_id, event).await;
    }
}
