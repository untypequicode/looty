//! Présentations : commentaires ancrés, versions (état des diapositives) et duplication, vus de
//! l'extérieur (HTTP, session réelle) avec les niveaux d'accès des pages.

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
    deck: Uuid,
    doc: Uuid,
}

const STATE: &[u8] = &[1, 2, 3, 4, 5];

async fn user(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id").bind(format!("{name}@example.org")).bind(name).fetch_one(pool).await.unwrap()
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
        let deck: Uuid = sqlx::query_scalar(
            "INSERT INTO pages (workspace_id, title, created_by, default_view, slides_state, slides_count, slides_text) VALUES ($1, 'Deck', $2, 'slides', $3, 2, '## 1\nHello') RETURNING id",
        )
        .bind(ws)
        .bind(ana)
        .bind(STATE)
        .fetch_one(&pool)
        .await
        .unwrap();
        let doc: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, created_by) VALUES ($1, 'Doc', $2) RETURNING id").bind(ws).bind(ana).fetch_one(&pool).await.unwrap();
        let w = World { app, store, pool, ws, ana, ben, gus, edi, zed, deck, doc };
        for (page, who, role) in [(deck, gus, "viewer"), (deck, edi, "editor")] {
            let (s, b) = w.call(ana, "PUT", &format!("/api/workspaces/{ws}/pages/{page}/permissions"), Some(json!({ "user_id": who, "role": role, "inherit": true }))).await;
            assert_eq!(s, 204, "{b}");
        }
        w
    }

    async fn call(&self, who: Uuid, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let mut data = HashMap::new();
        data.insert(SESSION_USER_KEY.to_string(), json!(who));
        let mut record = Record { id: Id::default(), data, expiry_date: OffsetDateTime::now_utc() + Duration::days(1) };
        self.store.create(&mut record).await.unwrap();
        let mut req = Request::builder().method(method).uri(path).header("cookie", format!("looty_session={}", record.id)).header("origin", "http://localhost");
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

    async fn comment(&self, who: Uuid, page: Uuid, extra: Value) -> (u16, Value) {
        let mut body = json!({ "kind": "page", "id": page, "content": "<p>Looks good</p>" });
        body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        self.call(who, "POST", &format!("/api/workspaces/{}/comments", self.ws), Some(body)).await
    }
}

#[sqlx::test]
async fn comments_anchor_on_slides_and_elements_with_the_page_access_levels(pool: PgPool) {
    let w = World::new(pool).await;
    // Un membre commente un élément, puis une diapositive.
    let (s, el) = w.comment(w.ben, w.deck, json!({ "anchor": "el:slide-1:box_2" })).await;
    assert_eq!((s, el["anchor"].as_str()), (200, Some("el:slide-1:box_2")), "{el}");
    let (s, sl) = w.comment(w.ben, w.deck, json!({ "anchor": "slide:slide-1" })).await;
    assert_eq!((s, sl["anchor"].as_str()), (200, Some("slide:slide-1")));
    // Sans ancre : toute la présentation, comme avant.
    let (s, general) = w.comment(w.ben, w.deck, json!({})).await;
    assert_eq!((s, general["anchor"].is_null()), (200, true));
    // Ancre mal formée, ou sur une page qui n'est pas une présentation : refusée.
    for bad in ["el:a", "page:x", "slide:", "el:a:b:c", "slide:a b"] {
        assert_eq!(w.comment(w.ben, w.deck, json!({ "anchor": bad })).await.0, 400, "{bad}");
    }
    assert_eq!(w.comment(w.ben, w.doc, json!({ "anchor": "slide:s1" })).await.0, 400);
    // Une réponse reste dans le fil : l'ancre est celle du fil, jamais la sienne.
    let (s, reply) = w.comment(w.ben, w.deck, json!({ "parent_id": el["id"], "anchor": "slide:other" })).await;
    assert_eq!((s, reply["anchor"].is_null(), reply["parent_id"] == el["id"]), (200, true, true));
    // La liste renvoie les ancres ; les fils se résolvent comme ceux des pages.
    let (s, list) = w.call(w.ben, "GET", &format!("/api/workspaces/{}/comments?kind=page&id={}", w.ws, w.deck), None).await;
    assert_eq!(s, 200);
    let anchors: Vec<_> = list.as_array().unwrap().iter().map(|c| c["anchor"].as_str().map(str::to_string)).collect();
    assert_eq!(anchors, vec![Some("el:slide-1:box_2".into()), Some("slide:slide-1".into()), None, None]);
    let id = el["id"].as_str().unwrap();
    assert_eq!(w.call(w.ben, "POST", &format!("/api/comments/{id}/resolve"), Some(json!({ "resolved": true }))).await.0, 204);
    let resolved: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar("SELECT resolved_at FROM item_comments WHERE id = $1").bind(Uuid::parse_str(id).unwrap()).fetch_one(&w.pool).await.unwrap();
    assert!(resolved.is_some());
    // Niveaux d'accès : l'invité en modification commente, la personne sans partage ne voit ni ne commente rien.
    assert_eq!(w.comment(w.edi, w.deck, json!({ "anchor": "slide:slide-2" })).await.0, 200);
    let (s, _) = w.comment(w.zed, w.deck, json!({ "anchor": "slide:slide-1" })).await;
    assert!(s == 403 || s == 404, "{s}");
    let (s, _) = w.call(w.zed, "GET", &format!("/api/workspaces/{}/comments?kind=page&id={}", w.ws, w.deck), None).await;
    assert!(s == 403 || s == 404, "{s}");
}

#[sqlx::test]
async fn an_anchored_comment_notifies_the_author_and_mentions_with_the_anchor(pool: PgPool) {
    let w = World::new(pool).await;
    let (s, c) = w.comment(w.ben, w.deck, json!({ "anchor": "el:s1:e1" })).await;
    assert_eq!(s, 200, "{c}");
    let (kind, target, data): (String, String, Value) = sqlx::query_as("SELECT kind, target, data FROM notifications WHERE user_id = $1").bind(w.ana).fetch_one(&w.pool).await.unwrap();
    assert_eq!((kind.as_str(), target), ("comment", format!("page:{}", w.deck)));
    assert_eq!(data["anchor"], "el:s1:e1");
    // Pas de notification pour soi-même, ni pour la personne sans accès.
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM notifications WHERE user_id = ANY($1)").bind(vec![w.ben, w.zed]).fetch_one(&w.pool).await.unwrap();
    assert_eq!(n, 0);
}

#[sqlx::test]
async fn revisions_of_a_presentation_snapshot_the_slides_state(pool: PgPool) {
    let w = World::new(pool).await;
    let base = format!("/api/pages/{}/revisions", w.deck);
    // Version nommée : l'état des diapositives, pas celui (vide) du texte.
    let (s, made) = w.call(w.ana, "POST", &base, Some(json!({ "kind": "manual", "label": "Before review" }))).await;
    assert_eq!(s, 200, "{made}");
    let rev = made["id"].as_str().expect("revision id").to_string();
    let (s, one) = w.call(w.ben, "GET", &format!("{base}/{rev}"), None).await;
    assert_eq!(s, 200);
    use base64::Engine;
    assert_eq!(base64::engine::general_purpose::STANDARD.decode(one["yjs_state"].as_str().unwrap()).unwrap(), STATE);
    let (s, list) = w.call(w.gus, "GET", &base, None).await;
    assert_eq!((s, list[0]["label"].as_str(), list[0]["kind"].as_str(), list[0]["size"].as_i64()), (200, Some("Before review"), Some("manual"), Some(5)));
    // Instantané « avant restauration », puis nommer une version existante.
    let (s, before) = w.call(w.edi, "POST", &base, Some(json!({ "kind": "restore" }))).await;
    assert_eq!(s, 200);
    assert!(before["id"].is_string());
    assert_eq!(w.call(w.edi, "PATCH", &format!("{base}/{}", before["id"].as_str().unwrap()), Some(json!({ "label": "Good one" }))).await.0, 204);
    // Lecture seule : voit les versions mais n'en crée ni n'en nomme ; sans partage : rien.
    assert_eq!(w.call(w.gus, "POST", &base, Some(json!({ "kind": "manual" }))).await.0, 403);
    assert_eq!(w.call(w.gus, "PATCH", &format!("{base}/{rev}"), Some(json!({ "label": "x" }))).await.0, 403);
    let (s, _) = w.call(w.zed, "GET", &base, None).await;
    assert!(s == 403 || s == 404, "{s}");
    // Une présentation sans état enregistré n'a rien à photographier.
    let empty: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, created_by, default_view) VALUES ($1, 'Fresh', $2, 'slides') RETURNING id").bind(w.ws).bind(w.ana).fetch_one(&w.pool).await.unwrap();
    let (s, none) = w.call(w.ana, "POST", &format!("/api/pages/{empty}/revisions"), Some(json!({ "kind": "manual" }))).await;
    assert_eq!((s, none["id"].is_null()), (200, true));
}

#[sqlx::test]
async fn duplicating_a_presentation_copies_its_slides_and_only_presentations(pool: PgPool) {
    let w = World::new(pool).await;
    let url = format!("/api/workspaces/{}/pages", w.ws);
    let (s, copy) = w.call(w.ben, "POST", &url, Some(json!({ "title": "Deck (copy)", "copy_slides_from": w.deck }))).await;
    assert_eq!((s, copy["default_view"].as_str()), (201, Some("slides")), "{copy}");
    let (state, count, text, view): (Vec<u8>, i32, Option<String>, String) =
        sqlx::query_as("SELECT slides_state, slides_count, slides_text, default_view FROM pages WHERE id = $1").bind(Uuid::parse_str(copy["id"].as_str().unwrap()).unwrap()).fetch_one(&w.pool).await.unwrap();
    assert_eq!((state.as_slice(), count, text.as_deref(), view.as_str()), (STATE, 2, Some("## 1\nHello"), "slides"));
    // La source garde son état ; une page de texte ne se copie pas comme présentation ; l'extérieur ne copie pas.
    let src: Vec<u8> = sqlx::query_scalar("SELECT slides_state FROM pages WHERE id = $1").bind(w.deck).fetch_one(&w.pool).await.unwrap();
    assert_eq!(src, STATE);
    assert_eq!(w.call(w.ben, "POST", &url, Some(json!({ "copy_slides_from": w.doc }))).await.0, 404);
    let (s, _) = w.call(w.zed, "POST", &url, Some(json!({ "copy_slides_from": w.deck }))).await;
    assert!(s == 403 || s == 404, "{s}");
    let pages: i64 = sqlx::query_scalar("SELECT count(*) FROM pages WHERE workspace_id = $1").bind(w.ws).fetch_one(&w.pool).await.unwrap();
    assert_eq!(pages, 3, "refused copies must not leave a page behind");
    // Un lecteur invité (sans droit de création) ne duplique pas.
    let (s, _) = w.call(w.gus, "POST", &url, Some(json!({ "copy_slides_from": w.deck }))).await;
    assert!(s == 403 || s == 404, "{s}");
}
