//! Contenu des cours Moodle dans Looty : sections et activités d'un cours, fichiers
//! (servis par Looty, qui ajoute le jeton : il ne quitte jamais le serveur), pages HTML
//! nettoyées et devoirs. Ce que Looty n'affiche pas (tests, forums, dépôts) s'ouvre sur le
//! site, connecté automatiquement.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderValue},
    response::Response,
    routing::get,
    Json, Router,
};
use chrono::DateTime;
use serde::Deserialize;
use serde_json::{json, Value};
use std::borrow::Cow;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::moodle::{conn_of, http, p, track};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/courses/{course_id}", get(course))
        .route("/courses/{course_id}/assign/{cmid}", get(assignment))
        .route("/file", get(file))
        .route("/html", get(page_html))
}

/// Adresse servie par Looty pour un fichier de la plateforme.
pub(crate) fn proxied(ws: Uuid, url: &str) -> String {
    format!("/api/workspaces/{ws}/moodle/file?url={}", crate::mailclient::pct(url))
}

/// Fichier de la plateforme (pluginfile) : seul ce qui vient de la plateforme connectée est
/// accepté ; l'adresse « webservice » est celle qui accepte le jeton.
fn plugin_url(base: &str, raw: &str) -> Option<reqwest::Url> {
    let rest = raw.strip_prefix(base)?;
    let rest = if let Some(r) = rest.strip_prefix("/pluginfile.php/") { format!("/webservice/pluginfile.php/{r}") } else if rest.starts_with("/webservice/pluginfile.php/") { rest.to_string() } else { return None };
    reqwest::Url::parse(&format!("{base}{rest}")).ok()
}

/// Le fichier vient bien de la plateforme `base` (pluginfile).
pub(crate) fn is_plugin_file(base: &str, raw: &str) -> bool {
    plugin_url(base, raw).is_some()
}

/// HTML de la plateforme, nettoyé ; images et fichiers passent par Looty, les adresses
/// relatives sont résolues depuis `page` (page d'un cours, chapitre d'un livre).
fn clean(html: &str, base: &str, ws: Uuid, page: Option<&reqwest::Url>) -> String {
    let base = base.to_string();
    let page = page.cloned();
    ammonia::Builder::default()
        .add_generic_attributes(["style", "align", "width", "height", "colspan", "rowspan"])
        .add_tags(["figure", "figcaption", "video", "audio", "source"])
        .add_tag_attributes("video", ["controls", "src", "poster"])
        .add_tag_attributes("audio", ["controls", "src"])
        .add_tag_attributes("source", ["src", "type"])
        .link_rel(Some("noopener noreferrer nofollow"))
        .set_tag_attribute_value("a", "target", "_blank")
        .url_schemes(["http", "https", "mailto", "data"].into_iter().collect())
        .attribute_filter(move |_el, attr, value| {
            if !matches!(attr, "src" | "href" | "poster") {
                return Some(Cow::Borrowed(value));
            }
            let abs = match &page {
                Some(pg) if !value.contains("://") && !value.starts_with('#') && !value.starts_with("mailto:") && !value.starts_with("data:") => {
                    pg.join(value).map(|u| u.to_string()).unwrap_or_else(|_| value.to_string())
                }
                _ => value.to_string(),
            };
            match plugin_url(&base, &abs) {
                Some(u) => Some(Cow::Owned(proxied(ws, u.as_str()))),
                None => Some(Cow::Owned(abs)),
            }
        })
        .clean(html)
        .to_string()
}

fn ts(v: &Value) -> Option<DateTime<chrono::Utc>> {
    v.as_i64().filter(|t| *t > 0).and_then(|t| DateTime::from_timestamp(t, 0))
}

/// Sections et activités d'un cours (ce que l'étudiant voit sur la page du cours).
async fn course_live(State(state): State<AppState>, user: AuthUser, Path((ws, course_id)): Path<(Uuid, i64)>) -> ApiResult<Json<Value>> {
    let (c, client, _) = conn_of(&state, &user, ws).await?;
    let v = track(&state, c.id, client.call("core_course_get_contents", &[p("courseid", course_id)]).await).await?;
    let base = client.base.clone();
    let sections: Vec<Value> = v
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|s| s["uservisible"].as_bool().unwrap_or(true))
                .map(|s| {
                    let modules: Vec<Value> = s["modules"]
                        .as_array()
                        .map(|m| {
                            m.iter()
                                .filter(|m| m["uservisible"].as_bool().unwrap_or(true) && m["visible"].as_i64().unwrap_or(1) != 0)
                                .map(|m| module(m, &base, ws))
                                .collect()
                        })
                        .unwrap_or_default();
                    json!({
                        "id": s["id"],
                        "name": s["name"],
                        "summary": clean(s["summary"].as_str().unwrap_or(""), &base, ws, None),
                        "modules": modules,
                    })
                })
                .filter(|s| !s["modules"].as_array().is_some_and(|m| m.is_empty()) || !s["summary"].as_str().unwrap_or("").trim().is_empty())
                .collect()
        })
        .unwrap_or_default();
    Ok(Json(json!({ "id": course_id, "url": format!("{base}/course/view.php?id={course_id}"), "sections": sections })))
}

fn module(m: &Value, base: &str, ws: Uuid) -> Value {
    let modname = m["modname"].as_str().unwrap_or("");
    let contents = m["contents"].as_array().cloned().unwrap_or_default();
    // Lien externe (activité URL) : l'adresse est le seul « contenu ».
    let link = if modname == "url" { contents.iter().find(|f| f["type"] == "url").and_then(|f| f["fileurl"].as_str()).map(str::to_string) } else { None };
    let files: Vec<Value> = contents
        .iter()
        .filter(|f| f["type"] == "file")
        .filter_map(|f| {
            let url = f["fileurl"].as_str()?;
            let path = f["filepath"].as_str().unwrap_or("/");
            Some(json!({
                "name": f["filename"],
                "path": path,
                "size": f["filesize"],
                "mime": f["mimetype"],
                "time": ts(&f["timemodified"]),
                "url": plugin_url(base, url).map(|u| proxied(ws, u.as_str())),
                // Page HTML (page, chapitre de livre) : lue dans Looty.
                "html": f["filename"].as_str() == Some("index.html"),
                "source": url,
            }))
        })
        .collect();
    let dates: Vec<Value> = m["dates"].as_array().map(|d| d.iter().map(|x| json!({ "label": x["label"], "time": ts(&x["timestamp"]) })).collect()).unwrap_or_default();
    json!({
        "id": m["id"],
        "name": m["name"],
        "type": modname,
        "type_name": m["modplural"],
        "url": m["url"].as_str().map(str::to_string).unwrap_or_else(|| format!("{base}/mod/{modname}/view.php?id={}", m["id"])),
        "description": clean(m["description"].as_str().unwrap_or(""), base, ws, None),
        "files": files,
        "link": link,
        "dates": dates,
        "done": m["completiondata"]["state"].as_i64().map(|s| s > 0),
    })
}

#[derive(Deserialize)]
struct UrlQuery {
    url: String,
    #[serde(default)]
    download: bool,
}

async fn fetch(client: &crate::moodle::Ws, raw: &str) -> ApiResult<reqwest::Response> {
    let mut url = plugin_url(&client.base, raw).ok_or_else(|| ApiError::bad_request("moodle_bad_file"))?;
    url.query_pairs_mut().append_pair("token", &client.token);
    let r = http().get(url).send().await.map_err(|_| ApiError::bad_request("moodle_unreachable"))?;
    match r.status().as_u16() {
        200..=299 => Ok(r),
        403 | 404 => Err(ApiError::NotFound),
        s => Err(ApiError::BadRequest(format!("moodle_http_error: {s}"))),
    }
}

async fn file(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<UrlQuery>) -> ApiResult<Response> {
    let (_, client, _) = conn_of(&state, &user, ws).await?;
    let name = q.url.split('?').next().unwrap_or("").rsplit('/').next().map(crate::moodle::pct_decode).unwrap_or_else(|| "file".into());
    let r = match fetch(&client, &q.url).await {
        Ok(r) => r,
        // Plateforme injoignable : la copie du fichier, s'il a déjà été ouvert.
        Err(e) if crate::moodle_cache::is_outage(&e) => {
            let (mime, cached_name, bytes, at) = crate::moodle_cache::recall_file(&state, ws, user.id, &q.url).await.ok_or(e)?;
            let len = bytes.len() as u64;
            let resp = file_response(Body::from(bytes), &mime, Some(len), &cached_name, q.download);
            return Ok(crate::moodle_cache::offline(resp, at));
        }
        Err(e) => return Err(e),
    };
    let mime = r.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("application/octet-stream").to_string();
    let len = r.content_length();
    if len.is_some_and(|l| l > crate::moodle_cache::max_file()) {
        return Ok(file_response(Body::from_stream(r.bytes_stream()), &mime, len, &name, q.download));
    }
    // Taille raisonnable (ou inconnue) : lu jusqu'à la limite et gardé pour la consultation
    // hors ligne ; au-delà, ce qui est lu est renvoyé suivi du reste, sans copie.
    use futures::StreamExt;
    let mut stream = r.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ApiError::bad_request("moodle_unreachable"))?;
        buf.extend_from_slice(&chunk);
        if buf.len() as u64 > crate::moodle_cache::max_file() {
            let head = futures::stream::once(async move { Ok::<_, reqwest::Error>(bytes::Bytes::from(buf)) });
            return Ok(file_response(Body::from_stream(head.chain(stream)), &mime, len, &name, q.download));
        }
    }
    crate::moodle_cache::remember_file(&state, ws, user.id, &q.url, &mime, &name, &buf).await;
    let n = buf.len() as u64;
    Ok(file_response(Body::from(buf), &mime, Some(n), &name, q.download))
}

/// Réponse d'un fichier de la plateforme : rien n'est exécuté depuis l'origine de Looty
/// (HTML et SVG toujours téléchargés, tout est servi en bac à sable).
fn file_response(body: Body, mime: &str, len: Option<u64>, name: &str, download: bool) -> Response {
    let risky = mime.contains("html") || mime.contains("svg") || mime.contains("xml") || mime.contains("javascript");
    let disposition = if download || risky { "attachment" } else { "inline" };
    let mut resp = Response::new(body);
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_str(if risky { "application/octet-stream" } else { mime }).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    if let Some(l) = len {
        h.insert(header::CONTENT_LENGTH, HeaderValue::from(l));
    }
    let fname = name.replace(['"', '\\', '\r', '\n'], "_");
    if let Ok(v) = HeaderValue::from_str(&format!("{disposition}; filename*=UTF-8''{}", crate::mailclient::pct(&fname))) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox; default-src 'none'; img-src data:; style-src 'unsafe-inline'"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=3600"));
    resp
}

/// Page HTML d'une activité (page, chapitre de livre), nettoyée pour être lue dans Looty.
async fn page_html_live(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<UrlQuery>) -> ApiResult<Json<Value>> {
    let (c, client, _) = conn_of(&state, &user, ws).await?;
    let page = plugin_url(&client.base, &q.url).ok_or_else(|| ApiError::bad_request("moodle_bad_file"))?;
    let r = track(&state, c.id, fetch(&client, &q.url).await).await?;
    let bytes = r.bytes().await.map_err(|_| ApiError::bad_request("moodle_unreachable"))?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err(ApiError::bad_request("moodle_too_large"));
    }
    let html = String::from_utf8_lossy(&bytes);
    Ok(Json(json!({ "html": clean(&html, &client.base, ws, Some(&page)) })))
}

/// Devoir : consigne, dates, état de la remise et note (le dépôt se fait sur le site).
async fn assignment_live(State(state): State<AppState>, user: AuthUser, Path((ws, course_id, cmid)): Path<(Uuid, i64, i64)>) -> ApiResult<Json<Value>> {
    let (c, client, _) = conn_of(&state, &user, ws).await?;
    let v = track(&state, c.id, client.call("mod_assign_get_assignments", &[p("courseids[0]", course_id)]).await).await?;
    let a = v["courses"]
        .as_array()
        .and_then(|cs| cs.iter().flat_map(|c| c["assignments"].as_array().cloned().unwrap_or_default()).find(|a| a["cmid"].as_i64() == Some(cmid)))
        .ok_or(ApiError::NotFound)?;
    let base = client.base.clone();
    let attachments: Vec<Value> = a["introattachments"]
        .as_array()
        .map(|l| {
            l.iter()
                .filter_map(|f| Some(json!({ "name": f["filename"], "size": f["filesize"], "mime": f["mimetype"], "url": plugin_url(&base, f["fileurl"].as_str()?).map(|u| proxied(ws, u.as_str())) })))
                .collect()
        })
        .unwrap_or_default();
    // L'état de la remise n'est pas toujours accessible (rôle, réglages) : facultatif.
    let s = client.call("mod_assign_get_submission_status", &[p("assignid", &a["id"])]).await.ok();
    let last = s.as_ref().map(|s| &s["lastattempt"]);
    Ok(Json(json!({
        "id": a["id"],
        "cmid": cmid,
        "name": a["name"],
        "intro": clean(a["intro"].as_str().unwrap_or(""), &base, ws, None),
        "attachments": attachments,
        "opens": ts(&a["allowsubmissionsfromdate"]),
        "due": ts(&a["duedate"]),
        "cutoff": ts(&a["cutoffdate"]),
        "status": last.and_then(|l| l["submission"]["status"].as_str().or(l["teamsubmission"]["status"].as_str())),
        "grading": last.and_then(|l| l["gradingstatus"].as_str()),
        "grade": s.as_ref().and_then(|s| s["feedback"]["gradefordisplay"].as_str()).map(|g| crate::ical::html_to_text(g)),
        "url": format!("{base}/mod/assign/view.php?id={cmid}"),
    })))
}

// ---------------------------------------------------------------------------
// Copie hors ligne (voir moodle_cache)
// ---------------------------------------------------------------------------

async fn course(State(state): State<AppState>, user: AuthUser, Path((ws, course_id)): Path<(Uuid, i64)>) -> ApiResult<Response> {
    let live = course_live(State(state.clone()), user, Path((ws, course_id))).await.map(|Json(v)| v);
    crate::moodle_cache::json_or_cached(&state, ws, user.id, &format!("course:{course_id}"), live).await
}

async fn page_html(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Query(q): Query<UrlQuery>) -> ApiResult<Response> {
    let key = format!("html:{}", q.url);
    let live = page_html_live(State(state.clone()), user, Path(ws), Query(q)).await.map(|Json(v)| v);
    crate::moodle_cache::json_or_cached(&state, ws, user.id, &key, live).await
}

async fn assignment(State(state): State<AppState>, user: AuthUser, Path((ws, course_id, cmid)): Path<(Uuid, i64, i64)>) -> ApiResult<Response> {
    let live = assignment_live(State(state.clone()), user, Path((ws, course_id, cmid))).await.map(|Json(v)| v);
    crate::moodle_cache::json_or_cached(&state, ws, user.id, &format!("assign:{course_id}:{cmid}"), live).await
}
