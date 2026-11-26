//! Fichiers envoyés (ADR 0022, phase 5, G1) : `/uploads/{nom}` suit l'accès à la page, la tâche ou
//! l'espace auquel le fichier est rattaché ; avatars et favicon restent publics ; coupure immédiate.

use axum::{body::Body, http::Request};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use time::{Duration, OffsetDateTime};
use tower::ServiceExt;
use tower_sessions::session::{Id, Record};
use tower_sessions::SessionStore;
use uuid::Uuid;

use crate::access::SESSION_USER_KEY;
use crate::ws::{guest_step, GuestStep};

struct W {
    app: axum::Router,
    store: crate::session_store::PgSessionStore,
    pool: PgPool,
    ws: Uuid,
    ana: Uuid,
    ben: Uuid,
    gus: Uuid,
    zed: Uuid,
    shared: Uuid,
    child: Uuid,
    other: Uuid,
}

struct Res {
    status: u16,
    headers: axum::http::HeaderMap,
    body: Vec<u8>,
}

async fn user(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id").bind(format!("{name}@example.org")).bind(name).fetch_one(pool).await.unwrap()
}

async fn page(pool: &PgPool, ws: Uuid, by: Uuid, title: &str, parent: Option<Uuid>, content: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO pages (workspace_id, title, content, created_by, parent_id) VALUES ($1, $2, $3, $4, $5) RETURNING id")
        .bind(ws)
        .bind(title)
        .bind(content)
        .bind(by)
        .bind(parent)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn disk(name: &str, data: &[u8]) {
    std::fs::create_dir_all(crate::upload::UPLOAD_DIR).unwrap();
    std::fs::write(std::path::Path::new(crate::upload::UPLOAD_DIR).join(name), data).unwrap();
}

fn fresh_name(ext: &str) -> String {
    format!("t-{}.{ext}", Uuid::new_v4())
}

impl W {
    async fn new(pool: PgPool) -> W {
        let store = crate::session_store::PgSessionStore::new(pool.clone());
        store.migrate().await.unwrap();
        let app = crate::app(crate::state::AppState::new(pool.clone(), "http://localhost".into()));
        let (ana, ben, gus, zed) = (user(&pool, "ana").await, user(&pool, "ben").await, user(&pool, "gus").await, user(&pool, "zed").await);
        let ws: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('Team') RETURNING id").fetch_one(&pool).await.unwrap();
        for (u, role) in [(ana, "owner"), (ben, "member")] {
            sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3)").bind(ws).bind(u.to_string()).bind(role).execute(&pool).await.unwrap();
        }
        let shared = page(&pool, ws, ana, "Shared", None, "<p>x</p>").await;
        let child = page(&pool, ws, ana, "Child", Some(shared), "<p>y</p>").await;
        let other = page(&pool, ws, ana, "Other", None, "<p>z</p>").await;
        sqlx::query("INSERT INTO page_permissions (page_id, user_id, role, inherit, created_by) VALUES ($1, $2, 'read', true, $3)").bind(shared).bind(gus).bind(ana).execute(&pool).await.unwrap();
        W { app, store, pool, ws, ana, ben, gus, zed, shared, child, other }
    }

    async fn session(&self, user: Uuid) -> String {
        let mut data = HashMap::new();
        data.insert(SESSION_USER_KEY.to_string(), json!(user));
        let mut record = Record { id: Id::default(), data, expiry_date: OffsetDateTime::now_utc() + Duration::days(1) };
        self.store.create(&mut record).await.unwrap();
        record.id.to_string()
    }

    async fn send(&self, who: Option<Uuid>, method: &str, path: &str, extra: &[(&str, &str)], body: Body, ctype: Option<&str>) -> Res {
        let mut req = Request::builder().method(method).uri(path).header("origin", "http://localhost");
        if let Some(u) = who {
            req = req.header("cookie", format!("looty_session={}", self.session(u).await));
        }
        if let Some(c) = ctype {
            req = req.header("content-type", c);
        }
        for (k, v) in extra {
            req = req.header(*k, *v);
        }
        let res = self.app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = res.status().as_u16();
        let headers = res.headers().clone();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec();
        Res { status, headers, body }
    }

    async fn get(&self, who: Option<Uuid>, path: &str) -> Res {
        self.send(who, "GET", path, &[], Body::empty(), None).await
    }

    async fn status(&self, who: Option<Uuid>, name: &str) -> u16 {
        self.get(who, &format!("/uploads/{name}")).await.status
    }

    /// Envoi par l'API (multipart), comme l'éditeur.
    async fn upload(&self, who: Uuid, query: &str, data: &[u8]) -> (u16, String) {
        let boundary = "XBOUNDARYX";
        let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"pic.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let r = self.send(Some(who), "POST", &format!("/api/upload{query}"), &[], Body::from(body), Some(&format!("multipart/form-data; boundary={boundary}"))).await;
        let v: Value = serde_json::from_slice(&r.body).unwrap_or(Value::Null);
        (r.status, v["url"].as_str().unwrap_or("").trim_start_matches("/uploads/").to_string())
    }

    async fn revoke(&self, who: Uuid) {
        sqlx::query("DELETE FROM page_permissions WHERE user_id = $1").bind(who).execute(&self.pool).await.unwrap();
    }
}

#[sqlx::test]
async fn an_upload_bound_to_a_page_follows_the_page_access(pool: PgPool) {
    let w = W::new(pool).await;
    let (s, name) = w.upload(w.ana, &format!("?page={}", w.child), b"PNGDATA").await;
    assert_eq!(s, 200);
    // Propriétaire, membre, invité (partage hérité) : oui ; sans partage : 404 ; anonyme : 401.
    assert_eq!(w.status(Some(w.ana), &name).await, 200);
    assert_eq!(w.status(Some(w.ben), &name).await, 200);
    assert_eq!(w.status(Some(w.gus), &name).await, 200);
    assert_eq!(w.status(Some(w.zed), &name).await, 404);
    assert_eq!(w.status(None, &name).await, 401);
    // Retrait de l'accès : coupé tout de suite, sans attendre un cache ni une expiration.
    w.revoke(w.gus).await;
    assert_eq!(w.status(Some(w.gus), &name).await, 404);
    // Un refus explicite sur la page coupe aussi un membre.
    sqlx::query("INSERT INTO page_permissions (page_id, user_id, role, inherit, created_by) VALUES ($1, $2, 'deny', true, $3)").bind(w.child).bind(w.ben).bind(w.ana).execute(&w.pool).await.unwrap();
    assert_eq!(w.status(Some(w.ben), &name).await, 404);
    assert_eq!(w.status(Some(w.ana), &name).await, 200, "the owner keeps access");
}

#[sqlx::test]
async fn uploading_into_a_page_needs_access_to_that_page(pool: PgPool) {
    let w = W::new(pool).await;
    assert_eq!(w.upload(w.zed, &format!("?page={}", w.shared), b"x").await.0, 404);
    assert_eq!(w.upload(w.gus, &format!("?page={}", w.other), b"x").await.0, 404);
    assert_eq!(w.upload(w.zed, &format!("?workspace={}", w.ws), b"x").await.0, 404);
    assert_eq!(w.upload(w.ben, &format!("?workspace={}", w.ws), b"x").await.0, 200);
}

#[sqlx::test]
async fn a_file_without_a_page_stays_with_its_uploader_then_follows_the_pages_that_cite_it(pool: PgPool) {
    let w = W::new(pool).await;
    let (_, name) = w.upload(w.ben, "", b"mine").await;
    assert_eq!(w.status(Some(w.ben), &name).await, 200);
    assert_eq!(w.status(Some(w.gus), &name).await, 404);
    // Une page partagée cite le fichier : l'invité le lit, pas la personne sans accès.
    sqlx::query("UPDATE pages SET content = $2 WHERE id = $1").bind(w.child).bind(format!("<img src=\"/uploads/{name}\">")).execute(&w.pool).await.unwrap();
    assert_eq!(w.status(Some(w.gus), &name).await, 200);
    assert_eq!(w.status(Some(w.zed), &name).await, 404);
    // La citation est retirée : l'accès aussi (la page est revérifiée).
    sqlx::query("UPDATE pages SET content = '<p>none</p>' WHERE id = $1").bind(w.child).execute(&w.pool).await.unwrap();
    assert_eq!(w.status(Some(w.gus), &name).await, 404);
}

#[sqlx::test]
async fn a_legacy_file_with_no_record_is_attributed_by_its_citations(pool: PgPool) {
    let w = W::new(pool).await;
    let name = fresh_name("png");
    disk(&name, b"old");
    assert_eq!(w.status(Some(w.gus), &name).await, 404, "unknown file: nobody but admins");
    let secret = page(&w.pool, w.ws, w.ana, "Secret", None, &format!("<img src=\"/uploads/{name}\">")).await;
    assert_eq!(w.status(Some(w.gus), &name).await, 404, "cited by a page the guest cannot read");
    assert_eq!(w.status(Some(w.ben), &name).await, 200);
    sqlx::query("INSERT INTO page_permissions (page_id, user_id, role, inherit, created_by) VALUES ($1, $2, 'read', false, $3)").bind(secret).bind(w.gus).bind(w.ana).execute(&w.pool).await.unwrap();
    assert_eq!(w.status(Some(w.gus), &name).await, 200);
    let _ = std::fs::remove_file(std::path::Path::new(crate::upload::UPLOAD_DIR).join(&name));
}

#[sqlx::test]
async fn the_backfill_migration_binds_cited_files_and_marks_avatars_public(pool: PgPool) {
    let w = W::new(pool).await;
    let (cited, avatar) = (fresh_name("png"), fresh_name("png"));
    sqlx::query("UPDATE pages SET content = $2 WHERE id = $1").bind(w.other).bind(format!("<img src=\"/uploads/{cited}\">")).execute(&w.pool).await.unwrap();
    sqlx::query("UPDATE users SET avatar_url = $2 WHERE id = $1").bind(w.zed).bind(format!("/uploads/{avatar}")).execute(&w.pool).await.unwrap();
    // Rejoue le remplissage de la migration sur ces données.
    let sql = include_str!("../migrations/20261216120000_upload_files.sql");
    sqlx::raw_sql(sql).execute(&w.pool).await.unwrap();
    let bound: Option<Uuid> = sqlx::query_scalar("SELECT page_id FROM upload_files WHERE name = $1").bind(&cited).fetch_one(&w.pool).await.unwrap();
    assert_eq!(bound, Some(w.other));
    let kind: String = sqlx::query_scalar("SELECT kind FROM upload_files WHERE name = $1").bind(&avatar).fetch_one(&w.pool).await.unwrap();
    assert_eq!(kind, "public");
}

#[sqlx::test]
async fn avatars_stay_public_and_other_files_cannot_be_made_public_by_strangers(pool: PgPool) {
    let w = W::new(pool).await;
    let (_, name) = w.upload(w.ben, "", b"avatar").await;
    let body = json!({ "name": "Ben", "avatar_url": format!("/uploads/{name}") });
    let put = |who: Uuid, b: Value| {
        let w = &w;
        async move { w.send(Some(who), "PUT", "/api/user/profile", &[], Body::from(b.to_string()), Some("application/json")).await.status }
    };
    // Ce n'est pas le fichier de zed.
    let s = put(w.zed, body.clone()).await;
    assert!(s == 400 || s == 404, "{s}");
    assert_eq!(w.status(None, &name).await, 401);
    assert_eq!(put(w.ben, body).await, 204);
    let r = w.get(None, &format!("/uploads/{name}")).await;
    assert_eq!(r.status, 200, "an avatar is public");
    assert!(r.headers["cache-control"].to_str().unwrap().starts_with("public"));
}

#[sqlx::test]
async fn public_link_pages_serve_their_own_images_with_the_link_token(pool: PgPool) {
    let w = W::new(pool).await;
    let (_, name) = w.upload(w.ana, &format!("?page={}", w.other), b"img").await;
    let token = Uuid::new_v4();
    sqlx::query("UPDATE pages SET content = $2, share_token = $3, is_public = true WHERE id = $1").bind(w.other).bind(format!("<img src=\"/uploads/{name}\">")).bind(token).execute(&w.pool).await.unwrap();
    let public = w.get(None, &format!("/api/public/pages/{token}")).await;
    assert_eq!(public.status, 200);
    let v: Value = serde_json::from_slice(&public.body).unwrap();
    let content = v["content"].as_str().unwrap();
    assert!(content.contains(&format!("/uploads/{name}?share={token}")), "{content}");
    assert_eq!(w.get(None, &format!("/uploads/{name}?share={token}")).await.status, 200);
    assert_eq!(w.status(None, &name).await, 401, "no token, no file");
    assert_eq!(w.get(None, &format!("/uploads/{name}?share={}", Uuid::new_v4())).await.status, 401, "another token");
    // Un fichier d'une autre page ne passe pas avec ce jeton.
    let (_, foreign) = w.upload(w.ana, &format!("?page={}", w.shared), b"other").await;
    assert_eq!(w.get(None, &format!("/uploads/{foreign}?share={token}")).await.status, 401);
    // Lien public retiré : plus de fichier.
    sqlx::query("UPDATE pages SET is_public = false, share_token = NULL WHERE id = $1").bind(w.other).execute(&w.pool).await.unwrap();
    assert_eq!(w.get(None, &format!("/uploads/{name}?share={token}")).await.status, 401);
}

#[sqlx::test]
async fn task_attachments_follow_the_board_access(pool: PgPool) {
    let w = W::new(pool).await;
    let board: Uuid = sqlx::query_scalar("INSERT INTO boards (workspace_id, name) VALUES ($1, 'B') RETURNING id").bind(w.ws).fetch_one(&w.pool).await.unwrap();
    let col: Uuid = sqlx::query_scalar("INSERT INTO board_columns (board_id, name, position) VALUES ($1, 'C', 0) RETURNING id").bind(board).fetch_one(&w.pool).await.unwrap();
    let task: Uuid = sqlx::query_scalar("INSERT INTO tasks (column_id, title, position) VALUES ($1, 'T', 0) RETURNING id").bind(col).fetch_one(&w.pool).await.unwrap();
    let boundary = "XBX";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.pdf\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
    body.extend_from_slice(b"%PDF-1.4 hello");
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let r = w.send(Some(w.ana), "POST", &format!("/api/boards/tasks/{task}/attachments"), &[], Body::from(body), Some(&format!("multipart/form-data; boundary={boundary}"))).await;
    assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
    let v: Value = serde_json::from_slice(&r.body).unwrap();
    let name = v["file_path"].as_str().unwrap().rsplit('/').next().unwrap().to_string();
    assert_eq!(w.status(Some(w.ben), &name).await, 200);
    assert_eq!(w.status(Some(w.gus), &name).await, 404, "a guest has no board rights");
    assert_eq!(w.status(Some(w.zed), &name).await, 404);
    assert_eq!(w.status(None, &name).await, 401);
}

#[sqlx::test]
async fn media_support_ranges_etag_and_private_caching(pool: PgPool) {
    let w = W::new(pool).await;
    let (_, name) = w.upload(w.ana, &format!("?page={}", w.shared), b"0123456789").await;
    let path = format!("/uploads/{name}");
    let r = w.send(Some(w.gus), "GET", &path, &[("range", "bytes=2-4")], Body::empty(), None).await;
    assert_eq!((r.status, r.body.as_slice()), (206, &b"234"[..]));
    assert_eq!(r.headers["content-range"], "bytes 2-4/10");
    assert_eq!(r.headers["accept-ranges"], "bytes");
    let r = w.send(Some(w.gus), "GET", &path, &[("range", "bytes=-3")], Body::empty(), None).await;
    assert_eq!(r.body, b"789");
    assert_eq!(w.send(Some(w.gus), "GET", &path, &[("range", "bytes=50-")], Body::empty(), None).await.status, 416);
    let full = w.get(Some(w.gus), &path).await;
    assert_eq!(full.status, 200);
    let cc = full.headers["cache-control"].to_str().unwrap();
    assert!(cc.contains("private") && cc.contains("no-cache") && !cc.contains("immutable"), "{cc}");
    assert!(full.headers["vary"].to_str().unwrap().contains("Cookie"));
    let etag = full.headers["etag"].to_str().unwrap().to_string();
    assert_eq!(w.send(Some(w.gus), "GET", &path, &[("if-none-match", &etag)], Body::empty(), None).await.status, 304);
    // Le 304 passe aussi par le contrôle d'accès.
    w.revoke(w.gus).await;
    assert_eq!(w.send(Some(w.gus), "GET", &path, &[("if-none-match", &etag)], Body::empty(), None).await.status, 404);
}

#[sqlx::test]
async fn signed_links_serve_clients_without_cookies_for_a_short_time(pool: PgPool) {
    let w = W::new(pool).await;
    let (_, name) = w.upload(w.ana, &format!("?page={}", w.other), b"secret").await;
    assert_eq!(w.get(Some(w.gus), &format!("/api/uploads/{name}/link")).await.status, 404, "no link without read access");
    let r = w.get(Some(w.ana), &format!("/api/uploads/{name}/link")).await;
    assert_eq!(r.status, 200);
    let url = serde_json::from_slice::<Value>(&r.body).unwrap()["url"].as_str().unwrap().to_string();
    assert_eq!(w.get(None, &url).await.status, 200);
    let (_, other) = w.upload(w.ana, &format!("?page={}", w.other), b"secret2").await;
    let sig = url.split("sig=").nth(1).unwrap();
    assert_eq!(w.get(None, &format!("/uploads/{other}?sig={sig}")).await.status, 401, "a link is for one file");
    assert_eq!(w.get(None, &format!("/uploads/{name}?sig=1.abcdef")).await.status, 401);
}

// --- Temps réel des invités (G8) ---

#[sqlx::test]
async fn a_guest_session_follows_grants_and_the_tree_live(pool: PgPool) {
    let w = W::new(pool).await;
    let mut set: HashSet<Uuid> = [w.shared, w.child].into_iter().collect();
    let ev = |v: Value| v.to_string();
    // Présence : traitée à part.
    assert_eq!(guest_step(&w.pool, &ev(json!({ "type": "USER_OFFLINE", "user_id": w.ben })), &mut set, w.ws, w.gus).await, GuestStep::Presence);
    // Événement sans lien avec ses pages : rien.
    assert_eq!(guest_step(&w.pool, &ev(json!({ "type": "TASK_CREATED" })), &mut set, w.ws, w.gus).await, GuestStep::Drop);
    assert_eq!(guest_step(&w.pool, &ev(json!({ "type": "PAGE_PERMISSIONS_UPDATED", "page_id": w.other })), &mut set, w.ws, w.gus).await, GuestStep::Drop);
    // Une nouvelle sous-page sous la page partagée arrive dans l'arbre (sans lien public pour qui lit seulement).
    let fresh = page(&w.pool, w.ws, w.ana, "Fresh", Some(w.shared), "<p>f</p>").await;
    let created = ev(json!({ "type": "PAGE_CREATED", "page": { "id": fresh, "share_token": "t", "title": "Fresh" } }));
    match guest_step(&w.pool, &created, &mut set, w.ws, w.gus).await {
        GuestStep::Send(t) => assert!(!t.contains("share_token"), "{t}"),
        other => panic!("{other:?}"),
    }
    assert!(set.contains(&fresh));
    // Une page hors de ses partages ne lui est pas transmise.
    let hidden = ev(json!({ "type": "PAGE_UPDATED", "page": { "id": w.other, "title": "Other" } }));
    assert_eq!(guest_step(&w.pool, &hidden, &mut set, w.ws, w.gus).await, GuestStep::Drop);
    // Un nouveau partage : l'ensemble est recalculé et le client recharge.
    sqlx::query("INSERT INTO page_permissions (page_id, user_id, role, inherit, created_by) VALUES ($1, $2, 'read', true, $3)").bind(w.other).bind(w.gus).bind(w.ana).execute(&w.pool).await.unwrap();
    let granted = ev(json!({ "type": "PAGE_PERMISSIONS_UPDATED", "page_id": w.other }));
    assert!(matches!(guest_step(&w.pool, &granted, &mut set, w.ws, w.gus).await, GuestStep::Send(_)));
    assert!(set.contains(&w.other));
    // Un partage retiré : la page sort de l'ensemble et le client recharge.
    sqlx::query("DELETE FROM page_permissions WHERE page_id = $1 AND user_id = $2").bind(w.other).bind(w.gus).execute(&w.pool).await.unwrap();
    assert!(matches!(guest_step(&w.pool, &granted, &mut set, w.ws, w.gus).await, GuestStep::Send(_)));
    assert!(!set.contains(&w.other));
    // Suppression d'une page de l'arbre.
    assert!(matches!(guest_step(&w.pool, &ev(json!({ "type": "PAGE_DELETED", "page_id": fresh })), &mut set, w.ws, w.gus).await, GuestStep::Send(_)));
    // Plus aucun accès à l'espace : la connexion se termine.
    w.revoke(w.gus).await;
    assert_eq!(guest_step(&w.pool, &granted, &mut set, w.ws, w.gus).await, GuestStep::Stop);
}

// --- Projets portés par une page : mêmes droits partout où on les atteint ---

#[sqlx::test]
async fn a_project_carried_by_a_page_follows_the_page_for_ai_context_and_study_links(pool: PgPool) {
    let w = W::new(pool).await;
    let project: Uuid = sqlx::query_scalar("INSERT INTO projects (workspace_id, kind, name) VALUES ($1, 'latex', 'Thesis') RETURNING id").bind(w.ws).fetch_one(&w.pool).await.unwrap();
    sqlx::query("UPDATE pages_all SET project_id = $2 WHERE id = $1").bind(w.other).bind(project).execute(&w.pool).await.unwrap();
    let gus = crate::access::AuthUser { id: w.gus, is_super_admin: false };
    let ben = crate::access::AuthUser { id: w.ben, is_super_admin: false };
    // `gus` n'a aucun partage sur la page `other` : le projet n'existe pas pour lui, ni en contexte IA.
    assert!(crate::projects::project_for_pool(&w.pool, &gus, project, crate::projects::Need::Read).await.is_err());
    assert!(crate::ai::check_context(&w.pool, &gus, "project", Some(project)).await.is_err());
    assert!(crate::ai::check_context(&w.pool, &ben, "project", Some(project)).await.is_ok());
    sqlx::query("INSERT INTO page_permissions (page_id, user_id, role, inherit, created_by) VALUES ($1, $2, 'read', false, $3)").bind(w.other).bind(w.gus).bind(w.ana).execute(&w.pool).await.unwrap();
    assert!(crate::projects::project_for_pool(&w.pool, &gus, project, crate::projects::Need::Read).await.is_ok());
    assert!(crate::projects::project_for_pool(&w.pool, &gus, project, crate::projects::Need::Write).await.is_err(), "read only");
    // Un membre dont la page est refusée perd aussi le projet.
    sqlx::query("INSERT INTO page_permissions (page_id, user_id, role, inherit, created_by) VALUES ($1, $2, 'deny', true, $3)").bind(w.other).bind(w.ben).bind(w.ana).execute(&w.pool).await.unwrap();
    assert!(crate::ai::check_context(&w.pool, &ben, "project", Some(project)).await.is_err());
}
