use axum::extract::FromRef;
use dashmap::DashMap;
use openidconnect::core::CoreClient;
use openidconnect::{EndpointMaybeSet, EndpointNotSet, EndpointSet};

/// Client OIDC découvert (point d'autorisation connu ; jeton et profil selon le fournisseur).
pub type OidcClient = CoreClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointMaybeSet, EndpointMaybeSet>;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::ws::Hub;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub oidc_clients: Arc<DashMap<Uuid, OidcClient>>,
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

    /// Événement destiné à une seule personne (tous ses onglets, dans chacun de ses espaces).
    pub async fn emit_user(&self, user_id: Uuid, event: serde_json::Value) {
        let spaces: Vec<Uuid> = sqlx::query_scalar("SELECT workspace_id FROM workspace_members WHERE user_id = $1")
            .bind(user_id.to_string())
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        for ws in spaces {
            self.hub.send_to(ws, user_id, event.clone()).await;
        }
    }
}
