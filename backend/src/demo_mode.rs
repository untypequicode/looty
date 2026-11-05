//! Mode démo (`LOOTY_DEMO=true`, GLOSSARY : *Démo*).

use axum::response::IntoResponse;

/// Le mode démo est-il actif ?
pub fn enabled() -> bool {
    std::env::var("LOOTY_DEMO").as_deref() == Ok("true")
}

/// Cette requête charge le serveur ou sort vers l'extérieur : la démo la refuse (`demo_unavailable`).
/// Table unique, lue par le middleware ; l'interface affiche « Indisponible dans la démo » pour les mêmes fonctions.
pub fn unavailable(method: &str, path: &str) -> bool {
    let seg: Vec<&str> = path.trim_matches('/').split('/').collect();
    match seg.as_slice() {
        // CalDAV entrant et flux d'abonnement sortants.
        ["dav", ..] | [".well-known", "caldav"] | ["api", "ics", _] => true,
        ["api", rest @ ..] => api_unavailable(method, rest),
        _ => false,
    }
}

fn api_unavailable(method: &str, seg: &[&str]) -> bool {
    match seg {
        // Administration du serveur, courrier de l'instance, stockage, statistiques, sauvegardes.
        // Connexion : la seule entrée de la démo est « Entrer dans la démo ».
        ["auth", "login" | "callback" | "dev-login" | "desktop", ..] => true,
        ["admin", ..] => true,
        ["mail", ..] => true,
        ["user", "caldav" | "push" | "notifications", ..] => true,
        ["projects", _, "compile" | "output.pdf"] => true,
        ["projects", _, "jupyter", ..] => true,
        ["calendars", _, "sync"] => true,
        // Créer un calendrier, c'est toujours s'abonner à un agenda externe (ICS ou CalDAV).
        ["workspaces", _, "calendars"] => method == "POST",
        ["workspaces", _, "latex"] => true,
        ["workspaces", _, "moodle", ..] => true,
        ["workspaces", _, "invites"] => method == "POST",
        ["workspaces", _, "access", "members", "add"] => true,
        _ => false,
    }
}


/// Middleware : en démo, refuse les routes de `unavailable` avant toute authentification.
pub async fn guard(
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if !state.demo {
        return next.run(req).await;
    }
    let refuse = |status: axum::http::StatusCode, code: &str| (status, axum::Json(serde_json::json!({ "error": code }))).into_response();
    if unavailable(req.method().as_str(), req.uri().path()) {
        return refuse(axum::http::StatusCode::FORBIDDEN, "demo_unavailable");
    }
    // Envois : taille par fichier, puis budget de la copie du visiteur.
    let upload = req.headers().get(axum::http::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("multipart/"));
    let length = req.headers().get(axum::http::header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<i64>().ok());
    let limits = &state.demo_limits;
    if length.is_some_and(|l| l > limits.max_file_bytes) || (upload && length.is_none()) {
        return refuse(axum::http::StatusCode::PAYLOAD_TOO_LARGE, "demo_file_too_large");
    }
    if let (true, Some(length), Some(session)) = (upload, length, req.extensions().get::<Session>().cloned()) {
        if let Ok(Some(copy)) = session.get::<Uuid>(SESSION_COPY_KEY).await {
            let reserved = sqlx::query("UPDATE demo_copies SET bytes_used = bytes_used + $2 WHERE id = $1 AND bytes_used + $2 <= $3")
                .bind(copy)
                .bind(length)
                .bind(limits.max_copy_bytes)
                .execute(&state.pool)
                .await
                .map(|r| r.rows_affected())
                .unwrap_or(0);
            if reserved == 0 {
                return refuse(axum::http::StatusCode::PAYLOAD_TOO_LARGE, "demo_quota_exceeded");
            }
        }
    }
    next.run(req).await
}

// ---------------------------------------------------------------------------
// Entrer dans la démo, limites et ménage
// ---------------------------------------------------------------------------

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_sessions::Session;
use uuid::Uuid;

use crate::access::SESSION_USER_KEY;
use crate::demo::{Lang, CHARACTERS};
use crate::state::AppState;

const SESSION_COPY_KEY: &str = "demo_copy";

pub fn routes() -> Router<AppState> {
    Router::new().route("/characters", get(characters)).route("/enter", post(enter))
}

fn env_i64(name: &str, default: i64) -> i64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Les personnages proposés sur la page de connexion de la démo.
async fn characters(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    if !state.demo {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(json!(CHARACTERS
        .iter()
        .map(|c| json!({ "key": c.key, "name": c.name, "role": c.role.as_str(), "color": [c.color.0, c.color.1] }))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct EnterRequest {
    character: String,
    lang: Option<String>,
}

fn refuse(status: StatusCode, code: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": code })))
}

/// Adresse du visiteur derrière le reverse proxy de l'instance (`X-Forwarded-For`, sinon `X-Real-IP`).
fn client_ip(h: &HeaderMap) -> String {
    let get = |n: &str| h.get(n).and_then(|v| v.to_str().ok()).map(|v| v.split(',').next().unwrap_or("").trim().to_string());
    get("x-forwarded-for").or_else(|| get("x-real-ip")).filter(|s| !s.is_empty()).unwrap_or_else(|| "unknown".into())
}

/// « Entrer dans la démo » : crée (ou retrouve, si la session en a déjà une) la copie du visiteur et le connecte
/// comme le personnage choisi. Changer de personnage reste dans la même copie.
async fn enter(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Json(p): Json<EnterRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    fn internal(e: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
        tracing::error!("demo: {e}");
        refuse(StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
    }
    if !state.demo {
        return Err(refuse(StatusCode::NOT_FOUND, "not_found"));
    }
    let key = p.character.as_str();
    if !CHARACTERS.iter().any(|c| c.key == key) {
        return Err(refuse(StatusCode::BAD_REQUEST, "invalid_character"));
    }
    let existing: Option<Uuid> = session.get(SESSION_COPY_KEY).await.map_err(internal)?;
    let alive: Option<Uuid> = match existing {
        Some(c) => sqlx::query_scalar("SELECT id FROM demo_copies WHERE id = $1 AND expires_at > now()").bind(c).fetch_optional(&state.pool).await.map_err(internal)?,
        None => None,
    };
    let copy = match alive {
        Some(c) => c,
        None => create(&state, &headers, p.lang.as_deref()).await?,
    };
    let user: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE demo_copy_id = $1 AND email LIKE $2")
        .bind(copy)
        .bind(format!("{key}.%"))
        .fetch_one(&state.pool)
        .await
        .map_err(internal)?;
    session.cycle_id().await.map_err(internal)?;
    session.insert(SESSION_USER_KEY, user).await.map_err(internal)?;
    session.insert(SESSION_COPY_KEY, copy).await.map_err(internal)?;
    Ok(Json(json!({ "copy": copy, "user": user })))
}

/// Nouvelle copie, sous les limites de l'instance : copies simultanées (`LOOTY_DEMO_MAX_COPIES`, 30) et copies
/// par adresse et par heure (`LOOTY_DEMO_COPIES_PER_IP_HOUR`, 5).
async fn create(state: &AppState, headers: &HeaderMap, lang: Option<&str>) -> Result<Uuid, (StatusCode, Json<Value>)> {
    let internal = |e: &dyn std::fmt::Display| {
        tracing::error!("demo copy: {e}");
        refuse(StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
    };
    let ip = client_ip(headers);
    let recent: i64 = sqlx::query_scalar("SELECT count(*) FROM demo_copies WHERE ip = $1 AND created_at > now() - interval '1 hour'")
        .bind(&ip)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| internal(&e))?;
    if recent >= state.demo_limits.copies_per_ip_hour {
        return Err(refuse(StatusCode::TOO_MANY_REQUESTS, "demo_rate_limited"));
    }
    let copy = Uuid::new_v4();
    // Le réservoir de copies est vérifié dans la même requête que la réservation : deux entrées simultanées
    // ne dépassent pas la limite.
    let reserved = sqlx::query(
        "INSERT INTO demo_copies (id, ip) SELECT $1, $2 WHERE (SELECT count(*) FROM demo_copies WHERE expires_at > now()) < $3",
    )
    .bind(copy)
    .bind(&ip)
    .bind(state.demo_limits.max_copies)
    .execute(&state.pool)
    .await
    .map_err(|e| internal(&e))?
    .rows_affected();
    if reserved == 0 {
        return Err(refuse(StatusCode::SERVICE_UNAVAILABLE, "demo_full"));
    }
    let lang = if lang == Some("en") { Lang::En } else { Lang::Fr };
    if let Err(e) = crate::demo::create_copy(state, lang, copy).await {
        let _ = remove_copy(state, copy).await;
        return Err(internal(&e));
    }
    Ok(copy)
}

/// Retire une copie : espaces (personnels compris), comptes des personnages, fichiers, puis la copie elle-même.
/// Utilisé pour le ménage des copies expirées et pour défaire une génération qui a échoué.
async fn remove_copy(state: &AppState, copy: Uuid) -> Result<(), String> {
    let db = |e: sqlx::Error| e.to_string();
    let pool = &state.pool;
    let users: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE demo_copy_id = $1").bind(copy).fetch_all(pool).await.map_err(db)?;
    let user_ids: Vec<String> = users.iter().map(Uuid::to_string).collect();
    let workspaces: Vec<Uuid> = sqlx::query_scalar(
        "SELECT w.id FROM workspaces w
          WHERE w.personal_of = ANY($1)
             OR EXISTS (SELECT 1 FROM workspace_members m WHERE m.workspace_id = w.id AND m.user_id = ANY($2))",
    )
    .bind(&users)
    .bind(&user_ids)
    .fetch_all(pool)
    .await
    .map_err(db)?;

    // Fichiers : repérés avant la suppression des lignes qui les citent.
    let mut files = crate::storage::files_in_workspaces(pool, &workspaces).await.map_err(|e| format!("{e:?}"))?;
    let avatars: Vec<String> = sqlx::query_scalar("SELECT avatar_url FROM users WHERE demo_copy_id = $1 AND avatar_url LIKE '/uploads/%'").bind(copy).fetch_all(pool).await.map_err(db)?;
    files.extend(avatars.iter().filter_map(|u| u.strip_prefix("/uploads/")).map(str::to_string));

    for w in &workspaces {
        state.emit(*w, json!({ "type": "WORKSPACE_DELETED", "workspace_id": w })).await;
    }
    let mut tx = pool.begin().await.map_err(db)?;
    sqlx::query("DELETE FROM workspaces WHERE id = ANY($1)").bind(&workspaces).execute(&mut *tx).await.map_err(db)?;
    // Les tables avec un identifiant d'utilisateur textuel n'ont pas de clé étrangère.
    for sql in [
        "DELETE FROM workspace_members WHERE user_id = ANY($1)",
        "DELETE FROM task_assignees WHERE user_id = ANY($1)",
        "DELETE FROM task_validations WHERE user_id = ANY($1)",
    ] {
        sqlx::query(sql).bind(&user_ids).execute(&mut *tx).await.map_err(db)?;
    }
    sqlx::query("DELETE FROM users WHERE demo_copy_id = $1").bind(copy).execute(&mut *tx).await.map_err(db)?;
    sqlx::query("DELETE FROM demo_copies WHERE id = $1").bind(copy).execute(&mut *tx).await.map_err(db)?;
    tx.commit().await.map_err(db)?;

    for name in files.iter().filter(|n| crate::upload::is_safe_name(n)) {
        let _ = tokio::fs::remove_file(std::path::Path::new(crate::upload::UPLOAD_DIR).join(name)).await;
    }
    for w in &workspaces {
        let _ = tokio::fs::remove_dir_all(std::path::Path::new(crate::upload::UPLOAD_DIR).join("drive").join(w.to_string())).await;
    }
    Ok(())
}

/// Supprime les copies expirées (24 h après leur création) ; renvoie leur nombre.
pub async fn purge_expired(state: &AppState) -> Result<usize, sqlx::Error> {
    let copies: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM demo_copies WHERE expires_at <= now()").fetch_all(&state.pool).await?;
    let mut removed = 0;
    for copy in copies {
        match remove_copy(state, copy).await {
            Ok(()) => removed += 1,
            Err(e) => tracing::warn!("demo: copy {copy} not removed: {e}"),
        }
    }
    Ok(removed)
}

/// Tâche de fond : ménage des copies expirées, toutes les 10 minutes.
pub fn start_purge_loop(state: AppState) {
    tokio::spawn(async move {
        loop {
            match purge_expired(&state).await {
                Ok(n) if n > 0 => tracing::info!("demo: {n} expired cop(ies) deleted"),
                Err(e) => tracing::warn!("demo purge: {e}"),
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
        }
    });
}

/// Limites de la démo : copies simultanées, copies par adresse et par heure, taille des envois.
#[derive(Clone, Debug)]
pub struct DemoLimits {
    pub max_copies: i64,
    pub copies_per_ip_hour: i64,
    pub max_file_bytes: i64,
    pub max_copy_bytes: i64,
}

impl Default for DemoLimits {
    fn default() -> Self {
        DemoLimits { max_copies: 30, copies_per_ip_hour: 5, max_file_bytes: 2 * 1024 * 1024, max_copy_bytes: 20 * 1024 * 1024 }
    }
}

impl DemoLimits {
    /// `LOOTY_DEMO_MAX_COPIES`, `LOOTY_DEMO_COPIES_PER_IP_HOUR`, `LOOTY_DEMO_MAX_FILE_MB`, `LOOTY_DEMO_MAX_COPY_MB`.
    pub fn from_env() -> Self {
        let d = DemoLimits::default();
        let mb = |name: &str, default: i64| env_i64(name, default / (1024 * 1024)) * 1024 * 1024;
        DemoLimits {
            max_copies: env_i64("LOOTY_DEMO_MAX_COPIES", d.max_copies),
            copies_per_ip_hour: env_i64("LOOTY_DEMO_COPIES_PER_IP_HOUR", d.copies_per_ip_hour),
            max_file_bytes: mb("LOOTY_DEMO_MAX_FILE_MB", d.max_file_bytes),
            max_copy_bytes: mb("LOOTY_DEMO_MAX_COPY_MB", d.max_copy_bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outgoing_routes_are_unavailable_in_the_demo() {
        let blocked = [
            ("POST", "/api/projects/7/compile"),
            ("GET", "/api/projects/7/output.pdf"),
            ("GET", "/api/workspaces/3/latex"),
            ("PUT", "/api/workspaces/3/latex"),
            ("GET", "/api/projects/7/jupyter"),
            ("PUT", "/api/projects/7/jupyter"),
            ("POST", "/api/projects/7/jupyter/test"),
            ("POST", "/api/projects/7/jupyter/kernels"),
            ("GET", "/api/projects/7/jupyter/kernels/k/channels"),
            ("GET", "/api/mail/accounts"),
            ("POST", "/api/mail/accounts/1/send"),
            ("GET", "/api/admin/mail"),
            ("GET", "/api/workspaces/3/moodle"),
            ("GET", "/api/workspaces/3/moodle/courses/9/grades"),
            ("POST", "/api/workspaces/3/calendars"),
            ("POST", "/api/calendars/5/sync"),
            ("GET", "/api/user/caldav"),
            ("POST", "/api/user/caldav/discover"),
            ("GET", "/api/ics/abc.ics"),
            ("PROPFIND", "/dav/"),
            ("GET", "/.well-known/caldav"),
            ("POST", "/api/workspaces/3/invites"),
            ("POST", "/api/workspaces/3/access/members/add"),
            ("PUT", "/api/user/notifications"),
            ("POST", "/api/user/push/subscriptions"),
            ("GET", "/api/admin/users"),
            ("GET", "/api/admin/storage"),
            ("GET", "/api/admin/insights/analytics"),
            ("POST", "/api/admin/backups"),
            ("POST", "/api/admin/backups/x/restore"),
        ];
        for (method, path) in blocked {
            assert!(unavailable(method, path), "{method} {path} should be refused");
        }
    }

    #[test]
    fn local_routes_stay_available() {
        let open = [
            ("GET", "/api/workspaces"),
            ("POST", "/api/workspaces/3/boards"),
            ("GET", "/api/workspaces/3/calendars"),
            ("GET", "/api/workspaces/3/invites"),
            ("PUT", "/api/calendars/5"),
            ("POST", "/api/upload"),
            ("GET", "/api/ws"),
            ("GET", "/api/auth/me"),
            ("GET", "/api/user/tokens"),
            ("POST", "/api/projects/7/files"),
            ("GET", "/api/workspaces/3/moodlefoo"),
            ("GET", "/api/health"),
        ];
        for (method, path) in open {
            assert!(!unavailable(method, path), "{method} {path} should stay open");
        }
    }
}

#[cfg(test)]
mod http_tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    async fn call(state: crate::state::AppState, method: &str, path: &str) -> (u16, String) {
        let res = crate::app(state).oneshot(Request::builder().method(method).uri(path).body(Body::empty()).unwrap()).await.unwrap();
        let status = res.status().as_u16();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8_lossy(&body).to_string())
    }

    #[sqlx::test]
    async fn the_backend_refuses_outgoing_routes_in_the_demo(pool: sqlx::PgPool) {
        let mut state = crate::state::AppState::new(pool, "http://localhost".into());
        state.demo = true;
        let (status, body) = call(state, "POST", "/api/projects/7/compile").await;
        assert_eq!((status, body.as_str()), (403, r#"{"error":"demo_unavailable"}"#));
    }

    #[sqlx::test]
    async fn nothing_changes_without_the_demo_flag(pool: sqlx::PgPool) {
        let state = crate::state::AppState::new(pool, "http://localhost".into());
        let (status, body) = call(state, "POST", "/api/projects/7/compile").await;
        assert!(!body.contains("demo_unavailable"), "{status} {body}");
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;
    use axum::{body::Body, http::{header, Request}};
    use serde_json::{json, Value};
    use tower::ServiceExt;

    struct Visitor {
        app: axum::Router,
        cookie: String,
    }

    impl Visitor {
        async fn new(pool: &sqlx::PgPool) -> Self {
            crate::session_store::PgSessionStore::new(pool.clone()).migrate().await.unwrap();
            let mut state = crate::state::AppState::new(pool.clone(), "http://localhost".into());
            state.demo = true;
            Visitor { app: crate::app(state), cookie: String::new() }
        }
        async fn call(&mut self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
            let mut req = Request::builder().method(method).uri(path).header(header::CONTENT_TYPE, "application/json");
            if !self.cookie.is_empty() {
                req = req.header(header::COOKIE, &self.cookie);
            }
            let res = self.app.clone().oneshot(req.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap()).await.unwrap();
            let status = res.status().as_u16();
            let set: Vec<String> = res.headers().get_all(header::SET_COOKIE).iter().filter_map(|c| c.to_str().ok()).map(|c| c.split(';').next().unwrap().to_string()).collect();
            if !set.is_empty() {
                self.cookie = set.join("; ");
            }
            let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
            (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
        }
        async fn enter(&mut self, character: &str) -> (u16, Value) {
            self.call("POST", "/api/auth/demo/enter", Some(json!({ "character": character, "lang": "en" }))).await
        }
    }

    fn names(workspaces: &Value) -> Vec<(String, String)> {
        let list = workspaces.get("workspaces").unwrap_or(workspaces);
        list.as_array().unwrap().iter().filter(|w| w["personal"] == false).map(|w| (w["id"].as_str().unwrap().into(), w["name"].as_str().unwrap().into())).collect()
    }

    #[sqlx::test]
    async fn each_visitor_gets_an_isolated_copy_of_the_demo_workspace(pool: sqlx::PgPool) {
        let (mut a, mut b) = (Visitor::new(&pool).await, Visitor::new(&pool).await);
        let (status, body) = a.enter("maya").await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(b.enter("maya").await.0, 200);

        let (_, wa) = a.call("GET", "/api/workspaces", None).await;
        let (_, wb) = b.call("GET", "/api/workspaces", None).await;
        let (na, nb) = (names(&wa), names(&wb));
        assert_eq!(na.iter().map(|w| w.1.as_str()).collect::<Vec<_>>(), ["Looty Demo"]);
        assert_eq!(nb.iter().map(|w| w.1.as_str()).collect::<Vec<_>>(), ["Looty Demo"]);
        assert_ne!(na[0].0, nb[0].0, "two visitors must not share a workspace");

        let (status, _) = b.call("GET", &format!("/api/workspaces/{}/members", na[0].0), None).await;
        assert!(status == 403 || status == 404, "visitor B can read visitor A's workspace: {status}");
    }

    #[sqlx::test]
    async fn switching_character_stays_in_the_same_copy(pool: sqlx::PgPool) {
        let mut v = Visitor::new(&pool).await;
        assert_eq!(v.enter("maya").await.0, 200);
        let (_, me) = v.call("GET", "/api/auth/me", None).await;
        assert_eq!(me["user"]["name"], "Maya Chen");
        assert_eq!(v.enter("noah").await.0, 200);
        let (_, me) = v.call("GET", "/api/auth/me", None).await;
        assert_eq!(me["user"]["name"], "Noah Fischer");
        let copies: i64 = sqlx::query_scalar("SELECT count(*) FROM demo_copies").fetch_one(&pool).await.unwrap();
        assert_eq!(copies, 1);
    }

    #[sqlx::test]
    async fn an_expired_copy_is_deleted_with_its_accounts_workspaces_and_files(pool: sqlx::PgPool) {
        let (mut a, mut b) = (Visitor::new(&pool).await, Visitor::new(&pool).await);
        assert_eq!(a.enter("maya").await.0, 200);
        assert_eq!(b.enter("maya").await.0, 200);
        let avatars = |pool: sqlx::PgPool, name: &'static str| async move {
            let urls: Vec<String> = sqlx::query_scalar("SELECT avatar_url FROM users WHERE demo_copy_id = (SELECT demo_copy_id FROM users WHERE name = 'Maya Chen' ORDER BY created_at LIMIT 1 OFFSET $1) AND avatar_url IS NOT NULL")
                .bind(if name == "a" { 0i64 } else { 1 })
                .fetch_all(&pool)
                .await
                .unwrap();
            urls
        };
        let (first_a, first_b) = (avatars(pool.clone(), "a").await, avatars(pool.clone(), "b").await);
        assert_eq!((first_a.len(), first_b.len()), (5, 5));
        let on_disk = |url: &String| std::path::Path::new("uploads").join(url.trim_start_matches("/uploads/")).exists();
        assert!(first_a.iter().chain(&first_b).all(on_disk));

        let copy_a: uuid::Uuid = sqlx::query_scalar("SELECT demo_copy_id FROM users WHERE name = 'Maya Chen' ORDER BY created_at LIMIT 1").fetch_one(&pool).await.unwrap();
        sqlx::query("UPDATE demo_copies SET expires_at = now() - interval '1 minute' WHERE id = $1").bind(copy_a).execute(&pool).await.unwrap();

        let mut state = crate::state::AppState::new(pool.clone(), "http://localhost".into());
        state.demo = true;
        assert_eq!(purge_expired(&state).await.unwrap(), 1);

        let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE demo_copy_id = $1").bind(copy_a).fetch_one(&pool).await.unwrap();
        let all_users: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&pool).await.unwrap();
        let shared: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE personal_of IS NULL").fetch_one(&pool).await.unwrap();
        let personal: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE personal_of IS NOT NULL").fetch_one(&pool).await.unwrap();
        assert_eq!((users, all_users, shared, personal), (0, 5, 1, 5), "only visitor B's copy must remain");
        assert!(!first_a.iter().any(on_disk), "avatars of the expired copy must be deleted");
        assert!(first_b.iter().all(on_disk), "the other copy keeps its files");
    }

    fn multipart(bytes: usize) -> (String, Vec<u8>) {
        let b = "XBOUNDARYX";
        let mut body = format!("--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\n").into_bytes();
        body.extend(std::iter::repeat_n(b'a', bytes));
        body.extend(format!("\r\n--{b}--\r\n").into_bytes());
        (format!("multipart/form-data; boundary={b}"), body)
    }

    impl Visitor {
        async fn upload(&mut self, bytes: usize) -> (u16, Value) {
            let (ct, body) = multipart(bytes);
            let req = Request::builder().method("POST").uri("/api/upload").header(header::CONTENT_TYPE, ct).header(header::CONTENT_LENGTH, body.len()).header(header::COOKIE, &self.cookie).body(Body::from(body)).unwrap();
            let res = self.app.clone().oneshot(req).await.unwrap();
            let status = res.status().as_u16();
            let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
            (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
        }
        fn with(pool: &sqlx::PgPool, f: impl FnOnce(&mut DemoLimits)) -> impl std::future::Future<Output = Visitor> + '_ {
            let mut limits = DemoLimits::default();
            f(&mut limits);
            async move {
                let v = Visitor::new(pool).await;
                let mut state = crate::state::AppState::new(pool.clone(), "http://localhost".into());
                state.demo = true;
                state.demo_limits = limits;
                Visitor { app: crate::app(state), cookie: v.cookie }
            }
        }
    }

    #[sqlx::test]
    async fn the_number_of_simultaneous_copies_is_limited(pool: sqlx::PgPool) {
        let mut a = Visitor::with(&pool, |l| l.max_copies = 1).await;
        let mut b = Visitor::with(&pool, |l| l.max_copies = 1).await;
        assert_eq!(a.enter("maya").await.0, 200);
        let (status, body) = b.enter("maya").await;
        assert_eq!((status, body["error"].as_str()), (503, Some("demo_full")));
        let copies: i64 = sqlx::query_scalar("SELECT count(*) FROM demo_copies").fetch_one(&pool).await.unwrap();
        assert_eq!(copies, 1);
    }

    #[sqlx::test]
    async fn entering_is_rate_limited_per_ip(pool: sqlx::PgPool) {
        let mut a = Visitor::with(&pool, |l| l.copies_per_ip_hour = 1).await;
        let mut b = Visitor::with(&pool, |l| l.copies_per_ip_hour = 1).await;
        assert_eq!(a.enter("maya").await.0, 200);
        assert_eq!(a.enter("noah").await.0, 200, "switching character is not a new copy");
        let (status, body) = b.enter("maya").await;
        assert_eq!((status, body["error"].as_str()), (429, Some("demo_rate_limited")));
    }

    #[sqlx::test]
    async fn uploads_are_capped_per_file_and_per_copy(pool: sqlx::PgPool) {
        let mut v = Visitor::with(&pool, |l| {
            l.max_file_bytes = 2_000;
            l.max_copy_bytes = 3_000;
        })
        .await;
        assert_eq!(v.enter("maya").await.0, 200);
        let (status, body) = v.upload(5_000).await;
        assert_eq!((status, body["error"].as_str()), (413, Some("demo_file_too_large")));
        assert_eq!(v.upload(1_000).await.0, 200);
        assert_eq!(v.upload(1_000).await.0, 200);
        let (status, body) = v.upload(1_000).await;
        assert_eq!((status, body["error"].as_str()), (413, Some("demo_quota_exceeded")));
    }

    #[sqlx::test]
    async fn the_demo_is_the_only_way_in(pool: sqlx::PgPool) {
        let mut v = Visitor::new(&pool).await;
        let (_, providers) = v.call("GET", "/api/auth/providers", None).await;
        assert_eq!(providers, json!([]));
        for path in ["/api/auth/login/00000000-0000-0000-0000-000000000000", "/api/auth/dev-login?email=a@b.c", "/api/auth/desktop/start"] {
            let (status, body) = v.call("GET", path, None).await;
            assert_eq!((status, body["error"].as_str()), (403, Some("demo_unavailable")), "{path}");
        }
    }

    #[sqlx::test]
    async fn app_passwords_are_unavailable_but_api_tokens_are_not(pool: sqlx::PgPool) {
        let mut v = Visitor::new(&pool).await;
        assert_eq!(v.enter("maya").await.0, 200);
        for scope in ["caldav", "ics"] {
            let (status, body) = v.call("POST", "/api/user/tokens", Some(json!({ "name": "x", "scope": scope }))).await;
            assert_eq!((status, body["error"].as_str()), (403, Some("demo_unavailable")), "{scope}");
        }
        assert_eq!(v.call("POST", "/api/user/tokens", Some(json!({ "name": "x", "scope": "api" }))).await.0, 200);
    }
}

