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
    pub fn new(pool: PgPool, base_url: String) -> Self {
        AppState {
            pool,
            oidc_clients: Arc::new(DashMap::new()),
            hub: Arc::new(Hub::new()),
            base_url,
            caldav_notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Diffuse un événement temps réel à tous les membres connectés d'un workspace.
    pub async fn emit(&self, workspace_id: Uuid, event: serde_json::Value) {
        let kind = event["type"].as_str().unwrap_or_default();
        if kind.starts_with("TASK_") {
            self.caldav_notify.notify_one();
        }
        // Droits modifiés : le serveur de collaboration revérifie les documents ouverts.
        if matches!(kind, "MEMBERS_UPDATED" | "PAGE_PERMISSIONS_UPDATED" | "WORKSPACE_DELETED") {
            let _ = sqlx::query("SELECT pg_notify('looty_access', $1)").bind(workspace_id.to_string()).execute(&self.pool).await;
        }
        // Élément personnel (GLOSSARY) : seul son propriétaire en est prévenu.
        let owner = ["task", "page", "board", "project", "file"]
            .iter()
            .find_map(|k| event.get(*k).and_then(|o| o.get("private_to")).and_then(|v| v.as_str()).and_then(|v| Uuid::parse_str(v).ok()));
        match owner {
            Some(user) => self.hub.send_to(workspace_id, user, event).await,
            None => self.hub.broadcast(workspace_id, event).await,
        }
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
