//! Client de messagerie complet, au-dessus des comptes de `mailbox` :
//!
//! - dossiers et sous-dossiers (LIST + rôles spéciaux RFC 6154 ou noms usuels, compteurs
//!   via STATUS), création / renommage / suppression ;
//! - liste paginée et recherche (UID SEARCH côté serveur) dans n'importe quel dossier ;
//! - lecture (HTML nettoyé, images `cid:` servies par Looty), pièces jointes ;
//! - lu / non lu, suivi (étoile), déplacement, corbeille (suppression définitive depuis la
//!   corbeille), vider un dossier ;
//! - envoi via SMTP (réponse, transfert avec pièces jointes d'origine, pièces jointes
//!   envoyées sur Looty, images de l'éditeur intégrées) puis copie dans « Envoyés ».
//!
//! Pour rester léger : rien n'est stocké hors de la boîte de réception (déjà en cache pour
//! les notifications), et les connexions IMAP sont réutilisées quelques dizaines de
//! secondes au lieu de se reconnecter à chaque clic.

use async_imap::types::{Flag, NameAttribute};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use mail_parser::{MessageParser, MimeHeaders};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::mailbox::{clean_html, connect_account, imap_err, own_account, Account, Imap};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/contacts", get(contacts))
        .route("/accounts/:id/folders", get(list_folders).post(create_folder))
        .route("/accounts/:id/folders/rename", post(rename_folder))
        .route("/accounts/:id/folders/delete", post(delete_folder))
        .route("/accounts/:id/folders/empty", post(empty_folder))
        .route("/accounts/:id/folder", get(list_folder))
        .route("/accounts/:id/folder/message", get(read_message))
        .route("/accounts/:id/folder/attachment", get(attachment))
        .route("/accounts/:id/folder/flags", post(set_flags))
        .route("/accounts/:id/folder/move", post(move_messages))
        .route("/accounts/:id/folder/delete", post(delete_messages))
        .route("/accounts/:id/send", post(send))
}

// ---------------------------------------------------------------------------
// Connexions IMAP réutilisées
// ---------------------------------------------------------------------------

const IDLE: Duration = Duration::from_secs(60);
static POOL: LazyLock<Mutex<HashMap<Uuid, Vec<(Imap, Instant)>>>> = LazyLock::new(Default::default);
static REAPER: OnceLock<()> = OnceLock::new();

/// Session IMAP pour un compte : une connexion restée ouverte si elle répond encore,
/// sinon une nouvelle.
async fn open(a: &Account) -> ApiResult<Imap> {
    REAPER.get_or_init(|| {
        tokio::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(20)).await;
                let mut p = POOL.lock().unwrap();
                p.values_mut().for_each(|v| v.retain(|(_, at)| at.elapsed() < IDLE));
                p.retain(|_, v| !v.is_empty());
            }
        });
    });
    loop {
        let idle = POOL.lock().unwrap().get_mut(&a.id).and_then(|v| v.pop());
        let Some((mut s, at)) = idle else { break };
        if at.elapsed() < IDLE && matches!(tokio::time::timeout(Duration::from_secs(5), s.noop()).await, Ok(Ok(()))) {
            drain(&s);
            return Ok(s);
        }
    }
    connect_account(a).await
}

/// Rend une session après usage (au plus deux par compte), sans dossier sélectionné :
/// un dossier renommé ou supprimé ailleurs ferait sinon couper la connexion par le
/// serveur. Refus du serveur (rien de sélectionné) : la session reste utilisable ;
/// connexion perdue ou délai dépassé : elle est abandonnée.
async fn release(id: Uuid, mut s: Imap) {
    use async_imap::error::Error;
    match tokio::time::timeout(Duration::from_secs(5), s.run_command_and_check_ok("UNSELECT")).await {
        Ok(Ok(())) | Ok(Err(Error::No(_) | Error::Bad(_))) => {}
        _ => return,
    }
    drain(&s);
    let mut p = POOL.lock().unwrap();
    let v = p.entry(id).or_default();
    if v.len() < 2 {
        v.push((s, Instant::now()));
    }
}

/// Vide les réponses non sollicitées (EXISTS, EXPUNGE…) : la file d'async-imap est
/// bornée (100) et bloquerait une session réutilisée si personne ne la lisait.
fn drain(s: &Imap) {
    while s.unsolicited_responses.try_recv().is_ok() {}
}

/// Oublie les connexions d'un compte (mot de passe changé, compte supprimé).
pub(crate) fn forget(id: Uuid) {
    POOL.lock().unwrap().remove(&id);
}

// ---------------------------------------------------------------------------
// Noms de dossiers (UTF-7 modifié, RFC 3501 §5.1.3)
// ---------------------------------------------------------------------------

fn b64() -> base64::engine::GeneralPurpose {
    use base64::engine::{general_purpose::GeneralPurposeConfig, DecodePaddingMode};
    base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new().with_encode_padding(false).with_decode_allow_trailing_bits(true).with_decode_padding_mode(DecodePaddingMode::Indifferent),
    )
}

pub(crate) fn decode_mutf7(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        let end = rest.find('-').unwrap_or(rest.len());
        let chunk = &rest[..end];
        rest = &rest[(end + 1).min(rest.len())..];
        if chunk.is_empty() {
            out.push('&');
            continue;
        }
        match b64().decode(chunk.replace(',', "/")) {
            Ok(bytes) => {
                let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                out.push_str(&String::from_utf16_lossy(&units));
            }
            Err(_) => {
                out.push('&');
                out.push_str(chunk);
                out.push('-');
            }
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn encode_mutf7(s: &str) -> String {
    let mut out = String::new();
    let mut run: Vec<u16> = Vec::new();
    let flush = |run: &mut Vec<u16>, out: &mut String| {
        if !run.is_empty() {
            let bytes: Vec<u8> = run.iter().flat_map(|u| u.to_be_bytes()).collect();
            out.push('&');
            out.push_str(&b64().encode(bytes).replace('/', ","));
            out.push('-');
            run.clear();
        }
    };
    for c in s.chars() {
        if (' '..='~').contains(&c) {
            flush(&mut run, &mut out);
            if c == '&' {
                out.push_str("&-");
            } else {
                out.push(c);
            }
        } else {
            let mut buf = [0u16; 2];
            run.extend_from_slice(c.encode_utf16(&mut buf));
        }
    }
    flush(&mut run, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Dossiers
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
struct Folder {
    /// Nom brut sur le serveur (à renvoyer tel quel).
    name: String,
    /// Dernier segment, décodé.
    label: String,
    parent: Option<String>,
    role: Option<&'static str>,
    selectable: bool,
    total: u32,
    unread: u32,
}

fn role_by_attr(attrs: &[NameAttribute]) -> Option<&'static str> {
    attrs.iter().find_map(|a| match a {
        NameAttribute::Sent => Some("sent"),
        NameAttribute::Trash => Some("trash"),
        NameAttribute::Drafts => Some("drafts"),
        NameAttribute::Junk => Some("junk"),
        NameAttribute::Archive => Some("archive"),
        NameAttribute::All => Some("all"),
        NameAttribute::Flagged => Some("flagged"),
        _ => None,
    })
}

/// Rôle déduit du nom, pour les serveurs sans « special-use ».
fn role_by_name(label: &str) -> Option<&'static str> {
    let l = label.to_lowercase();
    Some(match l.as_str() {
        "sent" | "sent items" | "sent messages" | "sent mail" | "envoyés" | "envoyes" | "messages envoyés" | "éléments envoyés" => "sent",
        "trash" | "deleted items" | "deleted messages" | "bin" | "corbeille" | "éléments supprimés" => "trash",
        "drafts" | "draft" | "brouillons" => "drafts",
        "junk" | "spam" | "junk e-mail" | "junk email" | "courrier indésirable" | "indésirables" | "pourriel" => "junk",
        "archive" | "archives" => "archive",
        _ => return None,
    })
}

/// Liste des dossiers avec leur rôle (sans compteurs).
async fn folders_of(s: &mut Imap) -> ApiResult<(String, Vec<Folder>)> {
    let names: Vec<_> = s.list(Some(""), Some("*")).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    // Toujours au moins INBOX : une liste vide signale une connexion fermée.
    if names.is_empty() {
        return Err(imap_err("empty folder list"));
    }
    let delimiter = names.iter().find_map(|n| n.delimiter().map(str::to_string)).unwrap_or_else(|| "/".into());
    let mut out: Vec<Folder> = names
        .iter()
        .map(|n| {
            let name = n.name().to_string();
            let d = n.delimiter().unwrap_or(&delimiter);
            let (parent, leaf) = match name.rsplit_once(d) {
                Some((p, l)) if !p.is_empty() => (Some(p.to_string()), l.to_string()),
                _ => (None, name.clone()),
            };
            let inbox = name.eq_ignore_ascii_case("INBOX");
            Folder {
                label: if inbox { "INBOX".into() } else { decode_mutf7(&leaf) },
                parent,
                role: if inbox { Some("inbox") } else { role_by_attr(n.attributes()) },
                selectable: !n.attributes().iter().any(|a| matches!(a, NameAttribute::NoSelect)),
                name,
                total: 0,
                unread: 0,
            }
        })
        .collect();
    // Rôles sans attribut : premier dossier au nom usuel.
    for role in ["sent", "trash", "drafts", "junk", "archive"] {
        if !out.iter().any(|f| f.role == Some(role)) {
            if let Some(f) = out.iter_mut().find(|f| f.role.is_none() && f.selectable && role_by_name(&f.label) == Some(role)) {
                f.role = Some(role);
            }
        }
    }
    // Boîte de réception, rôles connus, puis ordre alphabétique (sous-dossiers après leur parent).
    let rank = |f: &Folder| match f.role {
        Some("inbox") => 0,
        Some("drafts") => 1,
        Some("sent") => 2,
        Some("archive") => 3,
        Some("junk") => 4,
        Some("trash") => 5,
        _ => 6,
    };
    out.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok((delimiter, out))
}

async fn find_role(s: &mut Imap, role: &str) -> ApiResult<Option<String>> {
    Ok(folders_of(s).await?.1.into_iter().find(|f| f.role == Some(role)).map(|f| f.name))
}

async fn list_folders(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let mut s = open(&a).await?;
    let (delimiter, mut folders) = folders_of(&mut s).await?;
    for f in folders.iter_mut().filter(|f| f.selectable) {
        if let Ok(st) = s.status(&f.name, "(MESSAGES UNSEEN)").await {
            f.total = st.exists;
            f.unread = st.unseen.unwrap_or(0);
        }
    }
    release(a.id, s).await;
    Ok(Json(serde_json::json!({ "delimiter": delimiter, "folders": folders })))
}

fn folder_label_ok(l: &str) -> bool {
    !l.is_empty() && l.chars().count() <= 100 && !l.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | '"' | '*' | '%'))
}

#[derive(Deserialize)]
struct CreateFolderReq {
    parent: Option<String>,
    label: String,
}

async fn create_folder(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<CreateFolderReq>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let label = p.label.trim();
    if !folder_label_ok(label) {
        return Err(ApiError::bad_request("mail_invalid_folder"));
    }
    let mut s = open(&a).await?;
    let (delimiter, _) = folders_of(&mut s).await?;
    if label.contains(delimiter.as_str()) {
        return Err(ApiError::bad_request("mail_invalid_folder"));
    }
    let name = match p.parent.as_deref().filter(|x| !x.is_empty()) {
        Some(parent) => format!("{parent}{delimiter}{}", encode_mutf7(label)),
        None => encode_mutf7(label),
    };
    s.create(&name).await.map_err(|_| ApiError::bad_request("mail_folder_failed"))?;
    let _ = s.subscribe(&name).await;
    release(a.id, s).await;
    Ok(Json(serde_json::json!({ "name": name })))
}

#[derive(Deserialize)]
struct RenameFolderReq {
    name: String,
    label: String,
}

async fn rename_folder(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<RenameFolderReq>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let label = p.label.trim();
    if !folder_label_ok(label) || p.name.eq_ignore_ascii_case("INBOX") {
        return Err(ApiError::bad_request("mail_invalid_folder"));
    }
    let mut s = open(&a).await?;
    let (delimiter, _) = folders_of(&mut s).await?;
    if label.contains(delimiter.as_str()) {
        return Err(ApiError::bad_request("mail_invalid_folder"));
    }
    let to = match p.name.rsplit_once(delimiter.as_str()) {
        Some((parent, _)) if !parent.is_empty() => format!("{parent}{delimiter}{}", encode_mutf7(label)),
        _ => encode_mutf7(label),
    };
    s.rename(&p.name, &to).await.map_err(|_| ApiError::bad_request("mail_folder_failed"))?;
    let _ = s.subscribe(&to).await;
    release(a.id, s).await;
    Ok(Json(serde_json::json!({ "name": to })))
}

#[derive(Deserialize)]
struct FolderReq {
    name: String,
}

async fn delete_folder(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<FolderReq>) -> ApiResult<StatusCode> {
    let a = own_account(&state, &user, id).await?;
    if p.name.eq_ignore_ascii_case("INBOX") {
        return Err(ApiError::bad_request("mail_invalid_folder"));
    }
    let mut s = open(&a).await?;
    s.delete(&p.name).await.map_err(|_| ApiError::bad_request("mail_folder_failed"))?;
    release(a.id, s).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Vide un dossier (corbeille, indésirables) : suppression définitive.
async fn empty_folder(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<FolderReq>) -> ApiResult<StatusCode> {
    let a = own_account(&state, &user, id).await?;
    let mut s = open(&a).await?;
    let mb = s.select(&p.name).await.map_err(imap_err)?;
    if mb.exists > 0 {
        let _: Vec<_> = s.uid_store("1:*", "+FLAGS.SILENT (\\Deleted)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
        let _: Vec<_> = s.expunge().await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    }
    release(a.id, s).await;
    if is_inbox(&p.name) {
        sqlx::query("DELETE FROM mail_messages WHERE account_id = $1").bind(id).execute(&state.pool).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

fn is_inbox(name: &str) -> bool {
    name.eq_ignore_ascii_case("INBOX")
}

/// Encodage pour URL (paramètre de requête, nom de fichier RFC 5987).
pub(crate) fn pct(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ListQuery {
    name: String,
    before: Option<u32>,
    q: Option<String>,
    unread: Option<bool>,
    flagged: Option<bool>,
}

#[derive(Serialize)]
struct Summary {
    uid: u32,
    from_name: String,
    from_addr: String,
    to: Vec<String>,
    subject: String,
    snippet: String,
    sent_at: Option<DateTime<Utc>>,
    seen: bool,
    flagged: bool,
    answered: bool,
    has_attachments: bool,
}

const PAGE: usize = 40;

/// Chaîne IMAP entre guillemets (les recherches non ASCII passent en UTF-8).
fn imap_quote(s: &str) -> String {
    let clean: String = s.chars().filter(|c| !c.is_control()).take(200).collect();
    format!("\"{}\"", clean.replace('\\', "\\\\").replace('"', "\\\""))
}

fn uid_set(uids: &[u32]) -> ApiResult<String> {
    if uids.is_empty() || uids.len() > 1000 {
        return Err(ApiError::bad_request("invalid_value"));
    }
    Ok(uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(","))
}

fn summarize(uid: u32, f: &async_imap::types::Fetch) -> Summary {
    let raw = f.body().unwrap_or_default();
    let msg = MessageParser::default().parse(raw);
    let first = msg.as_ref().and_then(|m| m.from()).and_then(|f| f.first());
    let to = msg
        .as_ref()
        .and_then(|m| m.to())
        .map(|l| l.iter().map(|a| a.name().or(a.address()).unwrap_or("").to_string()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    Summary {
        uid,
        from_name: first.and_then(|a| a.name()).unwrap_or("").to_string(),
        from_addr: first.and_then(|a| a.address()).unwrap_or("").to_string(),
        to,
        subject: msg.as_ref().and_then(|m| m.subject()).unwrap_or("").chars().take(300).collect(),
        snippet: msg
            .as_ref()
            .and_then(|m| m.body_text(0))
            .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(180).collect())
            .unwrap_or_default(),
        sent_at: msg
            .as_ref()
            .and_then(|m| m.date())
            .and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0))
            .or_else(|| f.internal_date().map(|d| d.with_timezone(&Utc))),
        seen: f.flags().any(|x| matches!(x, Flag::Seen)),
        flagged: f.flags().any(|x| matches!(x, Flag::Flagged)),
        answered: f.flags().any(|x| matches!(x, Flag::Answered)),
        has_attachments: msg.as_ref().is_some_and(|m| m.attachment_count() > 0),
    }
}

/// Une page d'un dossier (40 messages, du plus récent au plus ancien), avec filtre et
/// recherche côté serveur.
async fn list_folder(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<ListQuery>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let mut s = open(&a).await?;
    let mb = s.select(&q.name).await.map_err(imap_err)?;
    let mut criteria = Vec::new();
    if q.unread == Some(true) {
        criteria.push("UNSEEN".to_string());
    }
    if q.flagged == Some(true) {
        criteria.push("FLAGGED".to_string());
    }
    let search = q.q.as_deref().map(str::trim).filter(|x| !x.is_empty());
    if let Some(text) = search {
        criteria.push(format!("TEXT {}", imap_quote(text)));
    }
    let mut uids: Vec<u32> = if mb.exists == 0 {
        Vec::new()
    } else {
        let mut query = if criteria.is_empty() { "ALL".to_string() } else { criteria.join(" ") };
        if search.is_some_and(|t| !t.is_ascii()) {
            query = format!("CHARSET UTF-8 {query}");
        }
        s.uid_search(&query).await.map_err(imap_err)?.into_iter().collect()
    };
    uids.retain(|u| q.before.is_none_or(|b| *u < b));
    uids.sort_unstable_by(|a, b| b.cmp(a));
    let has_more = uids.len() > PAGE;
    uids.truncate(PAGE);
    let mut messages = Vec::new();
    if !uids.is_empty() {
        let fetched: Vec<_> = s
            .uid_fetch(uid_set(&uids)?, "(UID FLAGS INTERNALDATE BODY.PEEK[]<0.16384>)")
            .await
            .map_err(imap_err)?
            .try_collect()
            .await
            .map_err(imap_err)?;
        for f in &fetched {
            if let Some(uid) = f.uid {
                messages.push(summarize(uid, f));
            }
        }
        messages.sort_by(|a, b| b.uid.cmp(&a.uid));
    }
    release(a.id, s).await;
    Ok(Json(serde_json::json!({ "messages": messages, "has_more": has_more, "total": mb.exists })))
}

#[derive(Deserialize)]
struct MsgQuery {
    name: String,
    uid: u32,
}

async fn fetch_raw(s: &mut Imap, folder: &str, uid: u32, peek: bool) -> ApiResult<Vec<u8>> {
    s.select(folder).await.map_err(imap_err)?;
    let item = if peek { "(UID BODY.PEEK[])" } else { "(UID FLAGS BODY[])" };
    let fetches: Vec<_> = s.uid_fetch(uid.to_string(), item).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    fetches.iter().find(|f| f.uid == Some(uid)).and_then(|f| f.body()).map(<[u8]>::to_vec).ok_or(ApiError::NotFound)
}

fn addrs(list: Option<&mail_parser::Address>) -> Vec<serde_json::Value> {
    list.map(|l| l.iter().map(|a| serde_json::json!({ "name": a.name(), "address": a.address() })).collect()).unwrap_or_default()
}

fn header_ids(v: &mail_parser::HeaderValue) -> Vec<String> {
    match v {
        mail_parser::HeaderValue::Text(t) => vec![format!("<{t}>")],
        mail_parser::HeaderValue::TextList(l) => l.iter().map(|t| format!("<{t}>")).collect(),
        _ => Vec::new(),
    }
}

async fn read_message(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<MsgQuery>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let mut s = open(&a).await?;
    // BODY[] (sans PEEK) : le message est marqué comme lu, comme dans un client classique.
    let raw = fetch_raw(&mut s, &q.name, q.uid, false).await?;
    let flags: Vec<_> = s.uid_fetch(q.uid.to_string(), "(UID FLAGS)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    release(a.id, s).await;
    let flagged = flags.iter().any(|f| f.flags().any(|x| matches!(x, Flag::Flagged)));
    let msg = MessageParser::default().parse(&raw).ok_or_else(|| ApiError::bad_request("mail_unreadable"))?;
    let att_url = |index: usize| {
        format!("/api/mail/accounts/{id}/folder/attachment?name={}&uid={}&index={index}", pct(&q.name), q.uid)
    };
    // Vrai HTML seulement : pour un message texte, mail-parser fabrique un HTML (sans liens).
    let mut html = msg.html_part(0).filter(|p| p.is_text_html()).and_then(|_| msg.body_html(0)).map(|h| clean_html(&h));
    let mut attachments = Vec::new();
    for (index, p) in msg.attachments().enumerate() {
        let cid = p.content_id().map(|c| c.trim_matches(['<', '>']).to_string());
        let inline_used = match (&cid, html.as_mut()) {
            (Some(cid), Some(h)) if h.contains(&format!("cid:{cid}")) => {
                *h = h.replace(&format!("cid:{cid}"), &format!("{}&inline=1", att_url(index)).replace('&', "&amp;"));
                true
            }
            _ => false,
        };
        if !inline_used {
            attachments.push(serde_json::json!({
                "index": index,
                "name": p.attachment_name().unwrap_or("attachment"),
                "size": p.len(),
                "type": p.content_type().map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or(""))),
            }));
        }
    }
    let text = if html.is_none() { msg.body_text(0).map(|t| t.to_string()) } else { None };
    if is_inbox(&q.name) {
        sqlx::query("UPDATE mail_messages SET seen = TRUE WHERE account_id = $1 AND uid = $2").bind(id).bind(q.uid as i64).execute(&state.pool).await?;
    }
    Ok(Json(serde_json::json!({
        "uid": q.uid,
        "folder": q.name,
        "subject": msg.subject(),
        "from": addrs(msg.from()),
        "to": addrs(msg.to()),
        "cc": addrs(msg.cc()),
        "reply_to": addrs(msg.reply_to()),
        "date": msg.date().and_then(|d| DateTime::from_timestamp(d.to_timestamp(), 0)),
        "message_id": msg.message_id().map(|m| format!("<{m}>")),
        "references": header_ids(msg.references()),
        "flagged": flagged,
        "html": html,
        "text": text,
        "attachments": attachments,
    })))
}

#[derive(Deserialize)]
struct AttQuery {
    name: String,
    uid: u32,
    index: usize,
    inline: Option<u8>,
}

/// Pièce jointe d'un message (images intégrées affichées, le reste téléchargé).
async fn attachment(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<AttQuery>) -> ApiResult<Response> {
    let a = own_account(&state, &user, id).await?;
    let mut s = open(&a).await?;
    let raw = fetch_raw(&mut s, &q.name, q.uid, true).await?;
    release(a.id, s).await;
    let msg = MessageParser::default().parse(&raw).ok_or_else(|| ApiError::bad_request("mail_unreadable"))?;
    let part = msg.attachments().nth(q.index).ok_or(ApiError::NotFound)?;
    let mime = part
        .content_type()
        .map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("octet-stream")))
        .unwrap_or_else(|| "application/octet-stream".into());
    let name = part.attachment_name().unwrap_or("attachment").replace(['"', '\r', '\n'], "");
    let image = mime.starts_with("image/") && !mime.contains("svg");
    let disposition = if q.inline == Some(1) && image {
        "inline".to_string()
    } else {
        format!("attachment; filename*=UTF-8''{}", pct(&name))
    };
    let body = part.contents().to_vec();
    Ok((
        [
            (header::CONTENT_TYPE, if image || q.inline != Some(1) { mime } else { "application/octet-stream".into() }),
            (header::CONTENT_DISPOSITION, disposition),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
            (header::CONTENT_SECURITY_POLICY, "sandbox".into()),
            (header::CACHE_CONTROL, "private, max-age=3600".into()),
        ],
        Body::from(body),
    )
        .into_response())
}

#[derive(Deserialize)]
struct FlagsReq {
    name: String,
    uids: Vec<u32>,
    seen: Option<bool>,
    flagged: Option<bool>,
}

async fn set_flags(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<FlagsReq>) -> ApiResult<StatusCode> {
    let a = own_account(&state, &user, id).await?;
    let set = uid_set(&p.uids)?;
    let mut s = open(&a).await?;
    s.select(&p.name).await.map_err(imap_err)?;
    for (flag, on) in [("\\Seen", p.seen), ("\\Flagged", p.flagged)] {
        if let Some(on) = on {
            let op = format!("{}FLAGS.SILENT ({flag})", if on { "+" } else { "-" });
            let _: Vec<_> = s.uid_store(&set, op).await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
        }
    }
    release(a.id, s).await;
    if let (true, Some(seen)) = (is_inbox(&p.name), p.seen) {
        let uids: Vec<i64> = p.uids.iter().map(|u| *u as i64).collect();
        sqlx::query("UPDATE mail_messages SET seen = $3 WHERE account_id = $1 AND uid = ANY($2)").bind(id).bind(&uids).bind(seen).execute(&state.pool).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Déplace des messages (MOVE, sinon COPY + suppression).
/// Par lots de 50 : chaque message déplacé produit une réponse EXPUNGE non sollicitée.
async fn move_uids(s: &mut Imap, uids: &[u32], to: &str) -> ApiResult<()> {
    for chunk in uids.chunks(50) {
        move_chunk(s, &uid_set(chunk)?, to).await?;
        drain(s);
    }
    Ok(())
}

async fn move_chunk(s: &mut Imap, set: &str, to: &str) -> ApiResult<()> {
    if s.uid_mv(set, to).await.is_ok() {
        return Ok(());
    }
    s.uid_copy(set, to).await.map_err(|_| ApiError::bad_request("mail_move_failed"))?;
    let _: Vec<_> = s.uid_store(set, "+FLAGS.SILENT (\\Deleted)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    // UID EXPUNGE (UIDPLUS) ne touche que ces messages ; sinon EXPUNGE classique.
    let expunged = match s.uid_expunge(set).await {
        Ok(stream) => stream.try_collect::<Vec<_>>().await.is_ok(),
        Err(_) => false,
    };
    if !expunged {
        let _: Vec<_> = s.expunge().await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
    }
    Ok(())
}

/// Après un changement dans la boîte de réception : cache à jour (sans notification).
async fn after_inbox_change(state: &AppState, a: &Account, from: &str, to: Option<&str>, uids: &[u32]) -> ApiResult<()> {
    if is_inbox(from) {
        let uids: Vec<i64> = uids.iter().map(|u| *u as i64).collect();
        sqlx::query("DELETE FROM mail_messages WHERE account_id = $1 AND uid = ANY($2)").bind(a.id).bind(&uids).execute(&state.pool).await?;
    }
    if to.is_some_and(is_inbox) {
        let (state, a) = (state.clone(), a.clone());
        tokio::spawn(async move {
            let _ = crate::mailbox::sync_account(&state, &a).await;
        });
    }
    Ok(())
}

#[derive(Deserialize)]
struct MoveReq {
    name: String,
    uids: Vec<u32>,
    to: String,
}

async fn move_messages(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<MoveReq>) -> ApiResult<StatusCode> {
    let a = own_account(&state, &user, id).await?;
    uid_set(&p.uids)?; // validation
    if p.to == p.name {
        return Ok(StatusCode::NO_CONTENT);
    }
    let mut s = open(&a).await?;
    s.select(&p.name).await.map_err(imap_err)?;
    move_uids(&mut s, &p.uids, &p.to).await?;
    release(a.id, s).await;
    after_inbox_change(&state, &a, &p.name, Some(&p.to), &p.uids).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct DeleteReq {
    name: String,
    uids: Vec<u32>,
}

/// Supprime : vers la corbeille, ou définitivement depuis la corbeille (ou sans corbeille
/// créable).
async fn delete_messages(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<DeleteReq>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let set = uid_set(&p.uids)?;
    let mut s = open(&a).await?;
    let mut trash = find_role(&mut s, "trash").await?;
    if trash.is_none() && s.create("Trash").await.is_ok() {
        let _ = s.subscribe("Trash").await;
        trash = Some("Trash".into());
    }
    s.select(&p.name).await.map_err(imap_err)?;
    let permanent = trash.as_deref().is_none_or(|t| t == p.name);
    if permanent {
        let _: Vec<_> = s.uid_store(&set, "+FLAGS.SILENT (\\Deleted)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
        let ok = match s.uid_expunge(&set).await {
            Ok(stream) => stream.try_collect::<Vec<_>>().await.is_ok(),
            Err(_) => false,
        };
        if !ok {
            let _: Vec<_> = s.expunge().await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
        }
    } else if let Some(t) = &trash {
        move_uids(&mut s, &p.uids, t).await?;
    }
    release(a.id, s).await;
    after_inbox_change(&state, &a, &p.name, None, &p.uids).await?;
    Ok(Json(serde_json::json!({ "permanent": permanent, "trash": trash })))
}

// ---------------------------------------------------------------------------
// Destinataires suggérés
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ContactsQuery {
    q: Option<String>,
}

/// Adresses connues : expéditeurs de la boîte de réception et membres des espaces partagés.
async fn contacts(State(state): State<AppState>, user: AuthUser, Query(q): Query<ContactsQuery>) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let term = format!("%{}%", q.q.as_deref().unwrap_or("").trim().to_lowercase().replace(['%', '_'], ""));
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, address FROM (
            SELECT DISTINCT ON (lower(u.email)) u.name AS name, u.email AS address, 0 AS src
              FROM users u
             WHERE u.email IS NOT NULL AND u.id <> $1 AND EXISTS (
                SELECT 1 FROM workspace_members a JOIN workspace_members b ON a.workspace_id = b.workspace_id
                 WHERE a.user_id = $1::text AND b.user_id = u.id::text)
            UNION ALL
            SELECT DISTINCT ON (lower(m.from_addr)) m.from_name, m.from_addr, 1
              FROM mail_messages m JOIN mail_accounts ac ON ac.id = m.account_id
             WHERE ac.user_id = $1 AND m.from_addr <> ''
         ) c
         WHERE lower(name) LIKE $2 OR lower(address) LIKE $2
         ORDER BY src, name LIMIT 12",
    )
    .bind(user.id)
    .bind(term)
    .fetch_all(&state.pool)
    .await?;
    let mut seen = std::collections::HashSet::new();
    Ok(Json(
        rows.into_iter()
            .filter(|(_, addr)| seen.insert(addr.to_lowercase()))
            .map(|(name, address)| serde_json::json!({ "name": name, "address": address }))
            .collect(),
    ))
}

// ---------------------------------------------------------------------------
// Envoi (SMTP)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SmtpReq {
    pub host: String,
    pub port: Option<u16>,
    pub security: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

pub struct SmtpValid {
    pub host: String,
    pub port: u16,
    pub security: String,
    /// `None` : même identifiant que l'IMAP.
    pub username: Option<String>,
    /// `None` : même mot de passe que l'IMAP.
    pub password_enc: Option<String>,
}

impl SmtpReq {
    /// Vérifie les paramètres d'envoi (connexion et authentification) avant de les garder.
    pub async fn validate(&self, imap_user: &str, imap_pass: &str, keep_pass: Option<&str>) -> ApiResult<SmtpValid> {
        let host = self.host.trim().to_lowercase();
        if host.is_empty() || host.len() > 255 || host.contains(['/', ' ', ':']) {
            return Err(ApiError::bad_request("mail_invalid_host"));
        }
        let security = self.security.as_deref().unwrap_or("starttls");
        if !matches!(security, "tls" | "starttls" | "none") || (security == "none" && !crate::caldav::allow_private()) {
            return Err(ApiError::bad_request("invalid_value"));
        }
        let port = self.port.unwrap_or(match security {
            "tls" => 465,
            "starttls" => 587,
            _ => 25,
        });
        let username = self.username.as_deref().map(str::trim).filter(|u| !u.is_empty() && *u != imap_user).map(str::to_string);
        let password = self.password.as_deref().filter(|p| !p.is_empty()).or(keep_pass).map(str::to_string);
        let cfg = SmtpCfg {
            host: host.clone(),
            port,
            security: security.to_string(),
            username: username.clone().unwrap_or_else(|| imap_user.to_string()),
            password: password.clone().unwrap_or_else(|| imap_pass.to_string()),
        };
        let t = transport(&cfg).await?;
        match tokio::time::timeout(Duration::from_secs(25), t.test_connection()).await {
            Ok(Ok(true)) => {}
            Ok(Ok(false)) | Err(_) => return Err(ApiError::bad_request("mail_smtp_unreachable")),
            Ok(Err(e)) => return Err(smtp_err(e)),
        }
        Ok(SmtpValid { host, port, security: security.to_string(), username, password_enc: password.map(|p| crate::crypto::encrypt(&p)) })
    }
}

struct SmtpCfg {
    host: String,
    port: u16,
    security: String,
    username: String,
    password: String,
}

fn smtp_err(e: lettre::transport::smtp::Error) -> ApiError {
    let s = e.to_string();
    if s.contains("535") || s.to_lowercase().contains("authentication") {
        ApiError::bad_request("mail_smtp_auth_failed")
    } else {
        ApiError::BadRequest(format!("mail_smtp_error: {}", s.chars().take(200).collect::<String>()))
    }
}

async fn transport(c: &SmtpCfg) -> ApiResult<AsyncSmtpTransport<Tokio1Executor>> {
    let addrs: Vec<IpAddr> = tokio::net::lookup_host((c.host.as_str(), c.port))
        .await
        .map_err(|_| ApiError::bad_request("mail_smtp_unreachable"))?
        .map(|a| a.ip())
        .collect();
    if addrs.is_empty() {
        return Err(ApiError::bad_request("mail_smtp_unreachable"));
    }
    if !crate::caldav::allow_private() && addrs.iter().any(|ip| crate::caldav::is_private(*ip)) {
        return Err(ApiError::bad_request("mail_private_address"));
    }
    let tls = || TlsParameters::new(c.host.clone()).map_err(|e| ApiError::BadRequest(format!("smtp_tls: {e}")));
    let mut b = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&c.host).port(c.port).timeout(Some(Duration::from_secs(20)));
    b = match c.security.as_str() {
        "tls" => b.tls(Tls::Wrapper(tls()?)),
        "none" => b.tls(Tls::None),
        _ => b.tls(Tls::Required(tls()?)),
    };
    if !c.username.is_empty() {
        b = b.credentials(Credentials::new(c.username.clone(), c.password.clone()));
    }
    Ok(b.build())
}

fn smtp_cfg(a: &Account) -> ApiResult<SmtpCfg> {
    let host = a.smtp_host.clone().ok_or_else(|| ApiError::bad_request("mail_no_smtp"))?;
    let imap_pass = crate::crypto::decrypt(&a.password_enc).ok_or_else(|| ApiError::bad_request("mail_secret_unreadable"))?;
    Ok(SmtpCfg {
        host,
        port: a.smtp_port.unwrap_or(587) as u16,
        security: a.smtp_security.clone().unwrap_or_else(|| "starttls".into()),
        username: a.smtp_username.clone().unwrap_or_else(|| a.username.clone()),
        password: a.smtp_password_enc.as_deref().and_then(crate::crypto::decrypt).unwrap_or(imap_pass),
    })
}

#[derive(Deserialize)]
struct AttachRef {
    url: String,
    name: String,
}

#[derive(Deserialize)]
struct MsgRef {
    folder: String,
    uid: u32,
}

#[derive(Deserialize)]
struct SendReq {
    to: Vec<String>,
    #[serde(default)]
    cc: Vec<String>,
    #[serde(default)]
    bcc: Vec<String>,
    subject: String,
    html: String,
    text: Option<String>,
    in_reply_to: Option<String>,
    #[serde(default)]
    references: Vec<String>,
    #[serde(default)]
    attachments: Vec<AttachRef>,
    /// Message auquel on répond (marqué « répondu »).
    reply: Option<MsgRef>,
    /// Message transféré : ses pièces jointes sont reprises.
    forward: Option<MsgRef>,
}

const MAX_SIZE: usize = 25 * 1024 * 1024;

fn parse_mailboxes(list: &[String]) -> ApiResult<Vec<Mailbox>> {
    list.iter()
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .map(|x| x.parse::<Mailbox>().map_err(|_| ApiError::BadRequest(format!("mail_invalid_address: {}", x.chars().take(100).collect::<String>()))))
        .collect()
}

/// Lit un fichier envoyé sur Looty (`/uploads/<nom>`).
async fn read_upload(url: &str) -> ApiResult<(Vec<u8>, String)> {
    let name = url.rsplit_once("/uploads/").map(|(_, n)| n).unwrap_or("");
    if !crate::upload::is_safe_name(name) {
        return Err(ApiError::bad_request("mail_invalid_attachment"));
    }
    let path = std::path::PathBuf::from(crate::upload::UPLOAD_DIR).join(name);
    let data = tokio::fs::read(&path).await.map_err(|_| ApiError::bad_request("mail_invalid_attachment"))?;
    Ok((data, mime_guess::from_path(&path).first_or_octet_stream().essence_str().to_string()))
}

/// Images de l'éditeur (`/uploads/...`) intégrées au message (`cid:`), pour qu'elles
/// s'affichent chez le destinataire sans accès à Looty.
fn inline_images(html: &str, base_url: &str) -> (String, Vec<String>) {
    let mut names = Vec::new();
    let mut out = html.to_string();
    for prefix in [format!("src=\"{base_url}/uploads/"), "src=\"/uploads/".to_string()] {
        let mut result = String::new();
        let mut rest = out.as_str();
        while let Some(i) = rest.find(&prefix) {
            result.push_str(&rest[..i]);
            let after = &rest[i + prefix.len()..];
            let end = after.find('"').unwrap_or(after.len());
            let name = &after[..end];
            if crate::upload::is_safe_name(name) {
                if !names.iter().any(|n| n == name) {
                    names.push(name.to_string());
                }
                result.push_str(&format!("src=\"cid:{name}"));
            } else {
                result.push_str(&rest[i..i + prefix.len() + end]);
            }
            rest = &after[end..];
        }
        result.push_str(rest);
        out = result;
    }
    (out, names)
}

fn content_type(mime: &str) -> ContentType {
    ContentType::parse(mime).unwrap_or_else(|_| ContentType::parse("application/octet-stream").unwrap())
}

async fn send(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(p): Json<SendReq>) -> ApiResult<Json<serde_json::Value>> {
    let a = own_account(&state, &user, id).await?;
    let cfg = smtp_cfg(&a)?;
    let (to, cc, bcc) = (parse_mailboxes(&p.to)?, parse_mailboxes(&p.cc)?, parse_mailboxes(&p.bcc)?);
    if to.len() + cc.len() + bcc.len() == 0 {
        return Err(ApiError::bad_request("mail_no_recipient"));
    }
    if to.len() + cc.len() + bcc.len() > 100 {
        return Err(ApiError::bad_request("mail_too_many_recipients"));
    }
    let from_addr = a.email.parse().map_err(|_| ApiError::bad_request("mail_invalid_from"))?;
    let from = Mailbox::new(a.display_name.clone().filter(|n| !n.is_empty()), from_addr);
    let domain = a.email.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_else(|| "looty.local".into());

    // Corps : texte + HTML (avec images intégrées).
    let cleaned = clean_html(&p.html);
    let (html, images) = inline_images(&cleaned, &state.base_url);
    let doc = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"></head><body style=\"font-family:-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif;font-size:14px;line-height:1.5\">{html}</body></html>"
    );
    let text = p.text.clone().unwrap_or_default();
    let mut size = doc.len() + text.len();
    let html_part = if images.is_empty() {
        MultiPart::alternative().singlepart(SinglePart::plain(text.clone())).singlepart(SinglePart::html(doc))
    } else {
        let mut related = MultiPart::related().singlepart(SinglePart::html(doc));
        for name in &images {
            let (data, mime) = read_upload(&format!("/uploads/{name}")).await?;
            size += data.len();
            related = related.singlepart(Attachment::new_inline(name.clone()).body(data, content_type(&mime)));
        }
        MultiPart::alternative().singlepart(SinglePart::plain(text.clone())).multipart(related)
    };

    // Pièces jointes : fichiers envoyés sur Looty, puis celles du message transféré.
    let mut files: Vec<(String, Vec<u8>, String)> = Vec::new();
    for att in &p.attachments {
        let (data, mime) = read_upload(&att.url).await?;
        let name: String = att.name.chars().filter(|c| !c.is_control()).take(200).collect();
        files.push((if name.is_empty() { "attachment".into() } else { name }, data, mime));
    }
    let mut session: Option<Imap> = None;
    if let Some(f) = &p.forward {
        let mut s = open(&a).await?;
        let raw = fetch_raw(&mut s, &f.folder, f.uid, true).await?;
        session = Some(s);
        if let Some(orig) = MessageParser::default().parse(&raw) {
            for part in orig.attachments() {
                let mime = part.content_type().map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("octet-stream"))).unwrap_or_else(|| "application/octet-stream".into());
                files.push((part.attachment_name().unwrap_or("attachment").to_string(), part.contents().to_vec(), mime));
            }
        }
    }
    size += files.iter().map(|(_, d, _)| d.len()).sum::<usize>();
    if size > MAX_SIZE {
        return Err(ApiError::bad_request("mail_too_large"));
    }
    let body = if files.is_empty() {
        html_part
    } else {
        let mut mixed = MultiPart::mixed().multipart(html_part);
        for (name, data, mime) in files {
            mixed = mixed.singlepart(Attachment::new(name).body(data, content_type(&mime)));
        }
        mixed
    };

    let mut b = Message::builder()
        .from(from)
        .subject(p.subject.chars().filter(|c| !c.is_control()).take(500).collect::<String>())
        .message_id(Some(format!("<{}@{domain}>", Uuid::new_v4())));
    for m in to {
        b = b.to(m);
    }
    for m in cc {
        b = b.cc(m);
    }
    for m in bcc {
        b = b.bcc(m);
    }
    let clean_id = |s: &str| s.chars().filter(|c| !c.is_control() && !c.is_whitespace()).take(300).collect::<String>();
    if let Some(irt) = p.in_reply_to.as_deref().map(clean_id).filter(|x| !x.is_empty()) {
        b = b.in_reply_to(irt);
    }
    let refs: Vec<String> = p.references.iter().map(|r| clean_id(r)).filter(|r| !r.is_empty()).take(30).collect();
    if !refs.is_empty() {
        b = b.references(refs.join(" "));
    }
    let message = b.multipart(body).map_err(|e| ApiError::BadRequest(format!("mail_invalid_message: {e}")))?;
    let raw = message.formatted();

    let t = transport(&cfg).await?;
    match tokio::time::timeout(Duration::from_secs(60), t.send(message)).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => return Err(smtp_err(e)),
        Err(_) => return Err(ApiError::bad_request("mail_timeout")),
    }

    // Copie dans « Envoyés » (Gmail et Outlook la font eux-mêmes) et réponse marquée.
    let host = cfg.host.as_str();
    let auto_sent = ["gmail.com", "googlemail.com", "office365.com", "outlook.com"].iter().any(|d| host.ends_with(d));
    let mut sent_folder = None;
    let imap_work = async {
        let mut s = match session.take() {
            Some(s) => s,
            None => open(&a).await?,
        };
        if !auto_sent {
            let mut sent = find_role(&mut s, "sent").await?;
            if sent.is_none() && s.create("Sent").await.is_ok() {
                let _ = s.subscribe("Sent").await;
                sent = Some("Sent".into());
            }
            if let Some(folder) = &sent {
                s.append(folder, Some("(\\Seen)"), None, &raw).await.map_err(imap_err)?;
            }
            sent_folder = sent;
        }
        if let Some(r) = &p.reply {
            s.select(&r.folder).await.map_err(imap_err)?;
            let _: Vec<_> = s.uid_store(r.uid.to_string(), "+FLAGS.SILENT (\\Answered)").await.map_err(imap_err)?.try_collect().await.map_err(imap_err)?;
        }
        release(a.id, s).await;
        ApiResult::Ok(())
    };
    // Le message est parti : un échec ici n'est signalé qu'en avertissement.
    let warning = imap_work.await.err().map(|e| format!("{e:?}"));
    Ok(Json(serde_json::json!({ "sent": true, "sent_folder": sent_folder, "warning": warning })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutf7_roundtrip() {
        assert_eq!(decode_mutf7("&AMk-l&AOk-ments envoy&AOk-s"), "Éléments envoyés");
        assert_eq!(decode_mutf7("Tom &- Jerry"), "Tom & Jerry");
        assert_eq!(encode_mutf7("Éléments envoyés"), "&AMk-l&AOk-ments envoy&AOk-s");
        assert_eq!(encode_mutf7("Tom & Jerry"), "Tom &- Jerry");
        for s in ["日本語", "Cours 📚 2026", "INBOX"] {
            assert_eq!(decode_mutf7(&encode_mutf7(s)), s);
        }
    }

    #[test]
    fn inline_upload_images() {
        let (h, names) = inline_images("<p><img src=\"/uploads/abc.png\"><img src=\"https://x.fr/uploads/def.jpg\"><img src=\"/uploads/../x\"></p>", "https://x.fr");
        assert_eq!(names, vec!["def.jpg", "abc.png"]);
        assert!(h.contains("cid:abc.png") && h.contains("cid:def.jpg") && h.contains("/uploads/../x"));
    }
}
