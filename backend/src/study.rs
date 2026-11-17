//! Révisions (GLOSSARY : Jeu de révision) : QCM, questionnaires, fiches de révision et
//! synthèses. Deux modes : ÉDITION (propriétaire et éditeurs : titre, type, éléments) et
//! UTILISATION (toute personne qui peut voir le jeu le passe). Les tentatives et l'état des
//! fiches sont strictement par personne : aucune route ne les montre à une autre personne,
//! pas même au propriétaire du jeu.
//!
//! Format des éléments : voir `docs/study-items.md` (schéma strict, destiné aussi à la
//! génération par une IA). Les types `ItemIn` / `Choice` ci-dessous en sont la définition
//! de référence, `validate_item` en applique les règles.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::{level, member_perms, AuthUser, Perms, Role};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub const KINDS: [&str; 4] = ["qcm", "questionnaire", "flashcards", "summary"];
pub const MAX_ITEMS: usize = 500;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sets", get(list_sets).post(create_set))
        .route("/sets/{id}", get(get_set).put(update_set).delete(delete_set))
        .route("/sets/{id}/revisions", get(list_revisions))
        .route("/sets/{id}/visibility", put(set_visibility))
        .route("/sets/{id}/attempts", get(list_attempts).post(submit_attempt))
        .route("/sets/{id}/cards", get(list_cards))
        .route("/items/{id}/review", post(review_card))
}

// ---------------------------------------------------------------------------
// Format des éléments
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    /// Identifiant court et stable dans l'élément (« a », « b »...).
    pub id: String,
    pub text: String,
}

/// Élément tel qu'il est écrit (par l'éditeur ou par une IA).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ItemIn {
    /// Présent quand on modifie un élément existant ; ignoré pour un nouveau.
    #[serde(default)]
    pub id: Option<Uuid>,
    #[serde(rename = "type")]
    pub kind: String,
    pub prompt: String,
    #[serde(default)]
    pub choices: Vec<Choice>,
    pub answer: Value,
    #[serde(default)]
    pub explanation: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnsSingle {
    choice: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnsMulti {
    choices: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnsText {
    #[serde(default)]
    accepted: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnsCard {
    back: String,
}

/// Types d'éléments admis pour chaque type de jeu.
pub fn allowed_types(kind: &str) -> &'static [&'static str] {
    match kind {
        "qcm" => &["single", "multi"],
        "questionnaire" => &["single", "multi", "text"],
        "flashcards" | "summary" => &["flashcard"],
        _ => &[],
    }
}

fn bad(code: &str) -> ApiError {
    ApiError::bad_request(code)
}

/// Vérifie un élément pour un jeu de type `set_kind`.
pub fn validate_item(set_kind: &str, it: &ItemIn) -> ApiResult<()> {
    if !allowed_types(set_kind).contains(&it.kind.as_str()) {
        return Err(bad("study_item_type"));
    }
    let prompt = it.prompt.trim();
    if prompt.is_empty() || prompt.chars().count() > 2000 || it.explanation.chars().count() > 4000 {
        return Err(bad("study_item_prompt"));
    }
    match it.kind.as_str() {
        "single" | "multi" => {
            if !(2..=10).contains(&it.choices.len()) {
                return Err(bad("study_item_choices"));
            }
            let mut ids = HashSet::new();
            for c in &it.choices {
                if c.id.is_empty() || c.id.chars().count() > 16 || c.text.trim().is_empty() || c.text.chars().count() > 500 || !ids.insert(c.id.as_str()) {
                    return Err(bad("study_item_choices"));
                }
            }
            if it.kind == "single" {
                let a: AnsSingle = serde_json::from_value(it.answer.clone()).map_err(|_| bad("study_item_answer"))?;
                if !ids.contains(a.choice.as_str()) {
                    return Err(bad("study_item_answer"));
                }
            } else {
                let a: AnsMulti = serde_json::from_value(it.answer.clone()).map_err(|_| bad("study_item_answer"))?;
                let uniq: HashSet<&str> = a.choices.iter().map(|s| s.as_str()).collect();
                if uniq.is_empty() || uniq.len() != a.choices.len() || !uniq.iter().all(|c| ids.contains(c)) {
                    return Err(bad("study_item_answer"));
                }
            }
        }
        "text" => {
            if !it.choices.is_empty() {
                return Err(bad("study_item_choices"));
            }
            let a: AnsText = serde_json::from_value(it.answer.clone()).map_err(|_| bad("study_item_answer"))?;
            if a.accepted.len() > 20 || a.accepted.iter().any(|s| s.trim().is_empty() || s.chars().count() > 200) {
                return Err(bad("study_item_answer"));
            }
        }
        _ => {
            if !it.choices.is_empty() {
                return Err(bad("study_item_choices"));
            }
            let a: AnsCard = serde_json::from_value(it.answer.clone()).map_err(|_| bad("study_item_answer"))?;
            if a.back.trim().is_empty() || a.back.chars().count() > 4000 {
                return Err(bad("study_item_answer"));
            }
        }
    }
    Ok(())
}

pub fn validate_title(title: &str) -> ApiResult<String> {
    let t = title.trim();
    if t.is_empty() || t.chars().count() > 200 {
        return Err(bad("study_invalid_title"));
    }
    Ok(t.to_string())
}

pub fn validate_items(kind: &str, items: &[ItemIn]) -> ApiResult<()> {
    if items.len() > MAX_ITEMS {
        return Err(bad("study_too_many_items"));
    }
    items.iter().try_for_each(|i| validate_item(kind, i))
}

// ---------------------------------------------------------------------------
// Notation
// ---------------------------------------------------------------------------

/// Texte comparé sans casse, sans accents, sans ponctuation ni espaces multiples.
pub fn fold_answer(s: &str) -> String {
    let mut out = String::new();
    for ch in s.trim().chars().flat_map(|c| c.to_lowercase()) {
        let c = match ch {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ÿ' => 'y',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Corrige un élément. `None` : non noté (fiche, ou question ouverte sans réponse attendue).
pub fn score_item(kind: &str, answer: &Value, given: Option<&Value>) -> Option<bool> {
    match kind {
        "single" => {
            let a: AnsSingle = serde_json::from_value(answer.clone()).ok()?;
            Some(given.and_then(|g| g.as_str()) == Some(a.choice.as_str()))
        }
        "multi" => {
            let a: AnsMulti = serde_json::from_value(answer.clone()).ok()?;
            let want: HashSet<&str> = a.choices.iter().map(|s| s.as_str()).collect();
            let got: Vec<&str> = given.and_then(|g| g.as_array()).map(|v| v.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
            let got_set: HashSet<&str> = got.iter().copied().collect();
            Some(got.len() == got_set.len() && got_set == want)
        }
        "text" => {
            let a: AnsText = serde_json::from_value(answer.clone()).ok()?;
            if a.accepted.is_empty() {
                return None;
            }
            let g = fold_answer(given.and_then(|g| g.as_str()).unwrap_or(""));
            Some(!g.is_empty() && a.accepted.iter().any(|x| fold_answer(x) == g))
        }
        _ => None,
    }
}

/// Répétition espacée simplifiée : (facilité, intervalle en jours, délai avant l'échéance).
pub fn next_review(ease: f32, interval_days: i32, rating: &str) -> Option<(f32, i32, Duration)> {
    let (e, i) = match rating {
        "again" => ((ease - 0.2).max(1.3), 0),
        "hard" => ((ease - 0.15).max(1.3), ((interval_days as f32 * 1.2).round() as i32).max(1)),
        "good" => (ease, ((interval_days as f32 * ease).round() as i32).max(1)),
        "easy" => ((ease + 0.15).min(3.5), ((interval_days as f32 * ease * 1.3).round() as i32).max(4)),
        _ => return None,
    };
    let delay = if i == 0 { Duration::minutes(10) } else { Duration::days(i as i64) };
    Some((e, i, delay))
}

// ---------------------------------------------------------------------------
// Accès
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetAccess {
    None,
    /// Peut passer le jeu.
    View,
    /// Peut le modifier.
    Edit,
}

/// Qui peut quoi, sans accès à la base (testé). `member` : rôle et droits de la personne dans
/// l'espace du jeu. Un jeu privé n'est vu que de son propriétaire ; « espace » : les membres
/// qui lisent les documents le passent, ceux qui les modifient l'éditent ; « lien » : il
/// suffit du jeton (la personne doit être connectée : ses tentatives lui sont propres).
pub fn resolve_access(owner: Uuid, user: Uuid, visibility: &str, share_token: Option<Uuid>, given: Option<Uuid>, member: Option<(Role, Perms)>) -> SetAccess {
    if owner == user {
        return SetAccess::Edit;
    }
    match visibility {
        "workspace" => match member {
            Some((_, p)) if p.docs >= level::EDIT => SetAccess::Edit,
            Some((_, p)) if p.docs >= level::READ => SetAccess::View,
            _ => SetAccess::None,
        },
        "link" if share_token.is_some() && share_token == given => SetAccess::View,
        _ => SetAccess::None,
    }
}

#[derive(sqlx::FromRow, Serialize, Clone)]
pub struct SetRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub title: String,
    pub kind: String,
    pub visibility: String,
    pub share_token: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct TokenQ {
    token: Option<Uuid>,
}

pub async fn access_for(pool: &PgPool, user: &AuthUser, set: &SetRow, token: Option<Uuid>) -> ApiResult<SetAccess> {
    let member = match (set.workspace_id, set.visibility.as_str()) {
        (Some(ws), "workspace") => member_perms(pool, user.id, ws).await?,
        _ => None,
    };
    Ok(resolve_access(set.owner_id, user.id, &set.visibility, set.share_token, token, member))
}

/// Charge le jeu et exige au moins `need` ; NotFound sinon (n'en révèle pas l'existence).
pub async fn load(pool: &PgPool, user: &AuthUser, id: Uuid, token: Option<Uuid>, need: SetAccess) -> ApiResult<(SetRow, SetAccess)> {
    let set: SetRow = sqlx::query_as("SELECT * FROM study_sets WHERE id = $1").bind(id).fetch_optional(pool).await?.ok_or(ApiError::NotFound)?;
    let acc = access_for(pool, user, &set, token).await?;
    if acc < need || acc == SetAccess::None {
        return Err(ApiError::NotFound);
    }
    Ok((set, acc))
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow, Serialize, Clone)]
pub struct ItemRow {
    pub id: Uuid,
    pub position: i32,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub kind: String,
    pub prompt: String,
    pub choices: Option<Value>,
    pub answer: Value,
    pub explanation: String,
}

pub async fn items_of(pool: &PgPool, set_id: Uuid) -> ApiResult<Vec<ItemRow>> {
    Ok(sqlx::query_as("SELECT id, position, type, prompt, choices, answer, explanation FROM study_items WHERE set_id = $1 ORDER BY position, id")
        .bind(set_id)
        .fetch_all(pool)
        .await?)
}

#[derive(sqlx::FromRow, Serialize)]
struct SetSummary {
    id: Uuid,
    owner_id: Uuid,
    workspace_id: Option<Uuid>,
    title: String,
    kind: String,
    visibility: String,
    updated_at: DateTime<Utc>,
    item_count: i64,
    /// Ma dernière tentative : (points, total) ; absent si jamais passé.
    last_score: Option<i32>,
    last_max: Option<i32>,
}

/// Mes jeux, puis ceux que mon espace partage. Jamais de liste publique.
async fn list_sets(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(json!(list_for(&state.pool, &user).await?)))
}

/// Les jeux que la personne peut ouvrir, avec `can_edit` (aussi utilisé par les outils IA).
pub async fn list_for(pool: &PgPool, user: &AuthUser) -> ApiResult<Vec<Value>> {
    let rows: Vec<SetSummary> = sqlx::query_as(
        "SELECT s.id, s.owner_id, s.workspace_id, s.title, s.kind, s.visibility, s.updated_at,
                (SELECT COUNT(*) FROM study_items i WHERE i.set_id = s.id) AS item_count,
                (SELECT a.score FROM study_attempts a WHERE a.set_id = s.id AND a.user_id = $1 ORDER BY a.started_at DESC LIMIT 1) AS last_score,
                (SELECT a.max_score FROM study_attempts a WHERE a.set_id = s.id AND a.user_id = $1 ORDER BY a.started_at DESC LIMIT 1) AS last_max
         FROM study_sets s
         WHERE s.owner_id = $1
            OR (s.visibility = 'workspace' AND s.workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = $2))
         ORDER BY s.updated_at DESC",
    )
    .bind(user.id)
    .bind(user.id.to_string())
    .fetch_all(pool)
    .await?;
    let mut perms: HashMap<Uuid, Option<(Role, Perms)>> = HashMap::new();
    let mut out = Vec::new();
    for r in rows {
        let access = if r.owner_id == user.id {
            SetAccess::Edit
        } else {
            let ws = r.workspace_id.ok_or(ApiError::NotFound)?;
            if !perms.contains_key(&ws) {
                perms.insert(ws, member_perms(pool, user.id, ws).await?);
            }
            resolve_access(r.owner_id, user.id, &r.visibility, None, None, perms[&ws])
        };
        if access == SetAccess::None {
            continue;
        }
        let mut v = serde_json::to_value(&r).unwrap_or(Value::Null);
        v["can_edit"] = json!(access == SetAccess::Edit);
        out.push(v);
    }
    Ok(out)
}

#[derive(Deserialize)]
struct CreateReq {
    title: String,
    kind: String,
    workspace_id: Option<Uuid>,
    #[serde(default)]
    items: Vec<ItemIn>,
}

async fn create_set(State(state): State<AppState>, user: AuthUser, Json(req): Json<CreateReq>) -> ApiResult<(StatusCode, Json<Value>)> {
    let title = validate_title(&req.title)?;
    if !KINDS.contains(&req.kind.as_str()) {
        return Err(bad("study_invalid_kind"));
    }
    validate_items(&req.kind, &req.items)?;
    if let Some(ws) = req.workspace_id {
        crate::access::require_perm(&state.pool, &user, ws, |p| p.docs >= level::EDIT).await?;
    }
    let mut tx = state.pool.begin().await?;
    let set: SetRow = sqlx::query_as("INSERT INTO study_sets (owner_id, workspace_id, title, kind) VALUES ($1, $2, $3, $4) RETURNING *")
        .bind(user.id)
        .bind(req.workspace_id)
        .bind(&title)
        .bind(&req.kind)
        .fetch_one(&mut *tx)
        .await?;
    write_items(&mut tx, set.id, &req.items).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(json!(set))))
}

/// Remplace la liste d'éléments en gardant les identifiants connus (les fiches mémorisées
/// et les tentatives s'y rattachent) ; les éléments absents de la liste sont supprimés.
pub async fn write_items(tx: &mut sqlx::PgConnection, set_id: Uuid, items: &[ItemIn]) -> ApiResult<()> {
    let existing: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM study_items WHERE set_id = $1").bind(set_id).fetch_all(&mut *tx).await?;
    let existing: HashSet<Uuid> = existing.into_iter().collect();
    let mut keep: Vec<Uuid> = Vec::new();
    for (pos, it) in items.iter().enumerate() {
        let choices = if it.choices.is_empty() { None } else { Some(json!(it.choices)) };
        match it.id.filter(|i| existing.contains(i) && !keep.contains(i)) {
            Some(id) => {
                sqlx::query("UPDATE study_items SET position = $2, type = $3, prompt = $4, choices = $5, answer = $6, explanation = $7 WHERE id = $1")
                    .bind(id)
                    .bind(pos as i32)
                    .bind(&it.kind)
                    .bind(it.prompt.trim())
                    .bind(choices)
                    .bind(&it.answer)
                    .bind(&it.explanation)
                    .execute(&mut *tx)
                    .await?;
                keep.push(id);
            }
            None => {
                let id: Uuid = sqlx::query_scalar("INSERT INTO study_items (set_id, position, type, prompt, choices, answer, explanation) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id")
                    .bind(set_id)
                    .bind(pos as i32)
                    .bind(&it.kind)
                    .bind(it.prompt.trim())
                    .bind(choices)
                    .bind(&it.answer)
                    .bind(&it.explanation)
                    .fetch_one(&mut *tx)
                    .await?;
                keep.push(id);
            }
        }
    }
    sqlx::query("DELETE FROM study_items WHERE set_id = $1 AND NOT (id = ANY($2))").bind(set_id).bind(&keep).execute(&mut *tx).await?;
    Ok(())
}

/// Le jeu et ses éléments. Pour qui ne peut que passer le jeu, les réponses attendues et
/// les explications des questions sont retirées (elles arrivent avec la correction) ; le
/// verso des fiches reste, il fait partie de l'étude.
async fn get_set(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<TokenQ>) -> ApiResult<Json<Value>> {
    let (set, acc) = load(&state.pool, &user, id, q.token, SetAccess::View).await?;
    let mut items = items_of(&state.pool, id).await?;
    if acc != SetAccess::Edit {
        for it in items.iter_mut() {
            if it.kind != "flashcard" {
                it.answer = Value::Null;
                it.explanation = String::new();
            }
        }
    }
    let mut v = serde_json::to_value(&set).unwrap_or(Value::Null);
    if acc != SetAccess::Edit || set.owner_id != user.id {
        // Le jeton ne sort que pour le propriétaire.
        v["share_token"] = Value::Null;
    }
    v["can_edit"] = json!(acc == SetAccess::Edit);
    v["is_owner"] = json!(set.owner_id == user.id);
    v["items"] = json!(items);
    Ok(Json(v))
}

#[derive(Deserialize)]
struct UpdateReq {
    title: String,
    /// Le type ne change que si tous les éléments restent valides pour lui.
    kind: Option<String>,
    items: Vec<ItemIn>,
}

/// Versions gardées avant chaque modification par l'IA ou restauration, la plus récente d'abord. Réservé à qui peut éditer le jeu.
pub async fn revisions_of(pool: &PgPool, user: &AuthUser, id: Uuid) -> ApiResult<Vec<Value>> {
    load(pool, user, id, None, SetAccess::Edit).await?;
    let rows: Vec<(Uuid, DateTime<Utc>, String, String, i32)> =
        sqlx::query_as("SELECT id, created_at, source, title, jsonb_array_length(items)::int FROM study_set_revisions WHERE set_id = $1 ORDER BY created_at DESC, id LIMIT 100").bind(id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(id, at, source, title, n)| json!({ "id": id, "created_at": at, "source": source, "title": title, "item_count": n })).collect())
}

async fn list_revisions(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    Ok(Json(revisions_of(&state.pool, &user, id).await?))
}

async fn update_set(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateReq>) -> ApiResult<Json<Value>> {
    let (set, _) = load(&state.pool, &user, id, None, SetAccess::Edit).await?;
    let title = validate_title(&req.title)?;
    let kind = req.kind.unwrap_or_else(|| set.kind.clone());
    if !KINDS.contains(&kind.as_str()) {
        return Err(bad("study_invalid_kind"));
    }
    validate_items(&kind, &req.items)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE study_sets SET title = $2, kind = $3, updated_at = NOW() WHERE id = $1").bind(id).bind(&title).bind(&kind).execute(&mut *tx).await?;
    write_items(&mut tx, id, &req.items).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_set(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let (set, _) = load(&state.pool, &user, id, None, SetAccess::Edit).await?;
    if set.owner_id != user.id {
        return Err(ApiError::Forbidden);
    }
    sqlx::query("DELETE FROM study_sets WHERE id = $1").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct VisibilityReq {
    visibility: String,
}

/// Change la visibilité (propriétaire seulement). « lien » crée le jeton (conservé tant que
/// le lien reste actif) ; quitter « lien » le révoque.
async fn set_visibility(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(req): Json<VisibilityReq>) -> ApiResult<Json<SetRow>> {
    let (set, _) = load(&state.pool, &user, id, None, SetAccess::Edit).await?;
    if set.owner_id != user.id {
        return Err(ApiError::Forbidden);
    }
    match req.visibility.as_str() {
        "private" | "link" => {}
        "workspace" if set.workspace_id.is_some() => {}
        _ => return Err(bad("study_invalid_visibility")),
    }
    let row: SetRow = sqlx::query_as(
        "UPDATE study_sets SET visibility = $2,
                share_token = CASE WHEN $2 = 'link' THEN COALESCE(share_token, gen_random_uuid()) ELSE NULL END,
                updated_at = NOW()
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&req.visibility)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

// --- Tentatives (privées) ---------------------------------------------------

#[derive(Deserialize)]
struct AttemptReq {
    /// Réponse par identifiant d'élément : texte (single, text) ou liste (multi).
    answers: HashMap<Uuid, Value>,
    started_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow, Serialize)]
struct AttemptRow {
    id: Uuid,
    set_id: Uuid,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
    score: i32,
    max_score: i32,
    answers: Value,
}

/// Corrige et enregistre une tentative ; renvoie la correction (réponses attendues,
/// explications) seulement maintenant.
async fn submit_attempt(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<TokenQ>, Json(req): Json<AttemptReq>) -> ApiResult<(StatusCode, Json<Value>)> {
    let (_, _) = load(&state.pool, &user, id, q.token, SetAccess::View).await?;
    if req.answers.len() > MAX_ITEMS {
        return Err(bad("too_many_answers"));
    }
    let items = items_of(&state.pool, id).await?;
    let (mut score, mut max) = (0, 0);
    let mut stored = serde_json::Map::new();
    let mut correction = Vec::new();
    for it in &items {
        let given = req.answers.get(&it.id);
        let ok = score_item(&it.kind, &it.answer, given);
        if let Some(ok) = ok {
            max += 1;
            if ok {
                score += 1;
            }
        }
        stored.insert(it.id.to_string(), json!({ "given": given, "correct": ok }));
        correction.push(json!({ "item_id": it.id, "correct": ok, "answer": it.answer, "explanation": it.explanation }));
    }
    if max == 0 {
        return Err(bad("study_nothing_to_score"));
    }
    let now = Utc::now();
    let started = req.started_at.filter(|s| *s <= now).unwrap_or(now);
    let attempt: AttemptRow = sqlx::query_as(
        "INSERT INTO study_attempts (set_id, user_id, started_at, finished_at, score, max_score, answers)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, set_id, started_at, finished_at, score, max_score, answers",
    )
    .bind(id)
    .bind(user.id)
    .bind(started)
    .bind(now)
    .bind(score)
    .bind(max)
    .bind(Value::Object(stored))
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "attempt": attempt, "correction": correction }))))
}

/// Mes tentatives sur ce jeu (celles des autres n'existent pas pour moi).
async fn list_attempts(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<TokenQ>) -> ApiResult<Json<Vec<AttemptRow>>> {
    load(&state.pool, &user, id, q.token, SetAccess::View).await?;
    Ok(Json(
        sqlx::query_as("SELECT id, set_id, started_at, finished_at, score, max_score, answers FROM study_attempts WHERE set_id = $1 AND user_id = $2 ORDER BY started_at DESC LIMIT 200")
            .bind(id)
            .bind(user.id)
            .fetch_all(&state.pool)
            .await?,
    ))
}

// --- Fiches (état par personne) ---------------------------------------------

#[derive(sqlx::FromRow, Serialize)]
struct CardState {
    item_id: Uuid,
    ease: f32,
    interval_days: i32,
    due_at: DateTime<Utc>,
}

async fn list_cards(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<TokenQ>) -> ApiResult<Json<Vec<CardState>>> {
    load(&state.pool, &user, id, q.token, SetAccess::View).await?;
    Ok(Json(
        sqlx::query_as(
            "SELECT c.item_id, c.ease, c.interval_days, c.due_at FROM study_card_state c
             JOIN study_items i ON i.id = c.item_id WHERE i.set_id = $1 AND c.user_id = $2",
        )
        .bind(id)
        .bind(user.id)
        .fetch_all(&state.pool)
        .await?,
    ))
}

#[derive(Deserialize)]
struct ReviewReq {
    /// again | hard | good | easy
    rating: String,
}

async fn review_card(State(state): State<AppState>, user: AuthUser, Path(item_id): Path<Uuid>, Query(q): Query<TokenQ>, Json(req): Json<ReviewReq>) -> ApiResult<Json<CardState>> {
    let set_id: Uuid = sqlx::query_scalar("SELECT set_id FROM study_items WHERE id = $1 AND type = 'flashcard'").bind(item_id).fetch_optional(&state.pool).await?.ok_or(ApiError::NotFound)?;
    load(&state.pool, &user, set_id, q.token, SetAccess::View).await?;
    let cur: Option<(f32, i32)> = sqlx::query_as("SELECT ease, interval_days FROM study_card_state WHERE user_id = $1 AND item_id = $2").bind(user.id).bind(item_id).fetch_optional(&state.pool).await?;
    let (ease, interval) = cur.unwrap_or((2.5, 0));
    let (ease, interval, delay) = next_review(ease, interval, &req.rating).ok_or_else(|| bad("study_invalid_rating"))?;
    Ok(Json(
        sqlx::query_as(
            "INSERT INTO study_card_state (user_id, item_id, ease, interval_days, due_at) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id, item_id) DO UPDATE SET ease = $3, interval_days = $4, due_at = $5
             RETURNING item_id, ease, interval_days, due_at",
        )
        .bind(user.id)
        .bind(item_id)
        .bind(ease)
        .bind(interval)
        .bind(Utc::now() + delay)
        .fetch_one(&state.pool)
        .await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }
    fn member(role: Role) -> Option<(Role, Perms)> {
        Some((role, Perms::preset(role)))
    }

    #[test]
    fn private_set_is_readable_by_its_owner_only() {
        let (owner, other, tok) = (u(1), u(2), u(9));
        assert_eq!(resolve_access(owner, owner, "private", None, None, None), SetAccess::Edit);
        assert_eq!(resolve_access(owner, other, "private", None, None, member(Role::Owner)), SetAccess::None);
        // Un jeton même exact ne sert à rien tant que le jeu n'est pas en mode lien.
        assert_eq!(resolve_access(owner, other, "private", Some(tok), Some(tok), None), SetAccess::None);
    }

    #[test]
    fn link_token_lets_view_but_not_edit() {
        let (owner, other, tok) = (u(1), u(2), u(9));
        assert_eq!(resolve_access(owner, other, "link", Some(tok), Some(tok), None), SetAccess::View);
        assert_eq!(resolve_access(owner, other, "link", Some(tok), Some(u(8)), None), SetAccess::None);
        assert_eq!(resolve_access(owner, other, "link", Some(tok), None, None), SetAccess::None);
    }

    #[test]
    fn workspace_visibility_follows_document_permissions() {
        let (owner, other) = (u(1), u(2));
        assert_eq!(resolve_access(owner, other, "workspace", None, None, member(Role::Member)), SetAccess::Edit);
        assert_eq!(resolve_access(owner, other, "workspace", None, None, member(Role::Viewer)), SetAccess::View);
        assert_eq!(resolve_access(owner, other, "workspace", None, None, member(Role::Guest)), SetAccess::None);
        assert_eq!(resolve_access(owner, other, "workspace", None, None, None), SetAccess::None);
    }

    fn item(kind: &str, choices: &[&str], answer: Value) -> ItemIn {
        ItemIn {
            id: None,
            kind: kind.into(),
            prompt: "Question ?".into(),
            choices: choices.iter().map(|c| Choice { id: c.to_string(), text: format!("Choix {c}") }).collect(),
            answer,
            explanation: String::new(),
        }
    }

    #[test]
    fn scoring_single_multi_text() {
        let single = json!({ "choice": "b" });
        assert_eq!(score_item("single", &single, Some(&json!("b"))), Some(true));
        assert_eq!(score_item("single", &single, Some(&json!("a"))), Some(false));
        assert_eq!(score_item("single", &single, None), Some(false));

        let multi = json!({ "choices": ["a", "c"] });
        assert_eq!(score_item("multi", &multi, Some(&json!(["c", "a"]))), Some(true));
        assert_eq!(score_item("multi", &multi, Some(&json!(["a"]))), Some(false));
        assert_eq!(score_item("multi", &multi, Some(&json!(["a", "b", "c"]))), Some(false));
        assert_eq!(score_item("multi", &multi, Some(&json!(["a", "a"]))), Some(false));

        let text = json!({ "accepted": ["Éléphant d'Asie"] });
        assert_eq!(score_item("text", &text, Some(&json!("  elephant  d'asie! "))), Some(true));
        assert_eq!(score_item("text", &text, Some(&json!("lion"))), Some(false));
        assert_eq!(score_item("text", &text, Some(&json!(""))), Some(false));
        assert_eq!(score_item("text", &json!({ "accepted": [] }), Some(&json!("x"))), None);
        assert_eq!(score_item("flashcard", &json!({ "back": "x" }), None), None);
    }

    #[test]
    fn item_schema_is_strict() {
        let ok = item("single", &["a", "b"], json!({ "choice": "a" }));
        assert!(validate_item("qcm", &ok).is_ok());
        // Type d'élément interdit pour ce type de jeu.
        assert!(validate_item("flashcards", &ok).is_err());
        // Réponse hors choix, doublons, champ inconnu.
        assert!(validate_item("qcm", &item("single", &["a", "b"], json!({ "choice": "z" }))).is_err());
        assert!(validate_item("qcm", &item("single", &["a", "a"], json!({ "choice": "a" }))).is_err());
        assert!(validate_item("qcm", &item("single", &["a", "b"], json!({ "choice": "a", "x": 1 }))).is_err());
        assert!(validate_item("qcm", &item("multi", &["a", "b"], json!({ "choices": [] }))).is_err());
        assert!(validate_item("qcm", &item("multi", &["a", "b", "c"], json!({ "choices": ["a", "c"] }))).is_ok());
        assert!(validate_item("flashcards", &item("flashcard", &[], json!({ "back": "Verso" }))).is_ok());
        assert!(validate_item("flashcards", &item("flashcard", &[], json!({ "back": " " }))).is_err());
        assert!(serde_json::from_value::<ItemIn>(json!({ "type": "text", "prompt": "?", "answer": {}, "bogus": 1 })).is_err());
    }

    #[test]
    fn spaced_repetition_moves_the_due_date() {
        let (e, i, _) = next_review(2.5, 0, "good").unwrap();
        assert_eq!((e, i), (2.5, 1));
        let (_, i2, _) = next_review(e, i, "good").unwrap();
        assert!(i2 >= 2);
        let (e3, i3, d) = next_review(2.5, 6, "again").unwrap();
        assert!(e3 < 2.5 && i3 == 0 && d == Duration::minutes(10));
        assert!(next_review(2.5, 1, "nope").is_none());
    }
}
