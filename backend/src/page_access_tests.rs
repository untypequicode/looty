//! Matrice d'autorisation des pages (ADR 0022) : personne x relation à la page x action, vue de
//! l'extérieur (HTTP, session réelle). Les tests décrivent le comportement attendu du modèle
//! actuel (invités : partage `viewer` / `editor`, sous-pages comprises ou non).

use axum::{body::Body, http::Request};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use time::{Duration, OffsetDateTime};
use tower::ServiceExt;
use tower_sessions::session::{Id, Record};
use tower_sessions::SessionStore;
use uuid::Uuid;

use crate::access::SESSION_USER_KEY;

struct World {
    app: axum::Router,
    store: crate::session_store::PgSessionStore,
    pool: PgPool,
    ws: Uuid,
    /// Propriétaire, membre, invité en lecture, invité en modification, personne sans partage.
    ana: Uuid,
    ben: Uuid,
    gus: Uuid,
    edi: Uuid,
    zed: Uuid,
    /// Top (non partagée) > shared > child > grand ; other : page sœur non partagée.
    top: Uuid,
    shared: Uuid,
    child: Uuid,
    grand: Uuid,
    other: Uuid,
}

async fn user(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id").bind(format!("{name}@example.org")).bind(name).fetch_one(pool).await.unwrap()
}

async fn page(pool: &PgPool, ws: Uuid, by: Uuid, title: &str, parent: Option<Uuid>) -> Uuid {
    sqlx::query_scalar("INSERT INTO pages (workspace_id, title, content, created_by, parent_id) VALUES ($1, $2, $3, $4, $5) RETURNING id")
        .bind(ws)
        .bind(title)
        .bind(format!("<p>{title} needle</p>"))
        .bind(by)
        .bind(parent)
        .fetch_one(pool)
        .await
        .unwrap()
}

impl World {
    async fn new(pool: PgPool) -> World {
        let store = crate::session_store::PgSessionStore::new(pool.clone());
        store.migrate().await.unwrap();
        let app = crate::app(crate::state::AppState::new(pool.clone(), "http://localhost".into()));
        let (ana, ben, gus, edi, zed) = (user(&pool, "ana").await, user(&pool, "ben").await, user(&pool, "gus").await, user(&pool, "edi").await, user(&pool, "zed").await);
        let ws: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('Team') RETURNING id").fetch_one(&pool).await.unwrap();
        for (u, role) in [(ana, "owner"), (ben, "member")] {
            sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3)").bind(ws).bind(u.to_string()).bind(role).execute(&pool).await.unwrap();
        }
        let top = page(&pool, ws, ana, "Top secret parent", None).await;
        let shared = page(&pool, ws, ana, "Shared root", Some(top)).await;
        let child = page(&pool, ws, ana, "Child", Some(shared)).await;
        let grand = page(&pool, ws, ana, "Grand", Some(child)).await;
        let other = page(&pool, ws, ana, "Other", None).await;
        let w = World { app, store, pool, ws, ana, ben, gus, edi, zed, top, shared, child, grand, other };
        w.share(shared, gus, "viewer", true).await;
        w.share(shared, edi, "editor", true).await;
        w
    }

    async fn session(&self, user: Uuid) -> String {
        let mut data = HashMap::new();
        data.insert(SESSION_USER_KEY.to_string(), json!(user));
        let mut record = Record { id: Id::default(), data, expiry_date: OffsetDateTime::now_utc() + Duration::days(1) };
        self.store.create(&mut record).await.unwrap();
        record.id.to_string()
    }

    async fn call(&self, who: Uuid, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let sid = self.session(who).await;
        let mut req = Request::builder().method(method).uri(path).header("cookie", format!("looty_session={sid}")).header("origin", "http://localhost");
        let body = match body {
            Some(b) => {
                req = req.header("content-type", "application/json");
                Body::from(b.to_string())
            }
            None => Body::empty(),
        };
        let res = self.app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status().as_u16();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn share(&self, page: Uuid, who: Uuid, role: &str, inherit: bool) {
        let (s, b) = self.call(self.ana, "PUT", &format!("/api/workspaces/{}/pages/{page}/permissions", self.ws), Some(json!({ "user_id": who, "role": role, "inherit": inherit }))).await;
        assert_eq!(s, 204, "{b}");
    }

    async fn get_page(&self, who: Uuid, page: Uuid) -> u16 {
        self.call(who, "GET", &format!("/api/workspaces/{}/pages/{page}", self.ws), None).await.0
    }

    async fn visible_titles(&self, who: Uuid) -> Vec<String> {
        let (s, b) = self.call(who, "GET", &format!("/api/workspaces/{}/pages", self.ws), None).await;
        assert_eq!(s, 200, "{b}");
        let mut t: Vec<String> = b.as_array().unwrap().iter().map(|p| p["title"].as_str().unwrap().to_string()).collect();
        t.sort();
        t
    }

    async fn collab(&self, who: Uuid, page: Uuid) -> (u16, Value) {
        self.call(who, "GET", &format!("/api/pages/{page}/access"), None).await
    }
}

#[sqlx::test]
async fn an_outsider_reads_the_shared_page_and_its_subpages_but_nothing_above_or_beside(pool: PgPool) {
    let w = World::new(pool).await;
    for p in [w.shared, w.child, w.grand] {
        assert_eq!(w.get_page(w.gus, p).await, 200);
    }
    for p in [w.top, w.other] {
        assert_eq!(w.get_page(w.gus, p).await, 404, "no access to a page that is not shared, nor to its existence");
    }
    assert_eq!(w.visible_titles(w.gus).await, vec!["Child", "Grand", "Shared root"], "the list holds only shared pages: no parent title leaks");
    // Sans partage : même l'espace est inconnu.
    for p in [w.shared, w.top] {
        assert_eq!(w.get_page(w.zed, p).await, 404);
    }
    assert_eq!(w.call(w.zed, "GET", &format!("/api/workspaces/{}/pages", w.ws), None).await.0, 404);
}

#[sqlx::test]
async fn read_and_edit_guests_differ_for_writes_and_the_collaboration_check(pool: PgPool) {
    let w = World::new(pool).await;
    let url = |p: Uuid| format!("/api/workspaces/{}/pages/{p}", w.ws);
    let (s, _) = w.call(w.gus, "PUT", &url(w.child), Some(json!({ "title": "Hacked" }))).await;
    assert_eq!(s, 403, "a reading guest cannot edit");
    let (s, _) = w.call(w.edi, "PUT", &url(w.child), Some(json!({ "title": "Edited" }))).await;
    assert_eq!(s, 200);
    assert_eq!(w.collab(w.gus, w.grand).await.1["can_write"], false);
    assert_eq!(w.collab(w.edi, w.grand).await.1["can_write"], true);
    assert_eq!(w.collab(w.edi, w.top).await.0, 404);
    assert_eq!(w.collab(w.zed, w.shared).await.0, 404);
    // Les invités ne dépassent pas leur sous-arbre : ni vers la racine, ni vers une page non partagée.
    let (s, _) = w.call(w.edi, "PUT", &url(w.child), Some(json!({ "parent_id": null }))).await;
    assert_eq!(s, 403);
    let (s, _) = w.call(w.edi, "PUT", &url(w.child), Some(json!({ "parent_id": w.other }))).await;
    assert_eq!(s, 404);
    // Ils ne gèrent pas les partages (la route n'existe pas pour qui n'est pas membre).
    let (s, _) = w.call(w.edi, "PUT", &format!("/api/workspaces/{}/pages/{}/permissions", w.ws, w.child), Some(json!({ "user_id": w.zed, "role": "viewer" }))).await;
    assert_eq!(s, 404);
}

#[sqlx::test]
async fn a_share_without_subpages_stops_at_the_page(pool: PgPool) {
    let w = World::new(pool).await;
    w.share(w.shared, w.gus, "viewer", false).await;
    assert_eq!(w.get_page(w.gus, w.shared).await, 200);
    assert_eq!(w.get_page(w.gus, w.child).await, 404);
    assert_eq!(w.visible_titles(w.gus).await, vec!["Shared root"]);
    // Un partage direct sur une sous-page l'ouvre (et ses descendants) sans ouvrir le reste.
    w.share(w.child, w.gus, "editor", true).await;
    assert_eq!(w.get_page(w.gus, w.grand).await, 200);
    assert_eq!(w.collab(w.gus, w.grand).await.1["can_write"], true, "the nearest grant wins: the sub-page grant overrides the parent one");
    assert_eq!(w.collab(w.gus, w.shared).await.1["can_write"], false);
}

#[sqlx::test]
async fn shared_with_me_lists_roots_and_inherited_subpages_and_nothing_else(pool: PgPool) {
    let w = World::new(pool).await;
    let (s, b) = w.call(w.gus, "GET", "/api/shared", None).await;
    assert_eq!(s, 200);
    let rows: HashMap<String, bool> = b.as_array().unwrap().iter().map(|p| (p["title"].as_str().unwrap().to_string(), p["root"].as_bool().unwrap())).collect();
    assert_eq!(rows, HashMap::from([("Shared root".to_string(), true), ("Child".to_string(), false), ("Grand".to_string(), false)]));
    assert_eq!(w.call(w.zed, "GET", "/api/shared", None).await.1, json!([]));
    // Un membre de l'espace n'y retrouve pas les pages de son propre espace.
    assert_eq!(w.call(w.ben, "GET", "/api/shared", None).await.1, json!([]));
}

#[sqlx::test]
async fn revoking_a_share_cuts_every_door_at_once(pool: PgPool) {
    let w = World::new(pool).await;
    assert_eq!(w.collab(w.edi, w.grand).await.0, 200);
    w.share(w.shared, w.edi, "none", true).await;
    for p in [w.shared, w.child, w.grand] {
        assert_eq!(w.get_page(w.edi, p).await, 404);
        assert_eq!(w.collab(w.edi, p).await.0, 404, "the collaboration server's check refuses right away");
        let (s, _) = w.call(w.edi, "GET", &format!("/api/pages/{p}/revisions"), None).await;
        assert_eq!(s, 404);
        let (s, _) = w.call(w.edi, "GET", &format!("/api/workspaces/{}/comments?kind=page&id={p}", w.ws), None).await;
        assert_eq!(s, 404);
    }
    assert_eq!(w.call(w.edi, "GET", "/api/shared", None).await.1, json!([]));
    assert_eq!(w.call(w.edi, "GET", "/api/workspaces", None).await.1["workspaces"], json!([]), "the space disappears from the guest's list");
}

#[sqlx::test]
async fn an_outsider_finds_shared_pages_in_search_and_nothing_else(pool: PgPool) {
    let w = World::new(pool).await;
    let (s, b) = w.call(w.gus, "GET", &format!("/api/workspaces/{}/search?q=needle", w.ws), None).await;
    assert_eq!(s, 200, "{b}");
    let mut titles: Vec<&str> = b["results"].as_array().unwrap().iter().map(|r| r["title"].as_str().unwrap()).collect();
    titles.sort();
    assert_eq!(titles, vec!["Child", "Grand", "Shared root"]);
    for r in b["results"].as_array().unwrap() {
        assert!(!r["path"].to_string().contains("Top secret"), "the breadcrumb stops at what the guest may see: {r}");
    }
    assert_eq!(w.call(w.zed, "GET", &format!("/api/workspaces/{}/search?q=needle", w.ws), None).await.0, 404);
}

#[sqlx::test]
async fn search_is_not_starved_by_pages_the_person_cannot_see(pool: PgPool) {
    let w = World::new(pool).await;
    // Beaucoup de pages plus récentes qui correspondent mais ne sont pas partagées.
    for i in 0..60 {
        page(&w.pool, w.ws, w.ana, &format!("Noise {i}"), None).await;
    }
    sqlx::query("UPDATE pages SET updated_at = NOW() + interval '1 day' WHERE title LIKE 'Noise%'").execute(&w.pool).await.unwrap();
    let (_, b) = w.call(w.gus, "GET", &format!("/api/workspaces/{}/search?q=needle", w.ws), None).await;
    assert_eq!(b["results"].as_array().unwrap().len(), 3, "{b}");
}

#[sqlx::test]
async fn the_public_link_token_is_only_shown_to_people_who_can_edit(pool: PgPool) {
    let w = World::new(pool).await;
    sqlx::query("UPDATE pages SET share_token = gen_random_uuid(), is_public = true WHERE id = $1").bind(w.child).execute(&w.pool).await.unwrap();
    let (_, read) = w.call(w.gus, "GET", &format!("/api/workspaces/{}/pages/{}", w.ws, w.child), None).await;
    assert!(read["share_token"].is_null(), "a reading guest does not get the public token: {read}");
    let (_, list) = w.call(w.gus, "GET", &format!("/api/workspaces/{}/pages", w.ws), None).await;
    assert!(list.as_array().unwrap().iter().all(|p| p["share_token"].is_null()));
    let (_, write) = w.call(w.edi, "GET", &format!("/api/workspaces/{}/pages/{}", w.ws, w.child), None).await;
    assert!(write["share_token"].is_string());
}

#[sqlx::test]
async fn moving_a_page_out_of_a_shared_tree_cuts_access_and_warns_the_collaboration_server(pool: PgPool) {
    let w = World::new(pool).await;
    let mut listener = sqlx::postgres::PgListener::connect_with(&w.pool).await.unwrap();
    listener.listen("looty_access").await.unwrap();
    let (s, b) = w.call(w.ben, "PUT", &format!("/api/workspaces/{}/pages/{}", w.ws, w.child), Some(json!({ "parent_id": w.other }))).await;
    assert_eq!(s, 200, "{b}");
    for p in [w.child, w.grand] {
        assert_eq!(w.get_page(w.gus, p).await, 404, "access inherited from the old parent is lost");
        assert_eq!(w.collab(w.edi, p).await.0, 404);
    }
    let note = tokio::time::timeout(std::time::Duration::from_secs(2), listener.recv()).await;
    assert!(note.is_ok(), "open collaboration sessions are revalidated immediately, not at the next 2-minute tick");
}

#[sqlx::test]
async fn a_blocked_member_and_a_sub_page_override_for_members(pool: PgPool) {
    let w = World::new(pool).await;
    // Refus explicite (membres) : la page et ses sous-pages disparaissent, le reste de l'espace demeure.
    w.share(w.child, w.ben, "blocked", true).await;
    assert_eq!(w.get_page(w.ben, w.child).await, 404);
    assert_eq!(w.get_page(w.ben, w.grand).await, 404);
    assert_eq!(w.get_page(w.ben, w.shared).await, 200);
    assert!(!w.visible_titles(w.ben).await.contains(&"Grand".to_string()));
    // Un refus ne s'applique pas à une personne extérieure : « blocked » n'a pas de sens sans accès de base.
    let (s, _) = w.call(w.ana, "PUT", &format!("/api/workspaces/{}/pages/{}/permissions", w.ws, w.child), Some(json!({ "user_id": w.zed, "role": "blocked" }))).await;
    assert_eq!(s, 400);
}
