//! Limites des jetons d'accès personnels (GLOSSARY : Jeton d'accès), appliquées avant les routes.
//!
//! Un jeton agit au nom de la personne, jamais au-delà de ses droits ; il peut seulement être plus
//! étroit : lecture seule, espaces choisis, expiration. Les contrôles de rôle restent ceux de chaque
//! route (`access.rs`) ; ce filtre ne fait que retirer ce que le jeton n'autorise pas.
//!
//! - Un jeton ne gère jamais les jetons ni le compte (créer un jeton plus large, supprimer le compte).
//! - Lecture seule : seules les requêtes GET et HEAD passent.
//! - Espaces choisis : seules passent les routes dont on sait de quel espace elles parlent
//!   (`/api/workspaces/{id}/…`, et les tableaux, colonnes, tâches, étiquettes et pages retrouvés en
//!   base). Tout le reste est refusé, par défaut : une route ajoutée plus tard reste fermée.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::hash_token;
use crate::state::AppState;

#[derive(sqlx::FromRow)]
struct Token {
    scope: String,
    access: String,
    expired: bool,
    workspace_ids: Option<Vec<Uuid>>,
}

/// Où va une requête, du point de vue d'un jeton limité à certains espaces.
#[derive(Debug, PartialEq)]
pub(crate) enum Target {
    /// Ne touche aucun espace (profil, spécification).
    Free,
    /// `GET /api/workspaces` : la liste est filtrée sur les espaces du jeton.
    WorkspaceList,
    Workspace(Uuid),
    Board(Uuid),
    Column(Uuid),
    Task(Uuid),
    Label(Uuid),
    Page(Uuid),
    Unknown,
}

fn bearer(req: &Request) -> Option<&str> {
    req.headers().get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ").map(str::trim)
}

fn refuse(status: StatusCode, code: &str) -> Response {
    (status, Json(serde_json::json!({ "error": code }))).into_response()
}

/// Routes qu'aucun jeton n'atteint : gestion des jetons, suppression du compte, connexions.
pub(crate) fn never_for_tokens(path: &str) -> bool {
    let segs: Vec<&str> = path.trim_start_matches("/api/").split('/').filter(|s| !s.is_empty()).collect();
    match segs.as_slice() {
        ["user", "tokens", ..] | ["user", "deletion", ..] => true,
        ["auth", "me"] => false,
        ["auth", ..] => true,
        _ => false,
    }
}

pub(crate) fn classify(path: &str) -> Target {
    let segs: Vec<&str> = path.strip_prefix("/api/").unwrap_or("").split('/').filter(|s| !s.is_empty()).collect();
    let id = |s: &&str| Uuid::parse_str(s).ok();
    match segs.as_slice() {
        ["auth", "me"] | ["openapi.json"] | ["health"] | ["settings"] => Target::Free,
        ["workspaces"] => Target::WorkspaceList,
        ["workspaces", w, ..] => id(w).map(Target::Workspace).unwrap_or(Target::Unknown),
        ["boards", "columns", c, ..] => id(c).map(Target::Column).unwrap_or(Target::Unknown),
        ["boards", "tasks", t, ..] => id(t).map(Target::Task).unwrap_or(Target::Unknown),
        ["boards", "labels", l, ..] => id(l).map(Target::Label).unwrap_or(Target::Unknown),
        ["boards", b, ..] => id(b).map(Target::Board).unwrap_or(Target::Unknown),
        ["pages", p, ..] => id(p).map(Target::Page).unwrap_or(Target::Unknown),
        _ => Target::Unknown,
    }
}

async fn workspace_of(pool: &PgPool, target: &Target) -> Option<Uuid> {
    match target {
        Target::Workspace(w) => Some(*w),
        Target::Board(b) => crate::access::board_workspace(pool, *b).await.ok(),
        Target::Column(c) => crate::access::column_ctx(pool, *c).await.ok().map(|c| c.workspace_id),
        Target::Task(t) => crate::access::task_ctx(pool, *t).await.ok().map(|c| c.workspace_id),
        Target::Label(l) => crate::access::label_ctx(pool, *l).await.ok().map(|c| c.0),
        Target::Page(p) => crate::access::page_workspace(pool, *p).await.ok(),
        _ => None,
    }
}

pub async fn guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let Some(raw) = bearer(&req).filter(|r| r.starts_with("looty_")).map(str::to_string) else {
        return next.run(req).await;
    };
    if !path.starts_with("/api/") {
        return next.run(req).await;
    }
    let tok: Option<Token> = sqlx::query_as(
        "SELECT scope, access, (expires_at IS NOT NULL AND expires_at <= NOW()) AS expired, workspace_ids FROM api_tokens WHERE token_hash = $1",
    )
    .bind(hash_token(&raw))
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);
    // Jeton inconnu ou d'une autre portée (CalDAV, flux) : l'extracteur répondra 401.
    let Some(tok) = tok.filter(|t| t.scope == "api") else {
        return next.run(req).await;
    };
    if tok.expired {
        return refuse(StatusCode::UNAUTHORIZED, "token_expired");
    }
    if never_for_tokens(&path) {
        return refuse(StatusCode::FORBIDDEN, "token_not_allowed");
    }
    if tok.access == "read" && !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return refuse(StatusCode::FORBIDDEN, "token_read_only");
    }
    let Some(allowed) = tok.workspace_ids.filter(|w| !w.is_empty()) else {
        return next.run(req).await;
    };
    let target = classify(&path);
    match target {
        Target::Free => next.run(req).await,
        Target::WorkspaceList if req.method() == Method::GET => {
            let res = next.run(req).await;
            filter_workspace_list(res, &allowed).await
        }
        Target::WorkspaceList | Target::Unknown => refuse(StatusCode::FORBIDDEN, "token_workspace_scope"),
        other => match workspace_of(&state.pool, &other).await {
            Some(w) if allowed.contains(&w) => next.run(req).await,
            // Ressource introuvable : même réponse que pour un espace hors du jeton.
            _ => refuse(StatusCode::FORBIDDEN, "token_workspace_scope"),
        },
    }
}

/// Ne garde, dans `{ "workspaces": [...] }`, que les espaces du jeton.
async fn filter_workspace_list(res: Response, allowed: &[Uuid]) -> Response {
    if !res.status().is_success() {
        return res;
    }
    let (parts, body) = res.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, 8 * 1024 * 1024).await else {
        return refuse(StatusCode::INTERNAL_SERVER_ERROR, "internal_error");
    };
    let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return refuse(StatusCode::INTERNAL_SERVER_ERROR, "internal_error");
    };
    if let Some(list) = json.get_mut("workspaces").and_then(|w| w.as_array_mut()) {
        list.retain(|w| w["id"].as_str().and_then(|s| Uuid::parse_str(s).ok()).is_some_and(|id| allowed.contains(&id)));
    }
    let mut res = Response::from_parts(parts, Body::from(json.to_string()));
    res.headers_mut().remove(header::CONTENT_LENGTH);
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "00000000-0000-0000-0000-000000000001";

    #[test]
    fn classify_finds_the_workspace_of_each_kind_of_route() {
        let id = Uuid::parse_str(ID).unwrap();
        assert_eq!(classify("/api/workspaces"), Target::WorkspaceList);
        assert_eq!(classify(&format!("/api/workspaces/{ID}/boards")), Target::Workspace(id));
        assert_eq!(classify(&format!("/api/boards/tasks/{ID}/move")), Target::Task(id));
        assert_eq!(classify(&format!("/api/boards/columns/{ID}/tasks")), Target::Column(id));
        assert_eq!(classify(&format!("/api/boards/{ID}/columns")), Target::Board(id));
        assert_eq!(classify(&format!("/api/pages/{ID}/access")), Target::Page(id));
        assert_eq!(classify("/api/workspaces/personal"), Target::Unknown);
        assert_eq!(classify("/api/user/profile"), Target::Unknown);
        assert_eq!(classify("/api/mail/messages"), Target::Unknown);
        assert_eq!(classify("/api/auth/me"), Target::Free);
    }

    #[test]
    fn tokens_never_manage_tokens_or_the_account() {
        for p in ["/api/user/tokens", "/api/user/tokens/x", "/api/user/deletion", "/api/auth/logout", "/api/auth/dev-login"] {
            assert!(never_for_tokens(p), "{p}");
        }
        for p in ["/api/auth/me", "/api/user/profile", "/api/workspaces"] {
            assert!(!never_for_tokens(p), "{p}");
        }
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use axum::http::Request;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    struct Api {
        app: axum::Router,
        pool: PgPool,
    }

    impl Api {
        async fn new(pool: &PgPool) -> Api {
            crate::session_store::PgSessionStore::new(pool.clone()).migrate().await.unwrap();
            Api { app: crate::app(AppState::new(pool.clone(), "http://localhost".into())), pool: pool.clone() }
        }
        async fn user(&self, name: &str) -> Uuid {
            sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id").bind(format!("{name}@example.org")).bind(name).fetch_one(&self.pool).await.unwrap()
        }
        async fn space(&self, name: &str, owner: Uuid) -> Uuid {
            let ws: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ($1) RETURNING id").bind(name).fetch_one(&self.pool).await.unwrap();
            sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')").bind(ws).bind(owner.to_string()).execute(&self.pool).await.unwrap();
            ws
        }
        async fn token(&self, user: Uuid, access: &str, expires_in_hours: i64, spaces: Option<Vec<Uuid>>) -> String {
            let raw = format!("looty_{}", Uuid::new_v4().simple());
            sqlx::query("INSERT INTO api_tokens (user_id, name, token_hash, scope, access, expires_at, workspace_ids) VALUES ($1, 't', $2, 'api', $3, NOW() + make_interval(hours => $4::int), $5)")
                .bind(user)
                .bind(hash_token(&raw))
                .bind(access)
                .bind(expires_in_hours as i32)
                .bind(spaces)
                .execute(&self.pool)
                .await
                .unwrap();
            raw
        }
        async fn call(&self, token: &str, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
            let req = Request::builder().method(method).uri(path).header("content-type", "application/json").header("authorization", format!("Bearer {token}"));
            let res = self.app.clone().oneshot(req.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap()).await.unwrap();
            let status = res.status().as_u16();
            let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
            (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
        }
    }

    fn code(v: &Value) -> &str {
        v["error"].as_str().unwrap_or("")
    }

    #[sqlx::test]
    async fn a_read_only_token_reads_but_never_writes(pool: PgPool) {
        let api = Api::new(&pool).await;
        let ana = api.user("ana").await;
        let ws = api.space("Team", ana).await;
        let ro = api.token(ana, "read", 24, None).await;
        assert_eq!(api.call(&ro, "GET", &format!("/api/workspaces/{ws}/boards"), None).await.0, 200);
        let (status, body) = api.call(&ro, "POST", &format!("/api/workspaces/{ws}/boards"), Some(json!({ "name": "X" }))).await;
        assert_eq!((status, code(&body)), (403, "token_read_only"));
        let rw = api.token(ana, "write", 24, None).await;
        assert_eq!(api.call(&rw, "POST", &format!("/api/workspaces/{ws}/boards"), Some(json!({ "name": "X" }))).await.0, 200);
    }

    #[sqlx::test]
    async fn a_token_limited_to_some_workspaces_sees_nothing_else(pool: PgPool) {
        let api = Api::new(&pool).await;
        let ana = api.user("ana").await;
        let (a, b) = (api.space("A", ana).await, api.space("B", ana).await);
        let tok = api.token(ana, "write", 24, Some(vec![a])).await;
        let full = api.token(ana, "write", 24, None).await;
        // Une tâche dans chaque espace, créée par le jeton complet.
        let mut tasks = Vec::new();
        for ws in [a, b] {
            let (_, board) = api.call(&full, "POST", &format!("/api/workspaces/{ws}/boards"), Some(json!({ "name": "Kanban" }))).await;
            let (_, full_board) = api.call(&full, "GET", &format!("/api/boards/{}", board["id"].as_str().unwrap()), None).await;
            let col = full_board["columns"][0]["id"].as_str().unwrap().to_string();
            let (status, task) = api.call(&full, "POST", &format!("/api/boards/columns/{col}/tasks"), Some(json!({ "title": "T" }))).await;
            assert_eq!(status, 200, "{task}");
            tasks.push((board["id"].as_str().unwrap().to_string(), task["id"].as_str().unwrap().to_string()));
        }
        let (_, list) = api.call(&tok, "GET", "/api/workspaces", None).await;
        let ids: Vec<&str> = list["workspaces"].as_array().unwrap().iter().map(|w| w["id"].as_str().unwrap()).collect();
        assert_eq!(ids, [a.to_string()], "only the token's workspace is listed");
        assert_eq!(api.call(&tok, "GET", &format!("/api/workspaces/{a}/boards"), None).await.0, 200);
        assert_eq!(api.call(&tok, "GET", &format!("/api/boards/tasks/{}", tasks[0].1), None).await.0, 200);
        for (path, what) in [
            (format!("/api/workspaces/{b}/boards"), "other workspace"),
            (format!("/api/boards/tasks/{}", tasks[1].1), "task of another workspace"),
            (format!("/api/boards/{}", tasks[1].0), "board of another workspace"),
            ("/api/user/profile".to_string(), "account settings"),
            ("/api/mail/accounts".to_string(), "route that is not workspace-scoped"),
        ] {
            let (status, body) = api.call(&tok, "GET", &path, None).await;
            assert_eq!((status, code(&body)), (403, "token_workspace_scope"), "{what}");
        }
        // Le jeton complet, lui, voit les deux.
        let (_, all) = api.call(&full, "GET", "/api/workspaces", None).await;
        assert_eq!(all["workspaces"].as_array().unwrap().len(), 2);
    }

    #[sqlx::test]
    async fn an_expired_token_is_refused_and_tokens_cannot_manage_tokens(pool: PgPool) {
        let api = Api::new(&pool).await;
        let ana = api.user("ana").await;
        let old = api.token(ana, "write", -1, None).await;
        let (status, body) = api.call(&old, "GET", "/api/workspaces", None).await;
        assert_eq!((status, code(&body)), (401, "token_expired"));
        let ok = api.token(ana, "write", 1, None).await;
        assert_eq!(api.call(&ok, "GET", "/api/workspaces", None).await.0, 200);
        for (m, p) in [("POST", "/api/user/tokens"), ("GET", "/api/user/tokens"), ("POST", "/api/user/deletion")] {
            let (status, body) = api.call(&ok, m, p, Some(json!({ "name": "wider" }))).await;
            assert_eq!((status, code(&body)), (403, "token_not_allowed"), "{m} {p}");
        }
        assert_eq!(api.call("looty_nope", "GET", "/api/workspaces", None).await.0, 401);
    }

    #[sqlx::test]
    async fn boards_and_documents_can_start_from_a_template(pool: PgPool) {
        let api = Api::new(&pool).await;
        let ana = api.user("ana").await;
        let ben = api.user("ben").await;
        let ws = api.space("Team", ana).await;
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'viewer')").bind(ws).bind(ben.to_string()).execute(&pool).await.unwrap();
        let tok = api.token(ana, "write", 24, None).await;
        // Modèles fournis (français par défaut, anglais selon la langue de la personne).
        let (_, list) = api.call(&tok, "GET", &format!("/api/workspaces/{ws}/templates"), None).await;
        assert_eq!(list.as_array().unwrap().len(), 5);
        let (status, board) = api.call(&tok, "POST", &format!("/api/workspaces/{ws}/boards"), Some(json!({ "name": "Sprint", "template_id": "project-tracking" }))).await;
        assert_eq!(status, 200, "{board}");
        let (_, full) = api.call(&tok, "GET", &format!("/api/boards/{}", board["id"].as_str().unwrap()), None).await;
        assert_eq!(full["columns"].as_array().unwrap().len(), 5);
        assert_eq!(full["labels"].as_array().unwrap().len(), 3);
        assert!(board["automations"]["on_complete_move_to"].is_string(), "the done column is wired");
        // Enregistrer ce tableau (avec une tâche de plus) comme modèle d'espace, puis en créer un autre.
        let col = full["columns"][1]["id"].as_str().unwrap();
        api.call(&tok, "POST", &format!("/api/boards/columns/{col}/tasks"), Some(json!({ "title": "Task type" }))).await;
        let (status, tpl) = api.call(&tok, "POST", &format!("/api/workspaces/{ws}/templates"), Some(json!({ "kind": "board", "source_id": board["id"], "name": "Our sprint" }))).await;
        assert_eq!(status, 200, "{tpl}");
        let (_, copy) = api.call(&tok, "POST", &format!("/api/workspaces/{ws}/boards"), Some(json!({ "name": "Next sprint", "template_id": tpl["id"] }))).await;
        let (_, copy_full) = api.call(&tok, "GET", &format!("/api/boards/{}", copy["id"].as_str().unwrap()), None).await;
        assert_eq!(copy_full["columns"].as_array().unwrap().len(), 5);
        let titles: Vec<&str> = copy_full["columns"].as_array().unwrap().iter().flat_map(|c| c["tasks"].as_array().unwrap()).map(|t| t["title"].as_str().unwrap()).collect();
        assert!(titles.contains(&"Task type"), "{titles:?}");
        // Un document : depuis un modèle fourni, puis depuis un document enregistré.
        let (status, page) = api.call(&tok, "POST", &format!("/api/workspaces/{ws}/pages"), Some(json!({ "template_id": "meeting-notes" }))).await;
        assert_eq!(status, 201, "{page}");
        assert!(page["content"].as_str().unwrap().contains("<h2>"));
        assert!(!page["title"].as_str().unwrap().is_empty());
        let (status, ptpl) = api.call(&tok, "POST", &format!("/api/workspaces/{ws}/templates"), Some(json!({ "kind": "page", "source_id": page["id"], "name": "Weekly" }))).await;
        assert_eq!(status, 200, "{ptpl}");
        let (status, page2) = api.call(&tok, "POST", &format!("/api/workspaces/{ws}/pages"), Some(json!({ "title": "Week 3", "template_id": ptpl["id"] }))).await;
        assert_eq!((status, page2["title"].as_str()), (201, Some("Week 3")));
        // Le mauvais genre, un modèle inconnu ou celui d'un autre espace sont refusés.
        let (status, _) = api.call(&tok, "POST", &format!("/api/workspaces/{ws}/boards"), Some(json!({ "name": "x", "template_id": "meeting-notes" }))).await;
        assert_eq!(status, 400);
        let other = api.space("Other", ana).await;
        let (status, _) = api.call(&tok, "POST", &format!("/api/workspaces/{other}/boards"), Some(json!({ "name": "x", "template_id": tpl["id"] }))).await;
        assert_eq!(status, 404);
        // Un lecteur ne voit aucun modèle (il ne peut rien créer) et n'en enregistre pas.
        let viewer = api.token(ben, "write", 24, None).await;
        let (_, none) = api.call(&viewer, "GET", &format!("/api/workspaces/{ws}/templates"), None).await;
        assert!(none.as_array().unwrap().is_empty());
        let (status, _) = api.call(&viewer, "POST", &format!("/api/workspaces/{ws}/templates"), Some(json!({ "kind": "page", "source_id": page["id"] }))).await;
        assert_eq!(status, 403);
        // Un élément personnel ne devient jamais un modèle.
        let (_, mine) = api.call(&tok, "POST", &format!("/api/workspaces/{ws}/boards"), Some(json!({ "name": "Mine", "personal": true }))).await;
        let (status, _) = api.call(&tok, "POST", &format!("/api/workspaces/{ws}/templates"), Some(json!({ "kind": "board", "source_id": mine["id"] }))).await;
        assert_eq!(status, 403);
    }

    #[sqlx::test]
    async fn only_workspace_admins_manage_webhooks_and_the_secret_is_shown_once(pool: PgPool) {
        let api = Api::new(&pool).await;
        let (ana, ben) = (api.user("ana").await, api.user("ben").await);
        let ws = api.space("Team", ana).await;
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'member')").bind(ws).bind(ben.to_string()).execute(&pool).await.unwrap();
        let (admin, member) = (api.token(ana, "write", 24, None).await, api.token(ben, "write", 24, None).await);
        let path = format!("/api/workspaces/{ws}/webhooks");
        let body = json!({ "url": "https://1.1.1.1/hook", "events": ["task.created", "page.updated"] });
        assert_eq!(api.call(&member, "POST", &path, Some(body.clone())).await.0, 403);
        assert_eq!(api.call(&member, "GET", &path, None).await.0, 403);
        let (status, made) = api.call(&admin, "POST", &path, Some(body)).await;
        assert_eq!(status, 200, "{made}");
        assert!(made["secret"].as_str().unwrap().starts_with("whsec_"));
        let (_, list) = api.call(&admin, "GET", &path, None).await;
        assert_eq!(list["webhooks"].as_array().unwrap().len(), 1);
        assert!(!list.to_string().contains("whsec_"), "the secret is never listed");
        let stored: String = sqlx::query_scalar("SELECT secret_enc FROM workspace_webhooks").fetch_one(&pool).await.unwrap();
        assert!(!stored.contains("whsec_"), "stored encrypted");
        for bad in [json!({ "url": "https://127.0.0.1/x", "events": ["task.created"] }), json!({ "url": "http://1.1.1.1/x", "events": ["task.created"] }), json!({ "url": "https://1.1.1.1/x", "events": ["nope"] }), json!({ "url": "https://1.1.1.1/x", "events": [] })] {
            assert_eq!(api.call(&admin, "POST", &path, Some(bad)).await.0, 400);
        }
        let id = made["id"].as_str().unwrap();
        let (status, hook) = api.call(&admin, "PUT", &format!("{path}/{id}"), Some(json!({ "active": false }))).await;
        assert_eq!((status, hook["active"].as_bool()), (200, Some(false)));
        let (status, rotated) = api.call(&admin, "POST", &format!("{path}/{id}/secret"), None).await;
        assert_eq!(status, 200);
        assert_ne!(rotated["secret"], made["secret"]);
        assert_eq!(api.call(&admin, "DELETE", &format!("{path}/{id}"), None).await.0, 204);
        // Un espace personnel n'a pas de webhooks.
        let mine: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name, personal_of) VALUES ('Perso', $1) RETURNING id").bind(ana).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')").bind(mine).bind(ana.to_string()).execute(&pool).await.unwrap();
        assert_eq!(api.call(&admin, "GET", &format!("/api/workspaces/{mine}/webhooks"), None).await.0, 403);
    }
}
