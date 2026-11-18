//! Entraînements (GLOSSARY, ADR 0011) : QCM, questionnaires, fiches de révision et
//! synthèses, éléments de l'espace comme les pages (arbre, déplacement, corbeille, personnel).
//! Deux modes : ÉDITION (droit d'édition des documents : titre, type, éléments, déplacement,
//! suppression) et UTILISATION (droit de lecture : s'entraîner). Les tentatives et l'état des
//! fiches sont strictement par personne : aucune route ne les montre à une autre personne,
//! pas même à l'autrice de l'entraînement.
//!
//! Format des éléments : voir `docs/study-items.md` (schéma strict, destiné aussi à la
//! génération par une IA). Les types `ItemIn` / `Choice` ci-dessous en sont la définition
//! de référence, `validate_item` en applique les règles.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
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
        .route("/sets/{id}/move", post(move_set))
        .route("/sets/{id}/duplicate", post(duplicate_set))
        .route("/sets/{id}/links", get(list_links).post(create_link))
        .route("/sets/{id}/links/{link_id}", axum::routing::delete(delete_link))
        .route("/linked", get(linked_sets))
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
/// Les textes (énoncé, choix, explication, verso) sont du Markdown : seule la longueur est contrôlée.
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
// Accès (ADR 0011) : un entraînement suit les droits des documents de l'espace
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetAccess {
    None,
    /// Peut s'entraîner (lecture des documents).
    View,
    /// Peut le modifier, le déplacer et le supprimer (édition des documents).
    Edit,
}

/// Qui peut quoi, sans accès à la base (testé). `member` : rôle et droits de la personne dans
/// l'espace de l'entraînement. Un entraînement personnel (`private_to`) n'est vu que de son
/// propriétaire, administrateurs exclus ; sinon le niveau « documents » du rôle décide : lecture
/// pour s'entraîner, édition pour modifier.
pub fn resolve_access(private_to: Option<Uuid>, user: Uuid, member: Option<(Role, Perms)>) -> SetAccess {
    let Some((_, p)) = member else { return SetAccess::None };
    if let Some(owner) = private_to {
        return if owner == user && p.docs >= level::READ { SetAccess::Edit } else { SetAccess::None };
    }
    if p.docs >= level::EDIT {
        SetAccess::Edit
    } else if p.docs >= level::READ {
        SetAccess::View
    } else {
        SetAccess::None
    }
}

#[derive(sqlx::FromRow, Serialize, Clone)]
pub struct SetRow {
    pub id: Uuid,
    /// Autrice de l'entraînement (elle seule peut le rendre personnel).
    pub owner_id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub private_to: Option<Uuid>,
    pub position: f64,
    pub title: String,
    pub kind: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn access_for(pool: &PgPool, user: &AuthUser, set: &SetRow) -> ApiResult<SetAccess> {
    let member = member_perms(pool, user.id, set.workspace_id).await?;
    Ok(resolve_access(set.private_to, user.id, member))
}

/// Charge l'entraînement (hors corbeille) et exige au moins `need` ; NotFound sinon
/// (n'en révèle pas l'existence).
pub async fn load(pool: &PgPool, user: &AuthUser, id: Uuid, need: SetAccess) -> ApiResult<(SetRow, SetAccess)> {
    let set: SetRow = sqlx::query_as("SELECT * FROM study_sets WHERE id = $1 AND deleted_at IS NULL").bind(id).fetch_optional(pool).await?.ok_or(ApiError::NotFound)?;
    let acc = access_for(pool, user, &set).await?;
    if acc < need || acc == SetAccess::None {
        return Err(ApiError::NotFound);
    }
    Ok((set, acc))
}

/// Comme `load`, mais Forbidden (et non NotFound) quand la personne voit l'entraînement sans
/// pouvoir le modifier.
async fn load_edit(pool: &PgPool, user: &AuthUser, id: Uuid) -> ApiResult<SetRow> {
    let (set, acc) = load(pool, user, id, SetAccess::View).await?;
    if acc != SetAccess::Edit {
        return Err(ApiError::Forbidden);
    }
    Ok(set)
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
    workspace_id: Uuid,
    parent_id: Option<Uuid>,
    private_to: Option<Uuid>,
    position: f64,
    title: String,
    kind: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    item_count: i64,
    /// Ma dernière tentative : (points, total) ; absent si jamais passé.
    last_score: Option<i32>,
    last_max: Option<i32>,
}

#[derive(Deserialize)]
struct ListQ {
    workspace_id: Option<Uuid>,
}

/// Les entraînements des espaces dont je suis membre (ceux d'un espace : `?workspace_id=`).
async fn list_sets(State(state): State<AppState>, user: AuthUser, Query(q): Query<ListQ>) -> ApiResult<Json<Value>> {
    Ok(Json(json!(list_for(&state.pool, &user, q.workspace_id).await?)))
}

/// Les entraînements que la personne peut ouvrir, avec `can_edit` (aussi utilisé par les outils IA).
pub async fn list_for(pool: &PgPool, user: &AuthUser, workspace: Option<Uuid>) -> ApiResult<Vec<Value>> {
    list_filtered(pool, user, workspace, None).await
}

async fn list_filtered(pool: &PgPool, user: &AuthUser, workspace: Option<Uuid>, only: Option<&[Uuid]>) -> ApiResult<Vec<Value>> {
    let rows: Vec<SetSummary> = sqlx::query_as(
        "SELECT s.id, s.owner_id, s.workspace_id, s.parent_id, s.private_to, s.position, s.title, s.kind, s.created_at, s.updated_at,
                (SELECT COUNT(*) FROM study_items i WHERE i.set_id = s.id) AS item_count,
                (SELECT a.score FROM study_attempts a WHERE a.set_id = s.id AND a.user_id = $1 ORDER BY a.started_at DESC LIMIT 1) AS last_score,
                (SELECT a.max_score FROM study_attempts a WHERE a.set_id = s.id AND a.user_id = $1 ORDER BY a.started_at DESC LIMIT 1) AS last_max
         FROM study_sets s
         WHERE s.deleted_at IS NULL
           AND s.workspace_id IN (SELECT workspace_id FROM workspace_members WHERE user_id = $2)
           AND (s.private_to IS NULL OR s.private_to = $1)
           AND ($3::uuid IS NULL OR s.workspace_id = $3)
           AND ($4::uuid[] IS NULL OR s.id = ANY($4))
         ORDER BY s.updated_at DESC",
    )
    .bind(user.id)
    .bind(user.id.to_string())
    .bind(workspace)
    .bind(only)
    .fetch_all(pool)
    .await?;
    let mut perms: HashMap<Uuid, Option<(Role, Perms)>> = HashMap::new();
    let mut out = Vec::new();
    for r in rows {
        if !perms.contains_key(&r.workspace_id) {
            perms.insert(r.workspace_id, member_perms(pool, user.id, r.workspace_id).await?);
        }
        let access = resolve_access(r.private_to, user.id, perms[&r.workspace_id]);
        if access == SetAccess::None {
            continue;
        }
        let mut v = serde_json::to_value(&r).unwrap_or(Value::Null);
        v["can_edit"] = json!(access == SetAccess::Edit);
        out.push(v);
    }
    Ok(out)
}

/// Distingue un champ absent d'un champ explicitement `null`.
fn double_option<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(de).map(Some)
}

#[derive(Deserialize)]
struct CreateReq {
    title: String,
    kind: String,
    workspace_id: Uuid,
    /// Page sous laquelle ranger l'entraînement (sinon à la racine de l'espace).
    parent_id: Option<Uuid>,
    /// Personnel : visible de son autrice seulement (ADR 0002).
    #[serde(default)]
    personal: bool,
    #[serde(default)]
    items: Vec<ItemIn>,
}

/// Emplacement d'un nouvel élément : `Some(propriétaire)` si l'entraînement doit être personnel.
/// Rôle qui crée des documents (ou qui les lit, pour un entraînement personnel, même sous une
/// page partagée qu'il ne peut que lire) ; sous une page personnelle, il l'est aussi.
async fn creation_place(pool: &PgPool, user: &AuthUser, ws: Uuid, parent: Option<Uuid>, personal: bool) -> ApiResult<Option<Uuid>> {
    let (_, perms) = member_perms(pool, user.id, ws).await?.ok_or(ApiError::NotFound)?;
    let parent_owner: Option<Uuid> = match parent {
        Some(p) => sqlx::query_scalar("SELECT private_to FROM pages WHERE id = $1").bind(p).fetch_optional(pool).await?.flatten(),
        None => None,
    };
    let personal = personal || parent_owner.is_some();
    if personal {
        if perms.docs < level::READ {
            return Err(ApiError::Forbidden);
        }
    } else if perms.docs < level::EDIT {
        return Err(ApiError::Forbidden);
    }
    if let Some(p) = parent {
        let (pws, _, access) = crate::access::page_access(pool, user, p).await?;
        if pws != ws || access == crate::access::PageAccess::None || (!personal && access != crate::access::PageAccess::Write) {
            return Err(ApiError::NotFound);
        }
    }
    Ok(personal.then_some(user.id))
}

pub async fn next_position(pool: &PgPool, ws: Uuid, parent: Option<Uuid>) -> ApiResult<f64> {
    Ok(sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 65536 FROM study_sets WHERE workspace_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND deleted_at IS NULL")
        .bind(ws)
        .bind(parent)
        .fetch_one(pool)
        .await?)
}

async fn create_set(State(state): State<AppState>, user: AuthUser, Json(req): Json<CreateReq>) -> ApiResult<(StatusCode, Json<Value>)> {
    let title = validate_title(&req.title)?;
    if !KINDS.contains(&req.kind.as_str()) {
        return Err(bad("study_invalid_kind"));
    }
    validate_items(&req.kind, &req.items)?;
    let private_to = creation_place(&state.pool, &user, req.workspace_id, req.parent_id, req.personal).await?;
    let position = next_position(&state.pool, req.workspace_id, req.parent_id).await?;
    let mut tx = state.pool.begin().await?;
    let set: SetRow = sqlx::query_as("INSERT INTO study_sets (owner_id, workspace_id, parent_id, private_to, position, title, kind) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *")
        .bind(user.id)
        .bind(req.workspace_id)
        .bind(req.parent_id)
        .bind(private_to)
        .bind(position)
        .bind(&title)
        .bind(&req.kind)
        .fetch_one(&mut *tx)
        .await?;
    write_items(&mut tx, set.id, &req.items).await?;
    tx.commit().await?;
    emit(&state, &set, "STUDY_CREATED").await;
    Ok((StatusCode::CREATED, Json(json!(set))))
}

/// Prévient les autres onglets de l'espace (listes et arbre rechargés). Un entraînement
/// personnel n'est annoncé qu'à son propriétaire côté client (la liste est refiltrée).
async fn emit(state: &AppState, set: &SetRow, kind: &str) {
    state.emit(set.workspace_id, json!({ "type": kind, "study": { "id": set.id, "private_to": set.private_to } })).await;
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

/// L'entraînement et ses éléments. Pour qui ne peut que s'entraîner, les réponses attendues et
/// les explications des questions sont retirées (elles arrivent avec la correction) ; le
/// verso des fiches reste, il fait partie de l'étude.
async fn get_set(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let (set, acc) = load(&state.pool, &user, id, SetAccess::View).await?;
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
    v["can_edit"] = json!(acc == SetAccess::Edit);
    v["is_owner"] = json!(set.owner_id == user.id);
    v["items"] = json!(items);
    Ok(Json(v))
}

#[derive(Deserialize)]
struct UpdateReq {
    title: Option<String>,
    /// Le type ne change que si tous les éléments restent valides pour lui (éléments fournis).
    kind: Option<String>,
    items: Option<Vec<ItemIn>>,
    /// Rangement : une page de l'espace, ou `null` pour la racine.
    #[serde(default, deserialize_with = "double_option")]
    parent_id: Option<Option<Uuid>>,
    position: Option<f64>,
}

/// Versions gardées avant chaque modification par l'IA ou restauration, la plus récente d'abord. Réservé à qui peut éditer l'entraînement.
pub async fn revisions_of(pool: &PgPool, user: &AuthUser, id: Uuid) -> ApiResult<Vec<Value>> {
    load(pool, user, id, SetAccess::Edit).await?;
    let rows: Vec<(Uuid, DateTime<Utc>, String, String, i32)> =
        sqlx::query_as("SELECT id, created_at, source, title, jsonb_array_length(items)::int FROM study_set_revisions WHERE set_id = $1 ORDER BY created_at DESC, id LIMIT 100").bind(id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(id, at, source, title, n)| json!({ "id": id, "created_at": at, "source": source, "title": title, "item_count": n })).collect())
}

async fn list_revisions(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    Ok(Json(revisions_of(&state.pool, &user, id).await?))
}

async fn update_set(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(req): Json<UpdateReq>) -> ApiResult<Json<Value>> {
    let set = load_edit(&state.pool, &user, id).await?;
    let title = match &req.title {
        Some(t) => validate_title(t)?,
        None => set.title.clone(),
    };
    let kind = req.kind.clone().unwrap_or_else(|| set.kind.clone());
    if !KINDS.contains(&kind.as_str()) {
        return Err(bad("study_invalid_kind"));
    }
    match &req.items {
        Some(items) => validate_items(&kind, items)?,
        // Changer de type sans renvoyer les éléments les laisserait peut-être invalides.
        None if kind != set.kind => return Err(bad("study_items_required")),
        None => {}
    }
    // Rangement sous une page de l'espace, modifiable ; l'entraînement suit une page personnelle.
    let mut private_to = set.private_to;
    if let Some(Some(parent)) = req.parent_id {
        let (pws, _) = crate::access::require_page_write(&state.pool, &user, parent).await?;
        if pws != set.workspace_id {
            return Err(bad("invalid_parent"));
        }
        let owner: Option<Uuid> = sqlx::query_scalar("SELECT private_to FROM pages WHERE id = $1").bind(parent).fetch_one(&state.pool).await?;
        private_to = owner.or(private_to);
    }
    // Remonter à la racine : réservé aux rôles qui créent des documents, comme pour une page.
    if req.parent_id == Some(None) && set.parent_id.is_some() {
        crate::access::require_perm(&state.pool, &user, set.workspace_id, |p| p.docs >= level::EDIT).await?;
    }
    let mut tx = state.pool.begin().await?;
    let updated: SetRow = sqlx::query_as(
        "UPDATE study_sets SET title = $2, kind = $3,
            parent_id = CASE WHEN $4 THEN $5 ELSE parent_id END,
            position = COALESCE($6, position), private_to = $7, updated_at = NOW()
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&title)
    .bind(&kind)
    .bind(req.parent_id.is_some())
    .bind(req.parent_id.flatten())
    .bind(req.position)
    .bind(private_to)
    .fetch_one(&mut *tx)
    .await?;
    if let Some(items) = &req.items {
        write_items(&mut tx, id, items).await?;
    }
    tx.commit().await?;
    emit(&state, &updated, "STUDY_UPDATED").await;
    Ok(Json(json!({ "ok": true })))
}

/// À la corbeille (ou supprimé si l'espace n'en a pas) : droit d'édition des documents.
async fn delete_set(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let set = load_edit(&state.pool, &user, id).await?;
    crate::trash::trash_study(&state, &set, Some(user.id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct MoveReq {
    workspace_id: Uuid,
    /// Page d'accueil dans l'espace cible (sinon à la racine).
    parent_id: Option<Uuid>,
}

/// Déplace l'entraînement (dans le même espace ou vers un autre, perso compris), comme une
/// page : rôles qui créent des documents dans les deux espaces. Les liens vers l'ancien espace
/// sont retirés ; tentatives et état des fiches suivent chaque personne.
async fn move_set(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(req): Json<MoveReq>) -> ApiResult<Json<Value>> {
    let set = load_edit(&state.pool, &user, id).await?;
    let target = req.workspace_id;
    crate::access::require_perm(&state.pool, &user, set.workspace_id, |p| p.docs >= level::EDIT).await?;
    crate::access::require_perm(&state.pool, &user, target, |p| p.docs >= level::EDIT).await?;
    let mut private_to = set.private_to;
    if let Some(parent) = req.parent_id {
        let (pws, _) = crate::access::require_page_write(&state.pool, &user, parent).await?;
        if pws != target {
            return Err(bad("invalid_parent"));
        }
        let owner: Option<Uuid> = sqlx::query_scalar("SELECT private_to FROM pages WHERE id = $1").bind(parent).fetch_one(&state.pool).await?;
        private_to = owner.or(private_to);
    }
    let position = next_position(&state.pool, target, req.parent_id).await?;
    let mut tx = state.pool.begin().await?;
    let moved: SetRow = sqlx::query_as("UPDATE study_sets SET workspace_id = $2, parent_id = $3, position = $4, private_to = $5, updated_at = NOW() WHERE id = $1 RETURNING *")
        .bind(id)
        .bind(target)
        .bind(req.parent_id)
        .bind(position)
        .bind(private_to)
        .fetch_one(&mut *tx)
        .await?;
    if target != set.workspace_id {
        sqlx::query("DELETE FROM study_links WHERE set_id = $1").bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    if target != set.workspace_id {
        emit(&state, &set, "STUDY_DELETED").await;
    }
    emit(&state, &moved, if target != set.workspace_id { "STUDY_CREATED" } else { "STUDY_UPDATED" }).await;
    Ok(Json(json!({ "workspace_id": target, "study_id": id })))
}

#[derive(Deserialize, Default)]
struct DuplicateReq {
    title: Option<String>,
}

/// Copie l'entraînement (éléments et liens) au même endroit, au nom de la personne.
async fn duplicate_set(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, body: Option<Json<DuplicateReq>>) -> ApiResult<(StatusCode, Json<Value>)> {
    let set = load_edit(&state.pool, &user, id).await?;
    crate::access::require_perm(&state.pool, &user, set.workspace_id, |p| p.docs >= level::EDIT).await?;
    let title = match body.and_then(|b| b.0.title) {
        Some(t) => validate_title(&t)?,
        None => validate_title(&format!("{} (copy)", set.title.chars().take(180).collect::<String>()))?,
    };
    let position = next_position(&state.pool, set.workspace_id, set.parent_id).await?;
    let mut tx = state.pool.begin().await?;
    let copy: SetRow = sqlx::query_as("INSERT INTO study_sets (owner_id, workspace_id, parent_id, private_to, position, title, kind) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *")
        .bind(user.id)
        .bind(set.workspace_id)
        .bind(set.parent_id)
        .bind(set.private_to)
        .bind(position)
        .bind(&title)
        .bind(&set.kind)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO study_items (set_id, position, type, prompt, choices, answer, explanation) SELECT $2, position, type, prompt, choices, answer, explanation FROM study_items WHERE set_id = $1")
        .bind(id)
        .bind(copy.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO study_links (set_id, target_kind, target_id, created_by) SELECT $2, target_kind, target_id, $3 FROM study_links WHERE set_id = $1")
        .bind(id)
        .bind(copy.id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    emit(&state, &copy, "STUDY_CREATED").await;
    Ok((StatusCode::CREATED, Json(json!(copy))))
}

// --- Liens vers les autres éléments de l'espace ------------------------------

pub const LINK_KINDS: [&str; 5] = ["page", "task", "event", "board", "project"];

/// L'élément visé s'il est visible de la personne : (espace, titre). `None` : il n'existe pas
/// ou elle ne le voit pas (page personnelle d'autrui, tableau sans accès...).
pub async fn resolve_target(pool: &PgPool, user: &AuthUser, kind: &str, id: Uuid) -> ApiResult<Option<(Uuid, String)>> {
    let found = match kind {
        "page" => match crate::access::page_access(pool, user, id).await {
            Ok((ws, _, access)) if access != crate::access::PageAccess::None => {
                let title: Option<String> = sqlx::query_scalar("SELECT title FROM pages WHERE id = $1").bind(id).fetch_optional(pool).await?;
                title.map(|t| (ws, t))
            }
            _ => None,
        },
        "task" => match crate::access::task_ctx_for(pool, user, id).await {
            Ok(ctx) => {
                let ok = member_perms(pool, user.id, ctx.workspace_id).await?.is_some_and(|(_, p)| p.boards >= level::READ);
                let title: Option<String> = sqlx::query_scalar("SELECT title FROM tasks WHERE id = $1").bind(id).fetch_optional(pool).await?;
                title.filter(|_| ok).map(|t| (ctx.workspace_id, t))
            }
            Err(_) => None,
        },
        "board" => match crate::access::board_workspace_for(pool, user, id).await {
            Ok(ws) => {
                let ok = member_perms(pool, user.id, ws).await?.is_some_and(|(_, p)| p.boards >= level::READ);
                let name: Option<String> = sqlx::query_scalar("SELECT name FROM boards WHERE id = $1").bind(id).fetch_optional(pool).await?;
                name.filter(|_| ok).map(|n| (ws, n))
            }
            Err(_) => None,
        },
        "event" => {
            let row: Option<(Uuid, String)> = sqlx::query_as("SELECT workspace_id, ics FROM workspace_events WHERE id = $1").bind(id).fetch_optional(pool).await?;
            match row {
                Some((ws, ics)) if member_perms(pool, user.id, ws).await?.is_some_and(|(_, p)| p.calendar >= level::READ) => {
                    Some((ws, crate::calendar::master_event(&ics).map(|e| e.summary).unwrap_or_default()))
                }
                _ => None,
            }
        }
        "project" => {
            let row: Option<(Uuid, String, Option<Uuid>)> = sqlx::query_as("SELECT workspace_id, name, private_to FROM projects WHERE id = $1").bind(id).fetch_optional(pool).await?;
            match row {
                Some((ws, name, owner)) if owner.is_none_or(|o| o == user.id) && member_perms(pool, user.id, ws).await?.is_some_and(|(_, p)| p.docs >= level::READ) => Some((ws, name)),
                _ => None,
            }
        }
        _ => None,
    };
    Ok(found)
}

#[derive(Serialize)]
struct LinkOut {
    id: Uuid,
    target_kind: String,
    target_id: Uuid,
    title: String,
}

/// Liens de l'entraînement, sans ceux dont la cible échappe à la personne.
async fn list_links(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Vec<LinkOut>>> {
    let (set, _) = load(&state.pool, &user, id, SetAccess::View).await?;
    let rows: Vec<(Uuid, String, Uuid)> = sqlx::query_as("SELECT id, target_kind, target_id FROM study_links WHERE set_id = $1 ORDER BY created_at").bind(id).fetch_all(&state.pool).await?;
    let mut out = Vec::new();
    for (lid, kind, target) in rows {
        if let Some((ws, title)) = resolve_target(&state.pool, &user, &kind, target).await? {
            if ws == set.workspace_id {
                out.push(LinkOut { id: lid, target_kind: kind, target_id: target, title });
            }
        }
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
struct LinkReq {
    target_kind: String,
    target_id: Uuid,
}

/// Lie l'entraînement à un élément du même espace que la personne voit ; idempotent.
async fn create_link(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(req): Json<LinkReq>) -> ApiResult<(StatusCode, Json<Value>)> {
    let set = load_edit(&state.pool, &user, id).await?;
    if !LINK_KINDS.contains(&req.target_kind.as_str()) {
        return Err(bad("link_invalid"));
    }
    match resolve_target(&state.pool, &user, &req.target_kind, req.target_id).await? {
        Some((ws, _)) if ws == set.workspace_id => {}
        _ => return Err(bad("link_invalid")),
    }
    let link: Uuid = sqlx::query_scalar(
        "INSERT INTO study_links (set_id, target_kind, target_id, created_by) VALUES ($1, $2, $3, $4)
         ON CONFLICT (set_id, target_kind, target_id) DO UPDATE SET set_id = EXCLUDED.set_id RETURNING id",
    )
    .bind(id)
    .bind(&req.target_kind)
    .bind(req.target_id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    emit(&state, &set, "STUDY_UPDATED").await;
    Ok((StatusCode::CREATED, Json(json!({ "id": link }))))
}

async fn delete_link(State(state): State<AppState>, user: AuthUser, Path((id, link_id)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let set = load_edit(&state.pool, &user, id).await?;
    let n = sqlx::query("DELETE FROM study_links WHERE id = $1 AND set_id = $2").bind(link_id).bind(id).execute(&state.pool).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    emit(&state, &set, "STUDY_UPDATED").await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct LinkedQ {
    target_kind: String,
    target_id: Uuid,
}

/// « Entraînements liés » d'un élément : ceux de son espace qui y sont liés et que la personne
/// peut ouvrir. Vide si elle ne voit pas l'élément.
async fn linked_sets(State(state): State<AppState>, user: AuthUser, Query(q): Query<LinkedQ>) -> ApiResult<Json<Vec<Value>>> {
    let Some((ws, _)) = resolve_target(&state.pool, &user, &q.target_kind, q.target_id).await? else {
        return Ok(Json(vec![]));
    };
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT set_id FROM study_links WHERE target_kind = $1 AND target_id = $2").bind(&q.target_kind).bind(q.target_id).fetch_all(&state.pool).await?;
    if ids.is_empty() {
        return Ok(Json(vec![]));
    }
    Ok(Json(list_filtered(&state.pool, &user, Some(ws), Some(&ids)).await?))
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
async fn submit_attempt(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(req): Json<AttemptReq>) -> ApiResult<(StatusCode, Json<Value>)> {
    let (_, _) = load(&state.pool, &user, id, SetAccess::View).await?;
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
async fn list_attempts(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Vec<AttemptRow>>> {
    load(&state.pool, &user, id, SetAccess::View).await?;
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

async fn list_cards(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Vec<CardState>>> {
    load(&state.pool, &user, id, SetAccess::View).await?;
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

async fn review_card(State(state): State<AppState>, user: AuthUser, Path(item_id): Path<Uuid>, Json(req): Json<ReviewReq>) -> ApiResult<Json<CardState>> {
    let set_id: Uuid = sqlx::query_scalar("SELECT set_id FROM study_items WHERE id = $1 AND type = 'flashcard'").bind(item_id).fetch_optional(&state.pool).await?.ok_or(ApiError::NotFound)?;
    load(&state.pool, &user, set_id, SetAccess::View).await?;
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
    fn personal_set_is_readable_by_its_owner_only_administrators_included() {
        let (owner, other) = (u(1), u(2));
        assert_eq!(resolve_access(Some(owner), owner, member(Role::Member)), SetAccess::Edit);
        assert_eq!(resolve_access(Some(owner), other, member(Role::Owner)), SetAccess::None);
        assert_eq!(resolve_access(Some(owner), other, member(Role::Member)), SetAccess::None);
        // L'auteur sorti de l'espace n'y voit plus rien.
        assert_eq!(resolve_access(Some(owner), owner, None), SetAccess::None);
        // Sans accès aux documents de l'espace, l'auteur n'a plus la main.
        assert_eq!(resolve_access(Some(owner), owner, member(Role::Guest)), SetAccess::None);
    }

    #[test]
    fn shared_set_follows_the_documents_level_of_the_role() {
        let other = u(2);
        assert_eq!(resolve_access(None, other, member(Role::Member)), SetAccess::Edit);
        assert_eq!(resolve_access(None, other, member(Role::Viewer)), SetAccess::View);
        assert_eq!(resolve_access(None, other, member(Role::Guest)), SetAccess::None);
        assert_eq!(resolve_access(None, other, None), SetAccess::None);
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

    // --- Avec la base ---

    async fn user(pool: &PgPool, name: &str) -> AuthUser {
        let id: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id").bind(format!("{name}@example.org")).bind(name).fetch_one(pool).await.unwrap();
        AuthUser { id, is_super_admin: false }
    }

    async fn join(pool: &PgPool, ws: Uuid, u: &AuthUser, role: &str) {
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3)").bind(ws).bind(u.id.to_string()).bind(role).execute(pool).await.unwrap();
    }

    struct World {
        ws: Uuid,
        owner: AuthUser,
        member: AuthUser,
        viewer: AuthUser,
        outsider: AuthUser,
    }

    async fn world(pool: &PgPool) -> World {
        let (owner, member, viewer, outsider) = (user(pool, "owner").await, user(pool, "member").await, user(pool, "viewer").await, user(pool, "outsider").await);
        let ws: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('W') RETURNING id").fetch_one(pool).await.unwrap();
        join(pool, ws, &owner, "owner").await;
        join(pool, ws, &member, "member").await;
        join(pool, ws, &viewer, "viewer").await;
        World { ws, owner, member, viewer, outsider }
    }

    async fn insert_set(pool: &PgPool, w: &World, author: &AuthUser, private_to: Option<Uuid>, parent: Option<Uuid>) -> Uuid {
        sqlx::query_scalar("INSERT INTO study_sets (owner_id, workspace_id, parent_id, private_to, title, kind) VALUES ($1, $2, $3, $4, 'T', 'qcm') RETURNING id")
            .bind(author.id)
            .bind(w.ws)
            .bind(parent)
            .bind(private_to)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[sqlx::test]
    async fn loading_follows_the_documents_level_and_personal_items(pool: PgPool) {
        let w = world(&pool).await;
        let shared = insert_set(&pool, &w, &w.member, None, None).await;
        let personal = insert_set(&pool, &w, &w.member, Some(w.member.id), None).await;
        // Éditeur de documents : modifie ; lecteur : s'entraîne seulement ; extérieur : rien.
        assert_eq!(load(&pool, &w.owner, shared, SetAccess::Edit).await.unwrap().1, SetAccess::Edit);
        assert_eq!(load(&pool, &w.viewer, shared, SetAccess::View).await.unwrap().1, SetAccess::View);
        assert!(matches!(load(&pool, &w.viewer, shared, SetAccess::Edit).await, Err(ApiError::NotFound)));
        assert!(load_edit(&pool, &w.viewer, shared).await.is_err());
        assert!(matches!(load(&pool, &w.outsider, shared, SetAccess::View).await, Err(ApiError::NotFound)));
        // Personnel : son auteur seul, même l'administrateur de l'espace n'y a pas accès.
        assert!(load(&pool, &w.member, personal, SetAccess::Edit).await.is_ok());
        assert!(matches!(load(&pool, &w.owner, personal, SetAccess::View).await, Err(ApiError::NotFound)));
        // La liste applique les mêmes règles.
        let ids = |v: Vec<Value>| v.iter().map(|s| s["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(ids(list_for(&pool, &w.owner, None).await.unwrap()), vec![shared.to_string()]);
        assert_eq!(ids(list_for(&pool, &w.member, Some(w.ws)).await.unwrap()).len(), 2);
        assert!(list_for(&pool, &w.outsider, None).await.unwrap().is_empty());
        // Corbeille : un entraînement supprimé n'est plus ouvrable.
        sqlx::query("UPDATE study_sets SET deleted_at = NOW() WHERE id = $1").bind(shared).execute(&pool).await.unwrap();
        assert!(load(&pool, &w.owner, shared, SetAccess::View).await.is_err());
    }

    #[sqlx::test]
    async fn creation_place_follows_pages_rules(pool: PgPool) {
        let w = world(&pool).await;
        // Le lecteur ne crée pas d'entraînement partagé, mais peut en avoir un personnel.
        assert!(matches!(creation_place(&pool, &w.viewer, w.ws, None, false).await, Err(ApiError::Forbidden)));
        assert_eq!(creation_place(&pool, &w.viewer, w.ws, None, true).await.unwrap(), Some(w.viewer.id));
        assert_eq!(creation_place(&pool, &w.member, w.ws, None, false).await.unwrap(), None);
        assert!(creation_place(&pool, &w.outsider, w.ws, None, true).await.is_err());
        // Sous une page personnelle : personnel d'office ; sous la page personnelle d'autrui : introuvable.
        let page: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, created_by, private_to) VALUES ($1, 'P', $2, $2) RETURNING id").bind(w.ws).bind(w.member.id).fetch_one(&pool).await.unwrap();
        assert_eq!(creation_place(&pool, &w.member, w.ws, Some(page), false).await.unwrap(), Some(w.member.id));
        assert!(creation_place(&pool, &w.owner, w.ws, Some(page), false).await.is_err());
        // Sous une page partagée : l'éditeur peut, le lecteur seulement en personnel.
        let shared: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, created_by) VALUES ($1, 'S', $2) RETURNING id").bind(w.ws).bind(w.owner.id).fetch_one(&pool).await.unwrap();
        assert_eq!(creation_place(&pool, &w.member, w.ws, Some(shared), false).await.unwrap(), None);
        assert!(creation_place(&pool, &w.viewer, w.ws, Some(shared), false).await.is_err());
        assert_eq!(creation_place(&pool, &w.viewer, w.ws, Some(shared), true).await.unwrap(), Some(w.viewer.id));
    }

    #[sqlx::test]
    async fn links_are_hidden_from_people_who_cannot_see_the_target(pool: PgPool) {
        let w = world(&pool).await;
        let open: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, created_by) VALUES ($1, 'Cours', $2) RETURNING id").bind(w.ws).bind(w.owner.id).fetch_one(&pool).await.unwrap();
        let secret: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, created_by, private_to) VALUES ($1, 'Secret', $2, $2) RETURNING id").bind(w.ws).bind(w.owner.id).fetch_one(&pool).await.unwrap();
        assert_eq!(resolve_target(&pool, &w.viewer, "page", open).await.unwrap(), Some((w.ws, "Cours".to_string())));
        assert_eq!(resolve_target(&pool, &w.viewer, "page", secret).await.unwrap(), None);
        assert!(resolve_target(&pool, &w.owner, "page", secret).await.unwrap().is_some());
        assert_eq!(resolve_target(&pool, &w.outsider, "page", open).await.unwrap(), None);
        assert_eq!(resolve_target(&pool, &w.owner, "page", Uuid::new_v4()).await.unwrap(), None);
        assert_eq!(resolve_target(&pool, &w.owner, "nonsense", open).await.unwrap(), None);
    }

    #[sqlx::test]
    async fn attempts_stay_per_user_whoever_edits(pool: PgPool) {
        let w = world(&pool).await;
        let set = insert_set(&pool, &w, &w.member, None, None).await;
        sqlx::query("INSERT INTO study_attempts (set_id, user_id, score, max_score) VALUES ($1, $2, 3, 4)").bind(set).bind(w.viewer.id).execute(&pool).await.unwrap();
        // L'autrice voit ses propres résultats (aucun), pas ceux du lecteur.
        let rows: Vec<Value> = list_for(&pool, &w.member, None).await.unwrap();
        assert!(rows[0]["last_score"].is_null());
        let rows = list_for(&pool, &w.viewer, None).await.unwrap();
        assert_eq!(rows[0]["last_score"], json!(3));
    }
}
