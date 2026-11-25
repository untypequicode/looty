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
    // Ils ne gèrent pas les partages sans le niveau « gestion » : ils lisent la page mais ne la partagent pas.
    let (s, _) = w.call(w.edi, "PUT", &format!("/api/workspaces/{}/pages/{}/permissions", w.ws, w.child), Some(json!({ "user_id": w.zed, "role": "viewer" }))).await;
    assert_eq!(s, 403);
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
    // Le refus vaut aussi pour une personne extérieure à l'espace (voir les tests de refus).
}

// ---------------------------------------------------------------------------
// Phase 2 : niveaux lecture < commentaire < modification < gestion, refus, étendue, migration.
// ---------------------------------------------------------------------------

impl World {
    async fn status(&self, who: Uuid, method: &str, path: String, body: Option<Value>) -> u16 {
        self.call(who, method, &path, body).await.0
    }

    async fn comment(&self, who: Uuid, page: Uuid) -> (u16, Value) {
        self.call(who, "POST", &format!("/api/workspaces/{}/comments", self.ws), Some(json!({ "kind": "page", "id": page, "content": "<p>hello</p>" }))).await
    }

    async fn activity_kinds(&self, page: Uuid) -> Vec<String> {
        sqlx::query_scalar("SELECT kind FROM page_activities WHERE page_id = $1 ORDER BY created_at").bind(page).fetch_all(&self.pool).await.unwrap()
    }
}

/// Personne x niveau x relation x action : lecture, commentaire, résolution, modification, gestion des partages, corbeille.
#[sqlx::test]
async fn every_level_opens_exactly_its_actions_for_an_outsider(pool: PgPool) {
    let w = World::new(pool).await;
    let page_url = |p: Uuid| format!("/api/workspaces/{}/pages/{p}", w.ws);
    for (level, can_comment, can_edit, can_manage) in [("read", false, false, false), ("comment", true, false, false), ("edit", true, true, false), ("manage", true, true, true)] {
        let who = user(&w.pool, &format!("lvl-{level}")).await;
        // Chaque niveau a ses pages : racine partagée S et sous-page D (la corbeille détruit).
        let s = page(&w.pool, w.ws, w.ana, &format!("S {level}"), None).await;
        let d = page(&w.pool, w.ws, w.ana, &format!("D {level}"), Some(s)).await;
        let d2 = page(&w.pool, w.ws, w.ana, &format!("D2 {level}"), Some(s)).await;
        w.share(s, who, level, true).await;
        for p in [s, d] {
            assert_eq!(w.get_page(who, p).await, 200, "{level} reads S and D");
            assert_eq!(w.collab(who, p).await.1["level"], level);
            let (cs, body) = w.comment(who, p).await;
            assert_eq!(cs == 200, can_comment, "{level} comment on {p}: {cs} {body}");
            if can_comment {
                let id = body["id"].as_str().unwrap().to_string();
                assert_eq!(w.status(who, "POST", format!("/api/comments/{id}/resolve"), Some(json!({ "resolved": true }))).await, 204, "same rule to resolve as to comment");
            }
            let edit = w.status(who, "PUT", page_url(p), Some(json!({ "title": format!("T {level}") }))).await;
            assert_eq!(edit == 200, can_edit, "{level} edit {p}");
            let share = w.status(who, "GET", format!("{}/permissions", page_url(p)), None).await;
            assert_eq!(share == 200, can_manage, "{level} lists shares of {p}: {share}");
            let put = w.status(who, "PUT", format!("{}/permissions", page_url(p)), Some(json!({ "user_id": w.zed, "role": "read" }))).await;
            assert_eq!(put == 204, can_manage, "{level} changes shares of {p}");
            w.status(w.ana, "PUT", format!("{}/permissions", page_url(p)), Some(json!({ "user_id": w.zed, "role": "none" }))).await;
        }
        // Les pages voisines et le parent restent invisibles.
        assert_eq!(w.get_page(who, w.top).await, 404);
        assert_eq!(w.get_page(who, w.other).await, 404);
        // G7 : mettre à la corbeille la page partagée elle-même demande la gestion ; une sous-page, la modification.
        let trash_d = w.status(who, "DELETE", page_url(d2), None).await;
        assert_eq!(trash_d == 204, can_edit, "{level} trashes a sub-page");
        let trash_s = w.status(who, "DELETE", page_url(s), None).await;
        assert_eq!(trash_s == 204, can_manage, "{level} trashes the shared root: {trash_s}");
    }
}

#[sqlx::test]
async fn an_outsider_can_be_denied_on_a_sub_page_and_scope_is_per_grant(pool: PgPool) {
    let w = World::new(pool).await;
    // Refus explicite sur une sous-page d'un arbre partagé (personne extérieure), sous-pages comprises.
    w.share(w.child, w.gus, "deny", true).await;
    assert_eq!(w.get_page(w.gus, w.shared).await, 200);
    for p in [w.child, w.grand] {
        assert_eq!(w.get_page(w.gus, p).await, 404);
        assert_eq!(w.collab(w.gus, p).await.0, 404, "the collaboration check agrees");
        assert_eq!(w.call(w.gus, "GET", &format!("/api/pages/{p}/revisions"), None).await.0, 404);
        assert_eq!(w.call(w.gus, "GET", &format!("/api/workspaces/{}/comments?kind=page&id={p}", w.ws), None).await.0, 404);
    }
    assert_eq!(w.visible_titles(w.gus).await, vec!["Shared root"]);
    let (_, shared) = w.call(w.gus, "GET", "/api/shared", None).await;
    assert_eq!(shared.as_array().unwrap().len(), 1);
    let (_, found) = w.call(w.gus, "GET", &format!("/api/workspaces/{}/search?q=needle", w.ws), None).await;
    assert_eq!(found["results"].as_array().unwrap().len(), 1);
    // Refus de cette page seulement : les sous-pages retombent sur le partage de la racine.
    w.share(w.child, w.gus, "deny", false).await;
    assert_eq!(w.get_page(w.gus, w.child).await, 404);
    assert_eq!(w.get_page(w.gus, w.grand).await, 200);
    // Un partage plus bas relève un refus plus haut (le plus proche gagne).
    w.share(w.grand, w.gus, "read", true).await;
    w.share(w.shared, w.gus, "deny", true).await;
    assert_eq!(w.get_page(w.gus, w.shared).await, 404);
    assert_eq!(w.get_page(w.gus, w.grand).await, 200);
    // Retirer le refus rend l'accès.
    w.share(w.shared, w.gus, "none", true).await;
    w.share(w.child, w.gus, "none", true).await;
    assert_eq!(w.get_page(w.gus, w.shared).await, 404, "no grant left on the shared root");
    assert_eq!(w.get_page(w.gus, w.grand).await, 200);
    // La liste des partages dit le niveau précis et garde les noms historiques.
    let (_, perms) = w.call(w.ana, "GET", &format!("/api/workspaces/{}/pages/{}/permissions", w.ws, w.grand), None).await;
    let mine = perms.as_array().unwrap().iter().find(|p| p["user_id"] == json!(w.gus)).unwrap();
    assert_eq!((mine["level"].as_str(), mine["role"].as_str()), (Some("read"), Some("viewer")));
}

#[sqlx::test]
async fn manage_lets_a_member_or_a_guest_share_and_nothing_more_than_that(pool: PgPool) {
    let w = World::new(pool).await;
    let perm = |p: Uuid| format!("/api/workspaces/{}/pages/{p}/permissions", w.ws);
    // Membre ordinaire : pas de gestion sans le niveau.
    assert_eq!(w.status(w.ben, "PUT", perm(w.shared), Some(json!({ "user_id": w.zed, "role": "read" }))).await, 403);
    w.share(w.shared, w.ben, "manage", true).await;
    assert_eq!(w.status(w.ben, "PUT", perm(w.child), Some(json!({ "user_id": w.zed, "role": "comment" }))).await, 204);
    assert_eq!(w.get_page(w.zed, w.grand).await, 200);
    // Il ne peut pas sortir de la page : le parent reste fermé à zed.
    assert_eq!(w.get_page(w.zed, w.shared).await, 404);
    w.share(w.child, w.zed, "none", true).await;
    // Invité gestionnaire : donne accès à une autre personne, et retire cet accès.
    w.share(w.shared, w.gus, "manage", true).await;
    assert_eq!(w.status(w.gus, "PUT", perm(w.shared), Some(json!({ "user_id": w.zed, "role": "edit" }))).await, 204);
    assert_eq!(w.collab(w.zed, w.grand).await.1["can_write"], true);
    assert_eq!(w.status(w.gus, "PUT", perm(w.shared), Some(json!({ "user_id": w.zed, "role": "none" }))).await, 204);
    assert_eq!(w.get_page(w.zed, w.grand).await, 404);
    // Mais pas ailleurs que sur ce qu'il gère.
    assert_eq!(w.status(w.gus, "PUT", perm(w.other), Some(json!({ "user_id": w.zed, "role": "read" }))).await, 404);
    // Valeur inconnue refusée.
    assert_eq!(w.status(w.gus, "PUT", perm(w.shared), Some(json!({ "user_id": w.zed, "role": "owner" }))).await, 400);
    // Anciens clients : `viewer` / `editor` / `blocked` fonctionnent toujours.
    for (old, new) in [("viewer", "comment"), ("editor", "edit"), ("blocked", "deny")] {
        w.share(w.shared, w.zed, old, true).await;
        let stored: String = sqlx::query_scalar("SELECT role FROM page_permissions WHERE page_id = $1 AND user_id = $2").bind(w.shared).bind(w.zed).fetch_one(&w.pool).await.unwrap();
        assert_eq!(stored, new);
    }
}

#[sqlx::test]
async fn a_read_guest_cannot_comment_but_a_comment_guest_can_resolve(pool: PgPool) {
    let w = World::new(pool).await;
    // G2 : gus a `viewer` (= commentaire) ; une personne en lecture seule ne commente plus ni ne résout.
    let (s, c) = w.comment(w.gus, w.child).await;
    assert_eq!(s, 200, "{c}");
    let id = c["id"].as_str().unwrap();
    assert_eq!(w.status(w.gus, "POST", format!("/api/comments/{id}/resolve"), Some(json!({ "resolved": true }))).await, 204);
    w.share(w.shared, w.gus, "read", true).await;
    assert_eq!(w.comment(w.gus, w.child).await.0, 403);
    assert_eq!(w.status(w.gus, "POST", format!("/api/comments/{id}/resolve"), Some(json!({ "resolved": false }))).await, 403);
    // Les commentaires restent lisibles.
    assert_eq!(w.call(w.gus, "GET", &format!("/api/workspaces/{}/comments?kind=page&id={}", w.ws, w.child), None).await.0, 200);
    // Membre : un partage en lecture lui retire aussi le commentaire sur cette page.
    w.share(w.child, w.ben, "read", true).await;
    assert_eq!(w.comment(w.ben, w.grand).await.0, 403);
    assert_eq!(w.comment(w.ben, w.shared).await.0, 200);
}

#[sqlx::test]
async fn moving_a_page_reports_who_gains_or_loses_access_and_the_preview_agrees(pool: PgPool) {
    let w = World::new(pool).await;
    let url = format!("/api/workspaces/{}/pages/{}", w.ws, w.child);
    let (s, preview) = w.call(w.ana, "GET", &format!("{url}/move-preview?parent_id={}", w.other), None).await;
    assert_eq!(s, 200, "{preview}");
    let lost = |v: &Value| -> HashMap<String, (String, String)> {
        v["changes"].as_array().unwrap().iter().map(|c| (c["name"].as_str().unwrap().to_string(), (c["before"].as_str().unwrap().to_string(), c["after"].as_str().unwrap().to_string()))).collect()
    };
    assert_eq!(lost(&preview), HashMap::from([("gus".to_string(), ("comment".to_string(), "none".to_string())), ("edi".to_string(), ("edit".to_string(), "none".to_string()))]));
    // Vers une page déjà partagée avec gus en lecture : son niveau change, rien d'autre.
    w.share(w.other, w.gus, "read", true).await;
    let (_, preview) = w.call(w.ana, "GET", &format!("{url}/move-preview?parent_id={}", w.other), None).await;
    assert_eq!(lost(&preview)["gus"], ("comment".to_string(), "read".to_string()));
    // Le déplacement lui-même renvoie le même constat et prévient la collaboration.
    let (s, moved) = w.call(w.ana, "PUT", &url, Some(json!({ "parent_id": w.other }))).await;
    assert_eq!(s, 200, "{moved}");
    assert_eq!(lost(&json!({ "changes": moved["access_changes"] })), lost(&preview));
    // Un partage sans changement ne remonte rien : sous le même parent, le déplacement est neutre.
    let (_, noop) = w.call(w.ana, "GET", &format!("{url}/move-preview?parent_id={}", w.other), None).await;
    assert_eq!(noop["changes"], json!([]));
    // Lecture seule : pas d'aperçu.
    assert_eq!(w.call(w.gus, "GET", &format!("/api/workspaces/{}/pages/{}/move-preview", w.ws, w.grand), None).await.0, 403);
}

#[sqlx::test]
async fn share_changes_are_recorded_in_the_page_activity(pool: PgPool) {
    let w = World::new(pool).await;
    w.share(w.child, w.zed, "read", true).await;
    w.share(w.child, w.zed, "none", true).await;
    w.share(w.child, w.zed, "none", true).await; // rien à retirer : pas d'entrée
    assert_eq!(w.activity_kinds(w.child).await, vec!["share", "unshare"]);
}

#[sqlx::test]
async fn a_suspended_account_loses_every_grant_in_the_resolver(pool: PgPool) {
    let w = World::new(pool).await;
    assert_eq!(crate::access::page_level(&w.pool, w.edi, w.grand).await.unwrap().2, crate::access::level::EDIT);
    sqlx::query("UPDATE users SET is_suspended = TRUE WHERE id = $1").bind(w.edi).execute(&w.pool).await.unwrap();
    assert_eq!(crate::access::page_level(&w.pool, w.edi, w.grand).await.unwrap().2, crate::access::level::NONE);
    assert!(crate::access::visible_levels(&w.pool, w.edi, w.ws).await.unwrap().values().all(|l| *l == crate::access::level::NONE));
    // Membre suspendu : de même, et le serveur de collaboration est déjà refusé à l'authentification.
    sqlx::query("UPDATE users SET is_suspended = TRUE WHERE id = $1").bind(w.ben).execute(&w.pool).await.unwrap();
    assert_eq!(crate::access::page_level(&w.pool, w.ben, w.other).await.unwrap().2, crate::access::level::NONE);
    assert_eq!(w.collab(w.edi, w.grand).await.0, 401);
}

/// Règle de la version précédente (avant les niveaux) : (lecture, écriture, commentaire).
fn legacy_rights(member_docs: Option<u8>, grant: Option<&str>) -> (bool, bool, bool) {
    use crate::access::level;
    let admin = member_docs == Some(u8::MAX);
    if admin {
        return (true, true, true);
    }
    let (read, write) = match grant {
        Some("blocked") => (false, false),
        Some("viewer") => (true, false),
        Some("editor") => (true, true),
        _ => match member_docs {
            None | Some(level::NONE) => (false, false),
            Some(l) if l >= level::EDIT => (true, true),
            Some(_) => (true, false),
        },
    };
    // Commentaire : un membre selon le rôle de l'espace, une personne extérieure dès qu'elle lit.
    let comment = read && match member_docs { Some(d) => d >= level::COMMENT, None => true };
    (read, write, comment)
}

/// Migration sans élargissement : les droits effectifs d'un échantillon (personne x partage x relation)
/// sont les mêmes avant (règles historiques) et après (valeurs converties, nouvelle résolution),
/// sauf le commentaire de qui modifie déjà (il comprend désormais le commentaire).
#[sqlx::test]
async fn migrating_existing_shares_changes_no_effective_right(pool: PgPool) {
    use crate::access::level;
    let w = World::new(pool).await;
    // Rôles personnalisés : docs lecture, docs commentaire, docs modification.
    let mut people: Vec<(Uuid, Option<u8>)> = vec![(w.gus, None)];
    for (name, docs) in [("r", "read"), ("c", "comment"), ("e", "edit"), ("n", "none")] {
        let u = user(&w.pool, &format!("mig-{name}")).await;
        let role_id: Uuid = sqlx::query_scalar("INSERT INTO workspace_roles (workspace_id, name, permissions) VALUES ($1, $2, $3) RETURNING id")
            .bind(w.ws).bind(format!("role-{name}")).bind(json!({ "docs": docs })).fetch_one(&w.pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role, role_id) VALUES ($1, $2, 'member', $3)").bind(w.ws).bind(u.to_string()).bind(role_id).execute(&w.pool).await.unwrap();
        people.push((u, Some(level::parse(docs).unwrap())));
    }
    people.push((w.ben, Some(level::EDIT)));
    people.push((w.ana, Some(u8::MAX))); // propriétaire
    // Partages historiques posés à la main (valeurs d'avant la migration).
    sqlx::query("DELETE FROM page_permissions").execute(&w.pool).await.unwrap();
    let grants: [(Option<&str>, Option<&str>, Option<&str>); 6] = [
        (Some("viewer"), None, None),
        (Some("editor"), None, None),
        (Some("blocked"), None, None),
        (Some("viewer"), Some("editor"), None),
        (Some("editor"), Some("blocked"), Some("viewer")),
        (None, None, None),
    ];
    // Chaque personne reçoit, selon son rang, une combinaison sur (shared, child, grand) ; inherit alterné.
    let mut expected = Vec::new();
    for (i, (u, docs)) in people.iter().enumerate() {
        let (g_shared, g_child, g_grand) = grants[i % grants.len()];
        for (page, g, inherit) in [(w.shared, g_shared, i % 2 == 0), (w.child, g_child, true), (w.grand, g_grand, false)] {
            if let Some(g) = g {
                // Un refus n'existait pas pour une personne extérieure.
                if docs.is_none() && g == "blocked" {
                    continue;
                }
                sqlx::query("INSERT INTO page_permissions (page_id, user_id, role, inherit) VALUES ($1, $2, $3, $4)").bind(page).bind(u).bind(g).bind(inherit).execute(&w.pool).await.unwrap();
            }
        }
        expected.push((*u, *docs));
    }
    // Droits d'avant, calculés par la règle historique sur l'arbre (partage le plus proche).
    let rows: Vec<(Uuid, Uuid, String, bool)> = sqlx::query_as("SELECT user_id, page_id, role, inherit FROM page_permissions").fetch_all(&w.pool).await.unwrap();
    let parent: HashMap<Uuid, Option<Uuid>> = HashMap::from([(w.top, None), (w.shared, Some(w.top)), (w.child, Some(w.shared)), (w.grand, Some(w.child)), (w.other, None)]);
    let nearest = |u: Uuid, p: Uuid| -> Option<String> {
        let mut cur = Some(p);
        while let Some(c) = cur {
            if let Some((_, _, r, inh)) = rows.iter().find(|(ru, rp, _, _)| *ru == u && *rp == c) {
                if c == p || *inh {
                    return Some(r.clone());
                }
            }
            cur = parent[&c];
        }
        None
    };
    let mut before = HashMap::new();
    for (u, docs) in &expected {
        for p in [w.top, w.shared, w.child, w.grand, w.other] {
            before.insert((*u, p), legacy_rights(*docs, nearest(*u, p).as_deref()));
        }
    }
    // La migration, telle que livrée.
    sqlx::raw_sql(include_str!("../migrations/20261215120000_page_access_levels.sql")).execute(&w.pool).await.unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM page_permissions WHERE role IN ('viewer', 'editor', 'blocked')").fetch_one(&w.pool).await.unwrap();
    assert_eq!(left, 0, "every legacy value is converted");
    let mut checked = 0;
    for ((u, p), (read, write, comment)) in before {
        let (_, _, lv) = crate::access::page_level(&w.pool, u, p).await.unwrap();
        assert_eq!(lv >= level::READ, read, "read: user {u} page {p}");
        assert_eq!(lv >= level::EDIT, write, "write: user {u} page {p}");
        // Pas de commentaire en plus pour qui ne pouvait que lire ; qui modifie comprend le commentaire.
        assert!(!(lv >= level::COMMENT && !comment && !write), "comment widened: user {u} page {p}");
        assert!(!(comment && lv < level::COMMENT), "comment lost: user {u} page {p}");
        checked += 1;
    }
    assert_eq!(checked, 7 * 5);
    // Le résultat de la migration ne contient plus de niveau « gestion » : jamais donné d'office.
    let manage: i64 = sqlx::query_scalar("SELECT count(*) FROM page_permissions WHERE role = 'manage'").fetch_one(&w.pool).await.unwrap();
    assert_eq!(manage, 0);
}

/// Une seule règle : la résolution d'une page et celle de tout l'espace donnent le même niveau
/// sur des arbres, des partages (niveaux, refus, étendues) et des rôles tirés au hasard.
#[sqlx::test]
async fn the_single_page_and_the_bulk_resolver_agree_on_random_trees(pool: PgPool) {
    use crate::access::level;
    let w = World::new(pool).await;
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut rnd = move |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let mut pages = vec![w.top, w.shared, w.child, w.grand, w.other];
    for i in 0..25 {
        let parent = if rnd(5) == 0 { None } else { Some(pages[rnd(pages.len() as u64) as usize]) };
        pages.push(page(&w.pool, w.ws, w.ana, &format!("rnd {i}"), parent).await);
    }
    // Quelques pages personnelles.
    for _ in 0..3 {
        let p = pages[rnd(pages.len() as u64) as usize];
        sqlx::query("UPDATE pages_all SET private_to = $2 WHERE id = $1").bind(p).bind(if rnd(2) == 0 { w.ben } else { w.ana }).execute(&w.pool).await.unwrap();
    }
    sqlx::query("DELETE FROM page_permissions").execute(&w.pool).await.unwrap();
    let values = ["read", "comment", "edit", "manage", "deny", "viewer", "editor", "blocked", "bogus"];
    let people = [w.ana, w.ben, w.gus, w.edi, w.zed];
    for p in &pages {
        for u in people {
            if rnd(4) == 0 {
                sqlx::query("INSERT INTO page_permissions (page_id, user_id, role, inherit) VALUES ($1, $2, $3, $4)").bind(p).bind(u).bind(values[rnd(values.len() as u64) as usize]).bind(rnd(2) == 0).execute(&w.pool).await.unwrap();
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for u in people {
        let bulk = crate::access::visible_levels(&w.pool, u, w.ws).await.unwrap();
        for p in &pages {
            let (_, _, single) = crate::access::page_level(&w.pool, u, *p).await.unwrap();
            assert_eq!(bulk[p], single, "user {u} page {p}");
            seen.insert(single);
        }
    }
    for l in [level::NONE, level::READ, level::EDIT] {
        assert!(seen.contains(&l), "the generated space exercises level {l}");
    }
}
