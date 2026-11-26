//! Partage de pages, phases 3 et 4 (ADR 0022) : groupes d'instance comme cibles, annuaire limité,
//! demandes d'accès, mentions des invités, notifications de partage, « Partagé avec moi ».
//! Tout passe par HTTP avec une vraie session.

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
    /// ana : propriétaire ; ben : membre ; gus, zed : personnes extérieures ; root : administrateur de l'instance.
    ana: Uuid,
    ben: Uuid,
    gus: Uuid,
    zed: Uuid,
    root: Uuid,
    /// top > shared > child ; other : page sœur de top.
    top: Uuid,
    shared: Uuid,
    child: Uuid,
    other: Uuid,
}

async fn user(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id").bind(format!("{name}@example.org")).bind(name).fetch_one(pool).await.unwrap()
}

async fn page(pool: &PgPool, ws: Uuid, by: Uuid, title: &str, parent: Option<Uuid>) -> Uuid {
    sqlx::query_scalar("INSERT INTO pages (workspace_id, title, content, created_by, parent_id) VALUES ($1, $2, '<p>x</p>', $3, $4) RETURNING id").bind(ws).bind(title).bind(by).bind(parent).fetch_one(pool).await.unwrap()
}

impl World {
    async fn new(pool: PgPool) -> World {
        let store = crate::session_store::PgSessionStore::new(pool.clone());
        store.migrate().await.unwrap();
        let app = crate::app(crate::state::AppState::new(pool.clone(), "http://localhost".into()));
        let (ana, ben, gus, zed, root) = (user(&pool, "ana").await, user(&pool, "ben").await, user(&pool, "gus").await, user(&pool, "zed").await, user(&pool, "root").await);
        sqlx::query("UPDATE users SET is_super_admin = TRUE WHERE id = $1").bind(root).execute(&pool).await.unwrap();
        let ws: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('Team') RETURNING id").fetch_one(&pool).await.unwrap();
        for (u, role) in [(ana, "owner"), (ben, "member")] {
            sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3)").bind(ws).bind(u.to_string()).bind(role).execute(&pool).await.unwrap();
        }
        let top = page(&pool, ws, ana, "Top secret parent", None).await;
        let shared = page(&pool, ws, ana, "Shared root", Some(top)).await;
        let child = page(&pool, ws, ana, "Child", Some(shared)).await;
        let other = page(&pool, ws, ana, "Other", None).await;
        World { app, store, pool, ws, ana, ben, gus, zed, root, top, shared, child, other }
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

    fn perm_url(&self, page: Uuid) -> String {
        format!("/api/workspaces/{}/pages/{page}/permissions", self.ws)
    }

    async fn share(&self, by: Uuid, page: Uuid, target: Value, role: &str, inherit: bool) -> (u16, Value) {
        let mut body = target;
        body["role"] = json!(role);
        body["inherit"] = json!(inherit);
        self.call(by, "PUT", &self.perm_url(page), Some(body)).await
    }

    async fn get_page(&self, who: Uuid, page: Uuid) -> u16 {
        self.call(who, "GET", &format!("/api/workspaces/{}/pages/{page}", self.ws), None).await.0
    }

    /// Niveau vu par la collaboration (`none` : pas d'accès).
    async fn level(&self, who: Uuid, page: Uuid) -> String {
        let (s, b) = self.call(who, "GET", &format!("/api/pages/{page}/access"), None).await;
        if s != 200 {
            return "none".into();
        }
        b["level"].as_str().unwrap_or("none").to_string()
    }

    async fn group(&self, name: &str, members: &[Uuid]) -> Uuid {
        let (s, g) = self.call(self.root, "POST", "/api/admin/share/groups", Some(json!({ "name": name }))).await;
        assert_eq!(s, 200, "{g}");
        let id: Uuid = g["id"].as_str().unwrap().parse().unwrap();
        for m in members {
            assert_eq!(self.call(self.root, "POST", &format!("/api/admin/share/groups/{id}/members"), Some(json!({ "user_id": m }))).await.0, 204);
        }
        id
    }

    async fn notifications(&self, who: Uuid, kind: &str) -> Vec<Value> {
        let (s, b) = self.call(who, "GET", &format!("/api/notifications?workspace_id={}", self.ws), None).await;
        assert_eq!(s, 200, "{b}");
        b.as_array().unwrap().iter().filter(|n| n["kind"] == kind).cloned().collect()
    }

    /// Notifications enregistrées pour la personne (même sans accès à l'espace, qui ne les lit donc pas).
    async fn stored(&self, who: Uuid, kind: &str) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM notifications WHERE user_id = $1 AND kind = $2").bind(who).bind(kind).fetch_one(&self.pool).await.unwrap()
    }

    async fn summary(&self, who: Uuid, page: Uuid) -> Value {
        let (s, b) = self.call(who, "GET", &format!("/api/workspaces/{}/pages/{page}/access-summary", self.ws), None).await;
        assert_eq!(s, 200, "{b}");
        b
    }
}

fn person<'a>(summary: &'a Value, name: &str) -> Option<&'a Value> {
    summary["people"].as_array().unwrap().iter().find(|p| p["name"] == name)
}

#[sqlx::test]
async fn an_instance_group_is_a_share_target_resolved_like_a_person(pool: PgPool) {
    let w = World::new(pool).await;
    let g = w.group("Cohort", &[w.gus, w.zed]).await;
    let (s, b) = w.share(w.ana, w.shared, json!({ "group_id": g }), "edit", true).await;
    assert_eq!(s, 204, "{b}");
    for who in [w.gus, w.zed] {
        assert_eq!(w.level(who, w.shared).await, "edit");
        assert_eq!(w.level(who, w.child).await, "edit", "sub-pages follow the scope of the group share");
        assert_eq!(w.get_page(who, w.top).await, 404, "nothing above the shared page");
    }
    let sum = w.summary(w.ana, w.child).await;
    let gus = person(&sum, "gus").unwrap();
    assert_eq!((gus["origin"].as_str(), gus["level"].as_str(), gus["is_member"].as_bool()), (Some("group"), Some("edit"), Some(false)));
    assert_eq!(gus["via_group"]["name"], "Cohort");
    assert_eq!(sum["groups"][0]["name"], "Cohort");
    assert_eq!(sum["groups"][0]["member_count"], 2);
    assert_eq!(sum["counts"]["outside"], 2, "two people outside the space can see it");
    // Une personne qui quitte le groupe perd l'accès tout de suite.
    assert_eq!(w.call(w.root, "DELETE", &format!("/api/admin/share/groups/{g}/members/{}", w.zed), None).await.0, 204);
    assert_eq!(w.level(w.zed, w.shared).await, "none");
    assert_eq!(w.level(w.gus, w.shared).await, "edit");
    // Un partage direct prime ; le retirer rend la main au groupe.
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "read", true).await.0, 204);
    assert_eq!(w.level(w.gus, w.child).await, "read");
    assert_eq!(person(&w.summary(w.ana, w.shared).await, "gus").unwrap()["origin"], "direct");
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "none", true).await.0, 204);
    assert_eq!(w.level(w.gus, w.child).await, "edit");
    // Le plus haut niveau parmi plusieurs groupes, le refus l'emporte.
    let g2 = w.group("Readers", &[w.gus]).await;
    assert_eq!(w.share(w.ana, w.shared, json!({ "group_id": g2 }), "manage", true).await.0, 204);
    assert_eq!(w.level(w.gus, w.child).await, "manage");
    assert_eq!(w.share(w.ana, w.child, json!({ "group_id": g2 }), "deny", false).await.0, 204);
    assert_eq!(w.level(w.gus, w.child).await, "none", "a deny from any group wins on that page");
    assert_eq!(w.level(w.gus, w.shared).await, "manage");
    // Retirer le partage du groupe, puis supprimer le groupe.
    assert_eq!(w.share(w.ana, w.child, json!({ "group_id": g2 }), "none", false).await.0, 204);
    assert_eq!(w.share(w.ana, w.shared, json!({ "group_id": g2 }), "none", true).await.0, 204);
    assert_eq!(w.level(w.gus, w.child).await, "edit");
    assert_eq!(w.call(w.root, "DELETE", &format!("/api/admin/share/groups/{g}"), None).await.0, 204);
    assert_eq!(w.level(w.gus, w.shared).await, "none");
    // Aucune cible, ou deux : refusé.
    assert_eq!(w.share(w.ana, w.shared, json!({}), "read", true).await.0, 400);
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus, "group_id": g2 }), "read", true).await.0, 400);
}

#[sqlx::test]
async fn groups_are_managed_by_instance_admins_and_searched_within_the_directory_rules(pool: PgPool) {
    let w = World::new(pool).await;
    assert_eq!(w.call(w.ana, "POST", "/api/admin/share/groups", Some(json!({ "name": "Nope" }))).await.0, 403);
    let g = w.group("Alumni", &[w.gus]).await;
    assert_eq!(w.call(w.root, "POST", "/api/admin/share/groups", Some(json!({ "name": "alumni" }))).await.0, 409, "names are unique, case aside");
    // Annuaire ouvert : tout le monde trouve le groupe.
    let (s, found) = w.call(w.ben, "GET", "/api/share/groups?q=alu", None).await;
    assert_eq!((s, found.as_array().unwrap().len()), (200, 1));
    assert_eq!(found[0]["id"], g.to_string());
    // Annuaire restreint : seulement les groupes dont on fait partie.
    assert_eq!(w.call(w.root, "PUT", "/api/admin/share/directory", Some(json!({ "scope": "shared" }))).await.0, 200);
    assert!(w.call(w.ben, "GET", "/api/share/groups?q=alu", None).await.1.as_array().unwrap().is_empty());
    assert_eq!(w.call(w.gus, "GET", "/api/share/groups?q=alu", None).await.1.as_array().unwrap().len(), 1);
    assert_eq!(w.call(w.root, "PUT", "/api/admin/share/directory", Some(json!({ "scope": "everyone" }))).await.0, 400);
}

#[sqlx::test]
async fn people_search_follows_the_instance_directory_setting(pool: PgPool) {
    let w = World::new(pool).await;
    let names = |v: Value| -> Vec<String> { v.as_array().unwrap().iter().map(|u| u["name"].as_str().unwrap().to_string()).collect() };
    assert_eq!(names(w.call(w.ben, "GET", "/api/users/search?q=zed", None).await.1), vec!["zed"], "open directory by default");
    // « shared » : les personnes de mes espaces, et une adresse e-mail exacte.
    w.call(w.root, "PUT", "/api/admin/share/directory", Some(json!({ "scope": "shared" }))).await;
    assert_eq!(names(w.call(w.ben, "GET", "/api/users/search?q=ana", None).await.1), vec!["ana"]);
    assert!(names(w.call(w.ben, "GET", "/api/users/search?q=zed", None).await.1).is_empty());
    assert_eq!(names(w.call(w.ben, "GET", "/api/users/search?q=zed@example.org", None).await.1), vec!["zed"]);
    // « email » : adresse complète seulement.
    w.call(w.root, "PUT", "/api/admin/share/directory", Some(json!({ "scope": "email" }))).await;
    assert!(names(w.call(w.ben, "GET", "/api/users/search?q=ana", None).await.1).is_empty());
    assert_eq!(names(w.call(w.ben, "GET", "/api/users/search?q=ana@example.org", None).await.1), vec!["ana"]);
    assert_eq!(names(w.call(w.root, "GET", "/api/users/search?q=zed", None).await.1), vec!["zed"], "instance admins see everyone");
    assert_eq!(w.call(w.ben, "GET", "/api/share/directory", None).await.1["scope"], "email");
}

#[sqlx::test]
async fn a_person_without_rights_asks_for_access_and_a_manager_decides(pool: PgPool) {
    let w = World::new(pool).await;
    let status = |who: Uuid, page: Uuid| {
        let w = &w;
        async move { w.call(who, "GET", &format!("/api/share/access-status/{page}"), None).await.1 }
    };
    assert_eq!(status(w.zed, w.shared).await["state"], "none");
    assert_eq!(status(w.zed, Uuid::new_v4()).await["state"], "none", "an unknown page answers like a known one");
    let ask = |who: Uuid, page: Uuid| {
        let w = &w;
        async move { w.call(who, "POST", "/api/share/request-access", Some(json!({ "page_id": page, "message": "please" }))).await }
    };
    let (s, b) = ask(w.zed, w.shared).await;
    assert_eq!((s, b["state"].as_str()), (200, Some("pending")));
    assert_eq!(status(w.zed, w.shared).await["state"], "pending");
    // Pas de doublon, pas de seconde notification.
    ask(w.zed, w.shared).await;
    let reqs = w.notifications(w.ana, "access_request").await;
    assert_eq!(reqs.len(), 1, "managers are notified once");
    assert_eq!(reqs[0]["data"]["requester_name"], "zed");
    assert_eq!(reqs[0]["target"], format!("page:{}", w.shared));
    // Une page inconnue ne crée rien.
    assert_eq!(ask(w.zed, Uuid::new_v4()).await.1["state"], "pending");
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM page_access_requests").fetch_one(&w.pool).await.unwrap(), 1);
    // Seules les personnes qui gèrent la page voient et décident.
    let url = format!("/api/workspaces/{}/pages/{}/access-requests", w.ws, w.shared);
    assert_eq!(w.call(w.ben, "GET", &url, None).await.0, 403);
    let (s, list) = w.call(w.ana, "GET", &url, None).await;
    assert_eq!((s, list.as_array().unwrap().len()), (200, 1));
    let rid = list[0]["id"].as_str().unwrap().to_string();
    assert_eq!(w.summary(w.ana, w.shared).await["pending_requests"], 1);
    assert_eq!(w.call(w.gus, "POST", &format!("{url}/{rid}"), Some(json!({ "decision": "approve" }))).await.0, 404, "an outsider cannot decide");
    assert_eq!(w.call(w.ana, "POST", &format!("{url}/{rid}"), Some(json!({ "decision": "approve", "level": "manage" }))).await.0, 400, "a request never gives manage");
    assert_eq!(w.call(w.ana, "POST", &format!("{url}/{rid}"), Some(json!({ "decision": "approve", "level": "comment" }))).await.0, 204);
    assert_eq!(w.level(w.zed, w.child).await, "comment");
    assert_eq!(w.get_page(w.zed, w.top).await, 404);
    assert_eq!(w.call(w.ana, "POST", &format!("{url}/{rid}"), Some(json!({ "decision": "deny" }))).await.0, 409);
    assert_eq!(w.notifications(w.zed, "share").await.len(), 1, "the requester is told");
    assert_eq!(status(w.zed, w.shared).await["state"], "ok");
    assert!(w.notifications(w.ana, "access_request").await[0]["read_at"].is_string(), "the request notification is cleared once handled");
    // Refus : la personne voit « refusée » et n'insiste pas.
    ask(w.gus, w.other).await;
    let list = w.call(w.ana, "GET", &format!("/api/workspaces/{}/pages/{}/access-requests", w.ws, w.other), None).await.1;
    let rid = list[0]["id"].as_str().unwrap();
    assert_eq!(w.call(w.ana, "POST", &format!("/api/workspaces/{}/pages/{}/access-requests/{rid}", w.ws, w.other), Some(json!({ "decision": "deny" }))).await.0, 204);
    assert_eq!(status(w.gus, w.other).await["state"], "denied");
    assert_eq!(ask(w.gus, w.other).await.1["state"], "denied");
    assert_eq!(w.get_page(w.gus, w.other).await, 404);
    assert_eq!(w.stored(w.gus, "access_denied").await, 1);
}

#[sqlx::test]
async fn a_person_who_manages_a_page_by_share_receives_and_handles_requests(pool: PgPool) {
    let w = World::new(pool).await;
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "manage", true).await.0, 204);
    w.call(w.zed, "POST", "/api/share/request-access", Some(json!({ "page_id": w.child }))).await;
    assert_eq!(w.notifications(w.gus, "access_request").await.len(), 1, "the guest manager is a recipient");
    // L'outsider gestionnaire partage à son tour, et voit la liste des gens.
    assert_eq!(w.share(w.gus, w.child, json!({ "user_id": w.zed }), "read", false).await.0, 204);
    assert_eq!(w.level(w.zed, w.child).await, "read");
    let sum = w.summary(w.gus, w.child).await;
    assert!(person(&sum, "zed").is_some() && person(&sum, "ana").is_some());
    // Un partage posé sur une page parente que le gestionnaire ne voit pas : l'origine est dite sans titre.
    assert_eq!(w.share(w.ana, w.top, json!({ "user_id": w.ben }), "edit", true).await.0, 204);
    let sum = w.summary(w.gus, w.child).await;
    let ben = person(&sum, "ben").unwrap();
    assert_eq!((ben["origin"].as_str(), ben["from"]["id"].is_null(), ben["from"]["title"].as_str()), (Some("inherited"), true, Some("")), "{sum}");
    assert!(!sum.to_string().contains("Top secret"), "no hidden title leaks");
}

#[sqlx::test]
async fn people_with_access_can_be_mentioned_including_outsiders(pool: PgPool) {
    let w = World::new(pool).await;
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "read", true).await.0, 204);
    let (s, list) = w.call(w.ana, "GET", &format!("/api/workspaces/{}/pages/{}/mentionable", w.ws, w.child), None).await;
    assert_eq!(s, 200, "{list}");
    let who: Vec<(&str, bool)> = list.as_array().unwrap().iter().map(|p| (p["name"].as_str().unwrap(), p["is_member"].as_bool().unwrap())).collect();
    assert!(who.contains(&("gus", false)) && who.contains(&("ben", true)) && !who.iter().any(|p| p.0 == "zed"), "{who:?}");
    assert_eq!(w.call(w.zed, "GET", &format!("/api/workspaces/{}/pages/{}/mentionable", w.ws, w.child), None).await.0, 404);
    let html = |id: Uuid| format!("<p><span data-type=\"mention\" data-id=\"{id}\">@x</span> hi</p>");
    let post = |content: String| {
        let w = &w;
        async move { w.call(w.ana, "POST", &format!("/api/workspaces/{}/comments", w.ws), Some(json!({ "kind": "page", "id": w.child, "content": content }))).await }
    };
    assert_eq!(post(html(w.gus)).await.0, 200);
    assert_eq!(post(html(w.zed)).await.0, 200);
    assert_eq!(w.notifications(w.gus, "mention").await.len(), 1);
    assert_eq!(w.stored(w.zed, "mention").await, 0, "no access, no excerpt");
}

#[sqlx::test]
async fn sharing_notifies_in_the_app_and_respects_notification_events(pool: PgPool) {
    let w = World::new(pool).await;
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "comment", true).await.0, 204);
    let n = w.notifications(w.gus, "share").await;
    assert_eq!(n.len(), 1);
    assert_eq!(n[0]["target"], format!("page:{}", w.shared));
    assert_eq!(n[0]["actor_name"], "ana");
    // Même niveau : pas de seconde notification ; un autre niveau : oui ; un refus : jamais.
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "comment", false).await.0, 204);
    assert_eq!(w.notifications(w.gus, "share").await.len(), 1);
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "edit", false).await.0, 204);
    assert_eq!(w.notifications(w.gus, "share").await.len(), 2);
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.zed }), "deny", false).await.0, 204);
    assert_eq!(w.stored(w.zed, "share").await, 0);
    // Un membre est prévenu aussi.
    assert_eq!(w.share(w.ana, w.other, json!({ "user_id": w.ben }), "read", true).await.0, 204);
    assert_eq!(w.notifications(w.ben, "share").await.len(), 1);
    // Les préférences : « page partagée » est un événement réglable (e-mail et push).
    assert_eq!(crate::delivery::event_of("share", "page:x"), "page_shared");
    assert_eq!(crate::delivery::event_of("access_request", "page:x"), "access_request");
    let (s, prefs) = w.call(w.gus, "GET", "/api/user/notifications", None).await;
    assert_eq!(s, 200, "{prefs}");
    assert!(prefs["events"].as_array().unwrap().iter().any(|e| e["event"] == "page_shared"));
}

#[sqlx::test]
async fn shared_with_me_groups_by_space_and_shows_level_origin_and_only_visible_paths(pool: PgPool) {
    let w = World::new(pool).await;
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "comment", true).await.0, 204);
    let g = w.group("Cohort", &[w.gus]).await;
    assert_eq!(w.share(w.ana, w.other, json!({ "group_id": g }), "read", false).await.0, 204);
    let (s, list) = w.call(w.gus, "GET", "/api/shared", None).await;
    assert_eq!(s, 200, "{list}");
    let by: HashMap<&str, &Value> = list.as_array().unwrap().iter().map(|p| (p["title"].as_str().unwrap(), p)).collect();
    assert_eq!(by.len(), 3, "{list}");
    let root = by["Shared root"];
    assert_eq!((root["level"].as_str(), root["origin"].as_str(), root["root"].as_bool(), root["workspace_name"].as_str()), (Some("comment"), Some("direct"), Some(true), Some("Team")));
    assert_eq!(root["shared_by"], "ana");
    assert_eq!(root["path_hidden"], true, "the hidden parent is signalled, never named");
    assert!(root["path"].as_array().unwrap().is_empty());
    let child = by["Child"];
    assert_eq!((child["origin"].as_str(), child["from"]["title"].as_str()), (Some("inherited"), Some("Shared root")));
    assert_eq!(child["path"][0]["title"], "Shared root");
    assert_eq!(child["path"].as_array().unwrap().len(), 1);
    assert!(!list.to_string().contains("Top secret"), "no parent title leaks");
    let other = by["Other"];
    assert_eq!((other["origin"].as_str(), other["via_group"].as_str(), other["level"].as_str()), (Some("group"), Some("Cohort"), Some("read")));
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "none", true).await.0, 204);
    assert_eq!(w.call(w.gus, "GET", "/api/shared", None).await.1.as_array().unwrap().len(), 1);
    assert!(w.call(w.ben, "GET", "/api/shared", None).await.1.as_array().unwrap().is_empty());
}

#[sqlx::test]
async fn a_manager_by_share_cannot_use_admin_only_group_routes_and_owner_pages_stay_private(pool: PgPool) {
    let w = World::new(pool).await;
    assert_eq!(w.share(w.ana, w.shared, json!({ "user_id": w.gus }), "manage", true).await.0, 204);
    assert_eq!(w.call(w.gus, "GET", "/api/admin/share/groups", None).await.0, 403);
    let mine: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, content, created_by, private_to) VALUES ($1, 'Diary', '', $2, $2) RETURNING id").bind(w.ws).bind(w.ana).fetch_one(&w.pool).await.unwrap();
    w.call(w.zed, "POST", "/api/share/request-access", Some(json!({ "page_id": mine }))).await;
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM page_access_requests WHERE page_id = $1").bind(mine).fetch_one(&w.pool).await.unwrap(), 0);
    let _ = w.top;
}
