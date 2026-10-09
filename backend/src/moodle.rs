//! Intégration Moodle (par ex. moodle.u-bordeaux.fr).
//!
//! - L'adresse de la plateforme est réglée par espace de travail (administrateurs).
//! - Chaque membre connecte son propre compte avec un jeton du service mobile
//!   (`moodle_mobile_app`) : identifiant + mot de passe pour les comptes locaux, ou jeton
//!   collé / lien `moodlemobile://token=…` obtenu après la connexion SSO (CAS).
//!   Le jeton est chiffré ; il donne les mêmes droits que l'application Moodle officielle.
//! - Cours, échéances à venir (devoirs, tests…), messages (lecture, réponse) et
//!   notifications, lus à la demande via l'API REST des services web.
//! - Le calendrier Moodle est importé comme abonnement ICS personnel (lien d'export
//!   Moodle) : il apparaît dans le calendrier Looty, sur le web comme sur Android.
//! - Toutes les 5 minutes : nouveaux messages et notifications → notification Looty.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

use crate::access::{require_member, require_ws_admin, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Plateforme proposée par défaut (Université de Bordeaux).
pub const DEFAULT_URL: &str = "https://moodle.u-bordeaux.fr";

pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(overview))
        .route("/settings", put(put_settings))
        .route("/site", get(site_probe))
        .route("/connect", post(connect).delete(disconnect))
        .route("/preferences", put(put_preferences))
        .route("/courses", get(courses))
        .route("/deadlines", get(deadlines))
        .route("/notifications", get(notifications))
        .route("/conversations", get(conversations))
        .route("/conversations/:id", get(conversation).post(send_message))
        .route("/calendar/sync", post(sync_calendar))
        .route("/open", get(open_link))
}

// ---------------------------------------------------------------------------
// Client des services web
// ---------------------------------------------------------------------------

fn http() -> &'static reqwest::Client {
    static C: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .redirect(reqwest::redirect::Policy::limited(3))
            .user_agent("Looty (MoodleMobile compatible)")
            .build()
            .expect("client http")
    })
}

/// Adresse de plateforme valide : https (http seulement en local), sans chemin final.
async fn clean_base(raw: &str) -> ApiResult<String> {
    let mut t = raw.trim().trim_end_matches('/').to_string();
    if !t.contains("://") {
        t = format!("https://{t}");
    }
    let url = crate::caldav::check_url(&t).await.map_err(|_| ApiError::bad_request("moodle_invalid_url"))?;
    let mut base = format!("{}://{}", url.scheme(), url.host_str().unwrap_or(""));
    if let Some(p) = url.port() {
        base.push_str(&format!(":{p}"));
    }
    base.push_str(url.path().trim_end_matches('/'));
    Ok(base)
}

#[derive(Clone)]
struct Ws {
    base: String,
    token: String,
}

impl Ws {
    /// Appel REST ; les erreurs Moodle (renvoyées avec un statut 200) deviennent des codes.
    async fn call(&self, function: &str, params: &[(String, String)]) -> ApiResult<Value> {
        let mut form: Vec<(String, String)> = vec![
            ("wstoken".into(), self.token.clone()),
            ("wsfunction".into(), function.into()),
            ("moodlewsrestformat".into(), "json".into()),
        ];
        form.extend_from_slice(params);
        let r = http()
            .post(format!("{}/webservice/rest/server.php", self.base))
            .form(&form)
            .send()
            .await
            .map_err(|_| ApiError::bad_request("moodle_unreachable"))?;
        if !r.status().is_success() {
            return Err(ApiError::BadRequest(format!("moodle_http_error: {}", r.status().as_u16())));
        }
        let v: Value = r.json().await.map_err(|_| ApiError::bad_request("moodle_bad_response"))?;
        if let Some(code) = v.get("errorcode").and_then(Value::as_str) {
            return Err(match code {
                "invalidtoken" | "accessexception" => ApiError::bad_request("moodle_invalid_token"),
                "servicenotavailable" | "webservicesnotenabled" | "enablewsdescription" => ApiError::bad_request("moodle_ws_disabled"),
                _ => ApiError::BadRequest(format!("moodle_error: {}", v.get("message").and_then(Value::as_str).unwrap_or(code))),
            });
        }
        Ok(v)
    }
}

fn p(k: &str, v: impl ToString) -> (String, String) {
    (k.to_string(), v.to_string())
}

fn valid_token(t: &str) -> bool {
    t.len() >= 16 && t.len() <= 128 && t.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Jeton (et jeton privé éventuel) à partir de ce que l'utilisateur colle ou de ce que
/// renvoie le portail officiel : jeton brut ou lien
/// `moodlemobile://token=BASE64(signature:::jeton[:::jeton privé])`.
fn parse_token(raw: &str) -> Option<(String, Option<String>)> {
    let t = raw.trim();
    // Lien éventuellement encodé (gestionnaire de protocole du navigateur).
    let decoded = if t.contains("%3A") || t.contains("%3D") { pct_decode(t) } else { t.to_string() };
    let encoded = decoded.split("token=").nth(1).map(|s| s.trim_end_matches(['/', '#', ' ']).to_string());
    let (token, private) = match encoded {
        Some(e) => {
            let bytes = base64::engine::general_purpose::STANDARD.decode(e).ok()?;
            let s = String::from_utf8(bytes).ok()?;
            let mut parts = s.split(":::");
            let _signature = parts.next();
            (parts.next()?.to_string(), parts.next().map(str::to_string).filter(|p| valid_token(p)))
        }
        None => (decoded, None),
    };
    valid_token(&token).then_some((token, private))
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// Connexion
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct Conn {
    id: Uuid,
    base_url: String,
    token_enc: String,
    moodle_user_id: Option<i64>,
    calendar_id: Option<Uuid>,
}

async fn conn_of(state: &AppState, user: &AuthUser, ws: Uuid) -> ApiResult<(Conn, Ws, i64)> {
    require_member(&state.pool, user, ws).await?;
    let c: Conn = sqlx::query_as("SELECT id, base_url, token_enc, moodle_user_id, calendar_id FROM moodle_connections WHERE workspace_id = $1 AND user_id = $2")
        .bind(ws)
        .bind(user.id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::bad_request("moodle_not_connected"))?;
    let token = crate::crypto::decrypt(&c.token_enc).ok_or_else(|| ApiError::bad_request("moodle_invalid_token"))?;
    let ws_client = Ws { base: c.base_url.clone(), token };
    let uid = c.moodle_user_id.unwrap_or(0);
    Ok((c, ws_client, uid))
}

/// Mémorise l'erreur (jeton révoqué…) pour l'afficher, puis la renvoie.
async fn track<T>(state: &AppState, id: Uuid, r: ApiResult<T>) -> ApiResult<T> {
    let err = match &r {
        Err(ApiError::BadRequest(m)) => Some(m.clone()),
        Err(_) => None,
        Ok(_) => None,
    };
    let _ = sqlx::query("UPDATE moodle_connections SET last_error = $2 WHERE id = $1").bind(id).bind(err).execute(&state.pool).await;
    r
}

#[derive(Serialize, sqlx::FromRow)]
struct ConnDto {
    base_url: String,
    moodle_user_id: Option<i64>,
    full_name: Option<String>,
    site_name: Option<String>,
    last_error: Option<String>,
    last_sync_at: Option<DateTime<Utc>>,
    calendar_id: Option<Uuid>,
    notify: bool,
}

async fn overview(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Value>> {
    let role = require_member(&state.pool, &user, ws).await?;
    let url: Option<String> = sqlx::query_scalar("SELECT moodle_url FROM workspaces WHERE id = $1").bind(ws).fetch_one(&state.pool).await?;
    let conn: Option<ConnDto> = sqlx::query_as(
        "SELECT base_url, moodle_user_id, full_name, site_name, last_error, last_sync_at, calendar_id, notify FROM moodle_connections WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(ws)
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(Json(json!({ "url": url, "default_url": DEFAULT_URL, "can_configure": role.is_admin(), "connection": conn })))
}

#[derive(Deserialize)]
struct SettingsReq {
    /// Adresse de la plateforme ; vide = Moodle désactivé pour l'espace.
    url: Option<String>,
}

async fn put_settings(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<SettingsReq>) -> ApiResult<Json<Value>> {
    require_ws_admin(&state.pool, &user, ws).await?;
    let url = match p.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) => Some(clean_base(u).await?),
        None => None,
    };
    sqlx::query("UPDATE workspaces SET moodle_url = $2 WHERE id = $1").bind(ws).bind(&url).execute(&state.pool).await?;
    Ok(Json(json!({ "url": url })))
}

#[derive(Deserialize)]
struct SiteQuery {
    url: Option<String>,
}

/// Informations publiques de la plateforme : nom, logo, type de connexion (1 : dans
/// l'application, 2/3 : SSO dans le navigateur) et lien de lancement SSO.
async fn site_probe(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<SiteQuery>) -> ApiResult<Json<Value>> {
    require_member(&state.pool, &user, ws).await?;
    let raw = match q.url {
        Some(u) => u,
        None => sqlx::query_scalar::<_, Option<String>>("SELECT moodle_url FROM workspaces WHERE id = $1").bind(ws).fetch_one(&state.pool).await?.unwrap_or_else(|| DEFAULT_URL.into()),
    };
    let base = clean_base(&raw).await?;
    let body = json!([{ "index": 0, "methodname": "tool_mobile_get_public_config", "args": {} }]);
    let r = http()
        .post(format!("{base}/lib/ajax/service-nologin.php?info=tool_mobile_get_public_config"))
        .json(&body)
        .send()
        .await
        .map_err(|_| ApiError::bad_request("moodle_unreachable"))?;
    let v: Value = r.json().await.map_err(|_| ApiError::bad_request("moodle_not_moodle"))?;
    let data = v.get(0).filter(|x| x["error"] != json!(true)).map(|x| x["data"].clone()).ok_or_else(|| ApiError::bad_request("moodle_ws_disabled"))?;
    let typeoflogin = data["typeoflogin"].as_i64().unwrap_or(1);
    // Portail officiel : la page de connexion habituelle (CAS…), puis Moodle redirige
    // vers <schéma>://token=… — `moodlemobile` (application de bureau / Android, qui
    // l'interceptent) ou `web+looty` (gestionnaire de protocole du navigateur, si la
    // plateforme n'impose pas son schéma).
    let passport: u32 = rand::random();
    let launch = |scheme: &str| format!("{base}/admin/tool/mobile/launch.php?service=moodle_mobile_app&passport={passport}&urlscheme={scheme}");
    Ok(Json(json!({
        "base_url": base,
        "site_name": data["sitename"],
        "logo": data["compactlogourl"].as_str().or(data["logourl"].as_str()),
        "sso": typeoflogin != 1,
        "launch_url": launch("moodlemobile"),
        "launch_url_web": launch("web%2Blooty"),
        "tokens_url": format!("{base}/user/managetoken.php"),
        "frameable": frameable(&base).await,
    })))
}

/// Le site accepte-t-il d'être affiché dans Looty (cadre) ? X-Frame-Options et
/// frame-ancestors de la page d'accueil.
async fn frameable(base: &str) -> bool {
    let Ok(r) = http().get(format!("{base}/")).send().await else { return false };
    let h = r.headers();
    let xfo = h.get("x-frame-options").and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let csp = h.get("content-security-policy").and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    // Cadre dans un autre site : le cookie de session n'est envoyé que s'il est
    // « SameSite=None » ; sinon Moodle s'y affiche toujours déconnecté (cas de Bordeaux).
    let session_ok = h.get_all("set-cookie").iter().filter_map(|v| v.to_str().ok()).filter(|c| c.starts_with("MoodleSession")).all(|c| {
        let c = c.to_ascii_lowercase();
        c.contains("samesite=none") && c.contains("secure")
    });
    xfo.is_empty() && !csp.contains("frame-ancestors") && session_ok
}

#[derive(Deserialize)]
struct OpenQuery {
    /// Page du site à ouvrir (par défaut l'accueil).
    url: Option<String>,
}

/// Lien vers le site officiel, connecté automatiquement quand c'est possible (clé
/// d'auto-connexion de l'application mobile, valable une minute) ; sinon le lien simple.
async fn open_link(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<OpenQuery>) -> ApiResult<Json<Value>> {
    let (c, client, uid) = conn_of(&state, &user, ws).await?;
    let target = q.url.filter(|u| u.starts_with(&format!("{}/", c.base_url)) || *u == c.base_url).unwrap_or_else(|| format!("{}/my/", c.base_url));
    let private: Option<String> = sqlx::query_scalar("SELECT private_token_enc FROM moodle_connections WHERE id = $1").bind(c.id).fetch_one(&state.pool).await?;
    let auto = async {
        let private = crate::crypto::decrypt(private.as_deref()?)?;
        let v = client.call("tool_mobile_get_autologin_key", &[p("privatetoken", private)]).await.ok()?;
        let key = v["key"].as_str()?;
        let base = v["autologinurl"].as_str()?;
        Some(format!("{base}?userid={uid}&key={key}&urltogo={}", crate::mailclient::pct(&target)))
    }
    .await;
    Ok(Json(json!({ "url": auto.clone().unwrap_or(target), "autologin": auto.is_some() })))
}

#[derive(Deserialize)]
struct ConnectReq {
    url: Option<String>,
    token: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

async fn connect(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<ConnectReq>) -> ApiResult<Json<Value>> {
    require_member(&state.pool, &user, ws).await?;
    let raw = match p.url.as_deref().filter(|u| !u.trim().is_empty()) {
        Some(u) => u.to_string(),
        None => sqlx::query_scalar::<_, Option<String>>("SELECT moodle_url FROM workspaces WHERE id = $1").bind(ws).fetch_one(&state.pool).await?.unwrap_or_else(|| DEFAULT_URL.into()),
    };
    let base = clean_base(&raw).await?;
    let (token, private) = match (&p.token, &p.username, &p.password) {
        (Some(t), _, _) if !t.trim().is_empty() => parse_token(t).ok_or_else(|| ApiError::bad_request("moodle_invalid_token"))?,
        (_, Some(u), Some(pw)) if !u.trim().is_empty() && !pw.is_empty() => {
            let r = http()
                .post(format!("{base}/login/token.php"))
                .form(&[("username", u.trim()), ("password", pw.as_str()), ("service", "moodle_mobile_app")])
                .send()
                .await
                .map_err(|_| ApiError::bad_request("moodle_unreachable"))?;
            let v: Value = r.json().await.map_err(|_| ApiError::bad_request("moodle_not_moodle"))?;
            match v["token"].as_str() {
                Some(t) => (t.to_string(), v["privatetoken"].as_str().filter(|x| valid_token(x)).map(str::to_string)),
                None if v["errorcode"] == json!("invalidlogin") => return Err(ApiError::bad_request("moodle_auth_failed")),
                None if v["errorcode"] == json!("enablewsdescription") => return Err(ApiError::bad_request("moodle_ws_disabled")),
                None => return Err(ApiError::BadRequest(format!("moodle_error: {}", v["error"].as_str().unwrap_or("login")))),
            }
        }
        _ => return Err(ApiError::bad_request("invalid_value")),
    };
    let client = Ws { base: base.clone(), token: token.clone() };
    let info = client.call("core_webservice_get_site_info", &[]).await?;
    let uid = info["userid"].as_i64().ok_or_else(|| ApiError::bad_request("moodle_bad_response"))?;
    let full_name = info["fullname"].as_str().unwrap_or("").to_string();
    let site_name = info["sitename"].as_str().unwrap_or("Moodle").to_string();

    // Reconnexion : l'ancien calendrier importé est remplacé.
    let old: Option<Option<Uuid>> = sqlx::query_scalar("SELECT calendar_id FROM moodle_connections WHERE workspace_id = $1 AND user_id = $2").bind(ws).bind(user.id).fetch_optional(&state.pool).await?;
    if let Some(Some(cal)) = old {
        sqlx::query("DELETE FROM calendars WHERE id = $1 AND user_id = $2").bind(cal).bind(user.id).execute(&state.pool).await?;
    }
    sqlx::query(
        "INSERT INTO moodle_connections (workspace_id, user_id, base_url, token_enc, moodle_user_id, full_name, site_name, last_sync_at, last_error, calendar_id, private_token_enc)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NULL, NULL, $8)
         ON CONFLICT (workspace_id, user_id) DO UPDATE SET base_url = $3, token_enc = $4, moodle_user_id = $5, full_name = $6, site_name = $7,
            last_sync_at = NOW(), last_error = NULL, calendar_id = NULL, last_notification_id = 0, last_message_id = 0, private_token_enc = $8",
    )
    .bind(ws)
    .bind(user.id)
    .bind(&base)
    .bind(crate::crypto::encrypt(&token))
    .bind(uid)
    .bind(&full_name)
    .bind(&site_name)
    .bind(private.as_deref().map(crate::crypto::encrypt))
    .execute(&state.pool)
    .await?;
    let calendar = import_calendar(&state, &client, uid, user.id, ws, &site_name).await;
    // Repère des notifications : seules les nouvelles seront signalées.
    let _ = poll_one(&state, ws, user.id, false).await;
    Ok(Json(json!({ "full_name": full_name, "site_name": site_name, "calendar": calendar.is_ok(), "calendar_error": calendar.err().map(|e| format!("{e:?}")) })))
}

/// Abonnement ICS personnel au calendrier Moodle (lien d'export avec jeton dédié).
async fn import_calendar(state: &AppState, client: &Ws, uid: i64, user_id: Uuid, ws: Uuid, site: &str) -> ApiResult<Uuid> {
    let t = client.call("core_calendar_get_calendar_export_token", &[]).await?;
    let auth = t["token"].as_str().ok_or_else(|| ApiError::bad_request("moodle_no_calendar_export"))?;
    let url = format!("{}/calendar/export_execute.php?userid={uid}&authtoken={auth}&preset_what=all&preset_time=recentupcoming", client.base);
    let host = reqwest::Url::parse(&url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO calendars (workspace_id, user_id, name, color, kind, url_enc, url_host, created_by)
         VALUES (NULL, $1, $2, '#f98012', 'ics', $3, $4, $1) RETURNING id",
    )
    .bind(user_id)
    .bind(site.chars().take(120).collect::<String>())
    .bind(crate::crypto::encrypt(&url))
    .bind(host.chars().take(255).collect::<String>())
    .fetch_one(&state.pool)
    .await?;
    sqlx::query("UPDATE moodle_connections SET calendar_id = $3 WHERE workspace_id = $1 AND user_id = $2").bind(ws).bind(user_id).bind(id).execute(&state.pool).await?;
    let _ = crate::calendar::sync_calendar(state, id).await;
    Ok(id)
}

async fn disconnect(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<StatusCode> {
    require_member(&state.pool, &user, ws).await?;
    let cal: Option<Option<Uuid>> = sqlx::query_scalar("DELETE FROM moodle_connections WHERE workspace_id = $1 AND user_id = $2 RETURNING calendar_id").bind(ws).bind(user.id).fetch_optional(&state.pool).await?;
    if let Some(Some(cal)) = cal {
        sqlx::query("DELETE FROM calendars WHERE id = $1 AND user_id = $2").bind(cal).bind(user.id).execute(&state.pool).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct PrefsReq {
    notify: bool,
}

async fn put_preferences(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<PrefsReq>) -> ApiResult<StatusCode> {
    require_member(&state.pool, &user, ws).await?;
    sqlx::query("UPDATE moodle_connections SET notify = $3 WHERE workspace_id = $1 AND user_id = $2").bind(ws).bind(user.id).bind(p.notify).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn sync_calendar(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Value>> {
    let (c, client, uid) = conn_of(&state, &user, ws).await?;
    let site: Option<String> = sqlx::query_scalar("SELECT site_name FROM moodle_connections WHERE id = $1").bind(c.id).fetch_one(&state.pool).await?;
    let id = match c.calendar_id {
        Some(id) => id,
        None => track(&state, c.id, import_calendar(&state, &client, uid, user.id, ws, site.as_deref().unwrap_or("Moodle")).await).await?,
    };
    let n = crate::calendar::sync_calendar(&state, id).await?;
    Ok(Json(json!({ "events": n })))
}

// ---------------------------------------------------------------------------
// Données
// ---------------------------------------------------------------------------

async fn courses(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    let (c, client, uid) = conn_of(&state, &user, ws).await?;
    let v = track(&state, c.id, client.call("core_enrol_get_users_courses", &[p("userid", uid)]).await).await?;
    let mut list: Vec<Value> = v
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|c| c["hidden"].as_i64().unwrap_or(0) == 0)
                .map(|c| {
                    json!({
                        "id": c["id"],
                        "fullname": c["displayname"].as_str().or(c["fullname"].as_str()),
                        "shortname": c["shortname"],
                        "category": c["categoryname"],
                        "progress": c["progress"],
                        "favourite": c["isfavourite"].as_bool().unwrap_or(false),
                        "lastaccess": c["lastaccess"],
                        "url": format!("{}/course/view.php?id={}", client.base, c["id"]),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    // Favoris, puis derniers consultés.
    list.sort_by_key(|c| (!c["favourite"].as_bool().unwrap_or(false), -c["lastaccess"].as_i64().unwrap_or(0)));
    Ok(Json(list))
}

/// Échéances à venir (et en retard depuis 14 jours) : devoirs, tests, rendez-vous…
async fn deadlines(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    let (c, client, _) = conn_of(&state, &user, ws).await?;
    let from = Utc::now().timestamp() - 14 * 86400;
    let v = track(&state, c.id, client.call("core_calendar_get_action_events_by_timesort", &[p("timesortfrom", from), p("limitnum", 50)]).await).await?;
    let now = Utc::now().timestamp();
    Ok(Json(
        v["events"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|e| e["timesort"].as_i64().unwrap_or(0) >= now || e["action"]["actionable"].as_bool().unwrap_or(true))
                    .map(|e| {
                        let ts = e["timesort"].as_i64().unwrap_or(0);
                        json!({
                            "id": e["id"],
                            "name": e["activityname"].as_str().or(e["name"].as_str()),
                            "event_name": e["name"],
                            "course": e["course"]["fullname"],
                            "course_id": e["course"]["id"],
                            "module": e["modulename"],
                            "due": DateTime::from_timestamp(ts, 0),
                            "overdue": ts < now,
                            "url": e["url"],
                            "action": e["action"]["name"],
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    ))
}

fn text_of(html: &str) -> String {
    crate::ical::html_to_text(html).split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn notifications(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Value>> {
    let (c, client, uid) = conn_of(&state, &user, ws).await?;
    let v = track(&state, c.id, client.call("message_popup_get_popup_notifications", &[p("useridto", uid), p("newestfirst", 1), p("limit", 30), p("offset", 0)]).await).await?;
    let list: Vec<Value> = v["notifications"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|n| {
                    json!({
                        "id": n["id"],
                        "subject": n["subject"],
                        "text": text_of(n["smallmessage"].as_str().or(n["fullmessage"].as_str()).unwrap_or("")),
                        "url": n["contexturl"],
                        "read": n["read"].as_bool().unwrap_or(false),
                        "time": DateTime::from_timestamp(n["timecreated"].as_i64().unwrap_or(0), 0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Json(json!({ "notifications": list, "unread": v["unreadcount"] })))
}

fn member_name(members: &Value, id: i64) -> String {
    members.as_array().and_then(|m| m.iter().find(|x| x["id"].as_i64() == Some(id))).and_then(|x| x["fullname"].as_str()).unwrap_or("").to_string()
}

async fn conversations(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    let (c, client, uid) = conn_of(&state, &user, ws).await?;
    let v = track(&state, c.id, client.call("core_message_get_conversations", &[p("userid", uid), p("limitfrom", 0), p("limitnum", 40)]).await).await?;
    Ok(Json(
        v["conversations"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|cv| {
                        let others: Vec<String> = cv["members"]
                            .as_array()
                            .map(|m| m.iter().filter(|x| x["id"].as_i64() != Some(uid)).filter_map(|x| x["fullname"].as_str().map(str::to_string)).collect())
                            .unwrap_or_default();
                        let last = cv["messages"].get(0);
                        json!({
                            "id": cv["id"],
                            "name": cv["name"].as_str().filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| others.join(", ")),
                            "group": cv["type"].as_i64() == Some(2),
                            "unread": cv["unreadcount"].as_i64().unwrap_or(0),
                            "last_text": last.map(|m| text_of(m["text"].as_str().unwrap_or(""))),
                            "last_from_me": last.map(|m| m["useridfrom"].as_i64() == Some(uid)),
                            "last_time": last.and_then(|m| DateTime::from_timestamp(m["timecreated"].as_i64().unwrap_or(0), 0)),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    ))
}

async fn conversation(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, i64)>) -> ApiResult<Json<Value>> {
    let (c, client, uid) = conn_of(&state, &user, ws).await?;
    let v = track(
        &state,
        c.id,
        client.call("core_message_get_conversation_messages", &[p("currentuserid", uid), p("convid", id), p("newest", 1), p("limitfrom", 0), p("limitnum", 60)]).await,
    )
    .await?;
    let mut messages: Vec<Value> = v["messages"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|m| {
                    let from = m["useridfrom"].as_i64().unwrap_or(0);
                    json!({
                        "id": m["id"],
                        "mine": from == uid,
                        "from": member_name(&v["members"], from),
                        "html": crate::mailbox::clean_html(m["text"].as_str().unwrap_or("")),
                        "time": DateTime::from_timestamp(m["timecreated"].as_i64().unwrap_or(0), 0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    messages.reverse();
    // Ouvrir la conversation la marque comme lue (comme dans Moodle).
    let _ = client.call("core_message_mark_all_conversation_messages_as_read", &[p("userid", uid), p("conversationid", id)]).await;
    Ok(Json(json!({ "id": id, "messages": messages })))
}

#[derive(Deserialize)]
struct SendReq {
    text: String,
}

async fn send_message(State(state): State<AppState>, user: AuthUser, Path((ws, id)): Path<(Uuid, i64)>, Json(b): Json<SendReq>) -> ApiResult<Json<Value>> {
    let (c, client, _) = conn_of(&state, &user, ws).await?;
    let text = b.text.trim();
    if text.is_empty() || text.chars().count() > 4000 {
        return Err(ApiError::bad_request("invalid_value"));
    }
    // Format 2 : texte brut (Moodle échappe et met en forme les retours à la ligne).
    let v = track(
        &state,
        c.id,
        client
            .call("core_message_send_messages_to_conversation", &[p("conversationid", id), p("messages[0][text]", text), p("messages[0][textformat]", 2)])
            .await,
    )
    .await?;
    Ok(Json(json!({ "sent": v.as_array().map(|a| a.len()).unwrap_or(0) })))
}

// ---------------------------------------------------------------------------
// Nouveaux messages et notifications
// ---------------------------------------------------------------------------

/// Relève une connexion ; `notify` faux = simple mise à jour des repères.
async fn poll_one(state: &AppState, ws: Uuid, user_id: Uuid, notify: bool) -> ApiResult<()> {
    let row: Option<(Uuid, String, String, Option<i64>, i64, i64, bool)> = sqlx::query_as(
        "SELECT id, base_url, token_enc, moodle_user_id, last_notification_id, last_message_id, notify FROM moodle_connections WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(ws)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((id, base, token_enc, uid, last_notif, last_msg, wants)) = row else { return Ok(()) };
    let token = crate::crypto::decrypt(&token_enc).ok_or_else(|| ApiError::bad_request("moodle_invalid_token"))?;
    let client = Ws { base, token };
    let uid = uid.unwrap_or(0);
    let notify = notify && wants;
    let r = async {
        let mut max_notif = last_notif;
        let n = client.call("message_popup_get_popup_notifications", &[p("useridto", uid), p("newestfirst", 1), p("limit", 20), p("offset", 0)]).await?;
        let mut fresh = Vec::new();
        for x in n["notifications"].as_array().into_iter().flatten() {
            let nid = x["id"].as_i64().unwrap_or(0);
            max_notif = max_notif.max(nid);
            if nid > last_notif && !x["read"].as_bool().unwrap_or(false) {
                fresh.push(json!({ "title": x["subject"], "id": nid }));
            }
        }
        let mut max_msg = last_msg;
        let cv = client.call("core_message_get_conversations", &[p("userid", uid), p("limitfrom", 0), p("limitnum", 20)]).await?;
        for c in cv["conversations"].as_array().into_iter().flatten() {
            let Some(m) = c["messages"].get(0) else { continue };
            let mid = m["id"].as_i64().unwrap_or(0);
            max_msg = max_msg.max(mid);
            let from = m["useridfrom"].as_i64().unwrap_or(0);
            if mid > last_msg && from != uid && c["unreadcount"].as_i64().unwrap_or(0) > 0 {
                let text: String = text_of(m["text"].as_str().unwrap_or("")).chars().take(140).collect();
                fresh.push(json!({ "title": text, "from": member_name(&c["members"], from), "conversation": c["id"] }));
            }
        }
        sqlx::query("UPDATE moodle_connections SET last_notification_id = $2, last_message_id = $3, last_error = NULL WHERE id = $1")
            .bind(id)
            .bind(max_notif)
            .bind(max_msg)
            .execute(&state.pool)
            .await?;
        if notify {
            for data in fresh.into_iter().take(5) {
                crate::notifications::notify(&state.pool, &state.hub, ws, user_id, None, "moodle", &format!("moodle:{id}"), data).await;
            }
        }
        ApiResult::Ok(())
    }
    .await;
    track(state, id, r).await
}

pub fn start_poll_loop(state: AppState) {
    let every = std::env::var("MOODLE_POLL_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(300u64).max(10);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(every)).await;
            let rows: Vec<(Uuid, Uuid)> = sqlx::query_as("SELECT workspace_id, user_id FROM moodle_connections WHERE notify AND (last_error IS NULL OR last_error NOT LIKE 'moodle_invalid_token%')")
                .fetch_all(&state.pool)
                .await
                .unwrap_or_default();
            for (ws, user) in rows {
                if let Err(e) = tokio::time::timeout(Duration::from_secs(60), poll_one(&state, ws, user, true)).await.unwrap_or(Err(ApiError::bad_request("moodle_timeout"))) {
                    tracing::debug!("moodle {ws}/{user}: {e:?}");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens() {
        let raw = "abcdef0123456789abcdef0123456789";
        let private = "fedcba9876543210fedcba9876543210";
        assert_eq!(parse_token(raw), Some((raw.to_string(), None)));
        let link = format!("moodlemobile://token={}", base64::engine::general_purpose::STANDARD.encode(format!("sig:::{raw}:::{private}")));
        assert_eq!(parse_token(&link), Some((raw.to_string(), Some(private.to_string()))));
        // Lien transmis par le gestionnaire de protocole du navigateur (encodé).
        let web = format!("web%2Blooty%3A%2F%2Ftoken%3D{}", base64::engine::general_purpose::STANDARD.encode(format!("sig:::{raw}")).replace('=', "%3D"));
        assert_eq!(parse_token(&web), Some((raw.to_string(), None)));
        assert_eq!(parse_token("nope"), None);
        assert_eq!(parse_token("moodlemobile://token=%%%"), None);
    }
}
