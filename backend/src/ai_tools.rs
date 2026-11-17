//! Outils offerts aux modèles et garde-fous d'écriture (docs/adr/0010).
//!
//! Tout est vérifié ici, côté serveur, avec les droits de la PERSONNE qui discute : le texte
//! de l'invite ou du modèle n'accorde jamais rien. Lecture seule par défaut ; écrire exige
//! une autorisation (`ai_write_grants`) valide pour cette conversation et cette page, plus le
//! droit d'écriture de la personne sur la page (`require_page_write`).

use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::{level, require_member_or_guest, require_perm, require_page_read, require_page_write, visible_pages, AuthUser, PageAccess};
use crate::ai_pages::{self, Op};
use crate::ai_provider::ToolSpec;
use crate::study;
use crate::error::{ApiError, ApiResult};

pub const MODES: [&str; 3] = ["suggest", "append", "replace"];

pub fn tool_specs() -> Vec<ToolSpec> {
    let mut v = vec![
        ToolSpec {
            name: "list_pages",
            description: "List the pages the user can access. Without workspace_id, lists the user's workspaces (or the pages of the conversation's workspace).",
            parameters: json!({ "type": "object", "properties": { "workspace_id": { "type": "string", "description": "Workspace UUID" } } }),
        },
        ToolSpec {
            name: "read_page",
            description: "Read a page as Markdown.",
            parameters: json!({ "type": "object", "properties": { "page_id": { "type": "string" } }, "required": ["page_id"] }),
        },
        ToolSpec {
            name: "write_page",
            description: "Write Markdown to a page. Only works on the page the user explicitly granted for writing in this conversation, within the granted mode: it may only propose a change (the user then accepts it), or append, or replace. `append` adds to the end; `replace` rewrites the whole page.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "page_id": { "type": "string" },
                    "markdown": { "type": "string" },
                    "operation": { "type": "string", "enum": ["append", "replace"], "description": "Default: append" }
                },
                "required": ["page_id", "markdown"]
            }),
        },
    ];
    v.extend(study_specs());
    v
}

pub struct ToolOutput {
    /// Texte renvoyé au modèle.
    pub text: String,
    /// Proposition à afficher (mode « suggest »).
    pub suggestion: Option<Value>,
    /// Page modifiée (l'interface recharge).
    pub changed_page: Option<Uuid>,
    /// Jeu de révision créé ou modifié (l'interface en affiche un lien).
    pub study_set: Option<Uuid>,
    /// Version du jeu enregistrée avant une modification : l'interface propose de la restaurer.
    pub study_revision: Option<Uuid>,
}

impl ToolOutput {
    fn text(t: impl Into<String>) -> Self {
        ToolOutput { text: t.into(), suggestion: None, changed_page: None, study_set: None, study_revision: None }
    }
}

/// Message d'erreur renvoyé au modèle (sans détail interne).
fn error_text(e: &ApiError) -> String {
    match e {
        ApiError::Forbidden => "error: forbidden. Either no write grant is active for this page in this conversation (ask the user to grant it in the chat), the grant mode does not allow this operation, or the user has no edit right on the page or study set. For study sets: ask the user to grant 'create study sets' in that workspace (or personal space), or an update grant on that set.".into(),
        ApiError::NotFound => "error: not found, or not accessible to the user.".into(),
        ApiError::BadRequest(m) | ApiError::Conflict(m) => format!("error: {m}"),
        _ => "error: internal error".into(),
    }
}

pub async fn execute(pool: &PgPool, user: &AuthUser, conv_id: Uuid, name: &str, args: &Value) -> ToolOutput {
    let r = match name {
        "list_pages" => list_pages(pool, user, conv_id, args).await,
        "read_page" => read_page(pool, user, args).await,
        "write_page" => write_page_tool(pool, user, conv_id, args).await,
        "create_study_set" => create_study_set_tool(pool, user, conv_id, args).await,
        "list_study_sets" => list_study_sets(pool, user).await,
        "read_study_set" => read_study_set(pool, user, args).await,
        "update_study_set" => update_study_set_tool(pool, user, conv_id, args).await,
        "read_attempts" => read_attempts(pool, user, args).await,
        other => Ok(ToolOutput::text(format!("error: unknown tool {other}"))),
    };
    r.unwrap_or_else(|e| ToolOutput::text(error_text(&e)))
}

fn uuid_arg(args: &Value, key: &str) -> ApiResult<Uuid> {
    args[key].as_str().and_then(|s| Uuid::parse_str(s.trim()).ok()).ok_or_else(|| ApiError::bad_request(format!("{key} must be a UUID")))
}

/// Espace de la conversation (page, projet ou espace en contexte).
async fn context_workspace(pool: &PgPool, conv_id: Uuid) -> ApiResult<Option<Uuid>> {
    let row: Option<(String, Option<Uuid>)> = sqlx::query_as("SELECT context_kind, context_id FROM ai_conversations WHERE id = $1").bind(conv_id).fetch_optional(pool).await?;
    let Some((kind, Some(id))) = row else { return Ok(None) };
    Ok(match kind.as_str() {
        "workspace" => Some(id),
        "page" => sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(id).fetch_optional(pool).await?,
        "project" => sqlx::query_scalar("SELECT workspace_id FROM projects WHERE id = $1").bind(id).fetch_optional(pool).await?,
        _ => None,
    })
}

async fn list_pages(pool: &PgPool, user: &AuthUser, conv_id: Uuid, args: &Value) -> ApiResult<ToolOutput> {
    let ws = match args["workspace_id"].as_str().filter(|s| !s.is_empty()) {
        Some(_) => Some(uuid_arg(args, "workspace_id")?),
        None => context_workspace(pool, conv_id).await?,
    };
    let Some(ws) = ws else {
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT w.id, w.name FROM workspaces w JOIN workspace_members m ON m.workspace_id = w.id WHERE m.user_id = $1 ORDER BY w.name",
        )
        .bind(user.id.to_string())
        .fetch_all(pool)
        .await?;
        let list: Vec<Value> = rows.into_iter().map(|(id, name)| json!({ "workspace_id": id, "name": name })).collect();
        return Ok(ToolOutput::text(json!({ "workspaces": list, "hint": "call list_pages again with a workspace_id" }).to_string()));
    };
    // Invalide l'accès (NotFound) si la personne n'est ni membre ni invitée de l'espace.
    let role = require_member_or_guest(pool, user, ws).await?;
    let visible = visible_pages(pool, user, ws, role).await?;
    let rows: Vec<(Uuid, Option<Uuid>, String)> = sqlx::query_as("SELECT id, parent_id, title FROM pages WHERE workspace_id = $1 ORDER BY position, created_at LIMIT 2000").bind(ws).fetch_all(pool).await?;
    let pages: Vec<Value> = rows
        .into_iter()
        .filter_map(|(id, parent, title)| match visible.get(&id) {
            Some(PageAccess::None) | None => None,
            Some(a) => Some(json!({ "page_id": id, "parent_id": parent, "title": title, "can_edit": *a == PageAccess::Write })),
        })
        .take(300)
        .collect();
    Ok(ToolOutput::text(json!({ "workspace_id": ws, "pages": pages }).to_string()))
}

/// Markdown courant d'une page : l'état Yjs, ou à défaut l'ancien HTML (converti si possible, sinon
/// son texte approximatif : lisible, mais la page ne sera pas modifiable, voir `legacy_state`).
pub async fn page_markdown(pool: &PgPool, page_id: Uuid) -> ApiResult<(String, String)> {
    let row: (String, Option<Vec<u8>>, Option<String>) = sqlx::query_as("SELECT title, yjs_state, content FROM pages WHERE id = $1").bind(page_id).fetch_one(pool).await?;
    let md = match row.1.as_deref().filter(|s| !s.is_empty()) {
        Some(state) => ai_pages::state_to_markdown(state).map_err(|e| ApiError::Internal(format!("yjs decode: {e}")))?,
        None => {
            let html = row.2.as_deref().unwrap_or("");
            match legacy_state(html) {
                Ok(state) => ai_pages::state_to_markdown(&state).map_err(|e| ApiError::Internal(format!("yjs decode: {e}")))?,
                Err(_) => ai_pages::html_to_text(html).trim().to_string(),
            }
        }
    };
    Ok((row.0, md))
}

/// État Yjs équivalent de l'ancien HTML d'une page jamais ouverte dans l'éditeur. `Err(balise)` si le
/// HTML contient un bloc que la conversion ne sait pas reproduire sans perte (image, tableau…) : la page
/// reste alors en lecture seule pour les agents.
pub fn legacy_state(html: &str) -> Result<Vec<u8>, String> {
    ai_pages::html_to_blocks(html).map(|b| ai_pages::blocks_to_state(&b))
}

async fn read_page(pool: &PgPool, user: &AuthUser, args: &Value) -> ApiResult<ToolOutput> {
    let page_id = uuid_arg(args, "page_id")?;
    let (ws, _, access) = require_page_read(pool, user, page_id).await?;
    let (title, mut md) = page_markdown(pool, page_id).await?;
    let truncated = md.chars().count() > ai_pages::MAX_READ_CHARS;
    if truncated {
        md = md.chars().take(ai_pages::MAX_READ_CHARS).collect();
    }
    Ok(ToolOutput::text(json!({ "page_id": page_id, "workspace_id": ws, "title": title, "can_edit": access == PageAccess::Write, "truncated": truncated, "markdown": md }).to_string()))
}

// ---------------------------------------------------------------------------
// Écriture
// ---------------------------------------------------------------------------

/// Mode de l'autorisation encore valable pour cette conversation et cette page, ou Forbidden.
pub async fn active_grant(pool: &PgPool, conv_id: Uuid, page_id: Uuid) -> ApiResult<(Uuid, String)> {
    sqlx::query_as(
        "SELECT id, mode FROM ai_write_grants
         WHERE conversation_id = $1 AND page_id = $2 AND revoked_at IS NULL AND expires_at > now()
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(conv_id)
    .bind(page_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::Forbidden)
}

pub enum WriteOutcome {
    Applied { revision: Option<Uuid> },
    Suggested { suggestion_id: Uuid, diff: Vec<(char, String)> },
}

/// Seul point d'entrée des écritures demandées par un modèle. Ordre des contrôles :
/// autorisation de la conversation pour CETTE page, droit d'écriture de la personne, puis
/// mode. Le contenu n'est écrit (après un instantané) qu'une fois tout validé.
pub async fn write_page(pool: &PgPool, user: &AuthUser, conv_id: Uuid, page_id: Uuid, op: Op, markdown: &str) -> ApiResult<WriteOutcome> {
    // La conversation doit appartenir à la personne : on n'utilise jamais l'autorisation d'autrui.
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM ai_conversations WHERE id = $1").bind(conv_id).fetch_optional(pool).await?;
    if owner != Some(user.id) {
        return Err(ApiError::NotFound);
    }
    let (grant_id, mode) = active_grant(pool, conv_id, page_id).await?;
    require_page_write(pool, user, page_id).await?;
    if mode == "append" && op == Op::Replace {
        return Err(ApiError::Forbidden);
    }
    if markdown.len() > 500_000 {
        return Err(ApiError::bad_request("markdown_too_large"));
    }
    let (_, before) = page_markdown(pool, page_id).await?;
    if mode == "suggest" {
        let after = match op {
            Op::Replace => markdown.to_string(),
            Op::Append => format!("{before}\n\n{markdown}").trim_start().to_string(),
        };
        let diff = ai_pages::diff_lines(&before, &after);
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO ai_suggestions (conversation_id, grant_id, page_id, before_md, after_md, apply_mode, payload_md)
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        )
        .bind(conv_id)
        .bind(grant_id)
        .bind(page_id)
        .bind(&before)
        .bind(&after)
        .bind(if op == Op::Append { "append" } else { "replace" })
        .bind(markdown)
        .fetch_one(pool)
        .await?;
        return Ok(WriteOutcome::Suggested { suggestion_id: id, diff });
    }
    let revision = apply_change(pool, user, page_id, op, markdown, None).await?;
    Ok(WriteOutcome::Applied { revision })
}

/// Écrit réellement : l'instantané de l'état lu et le remplacement conditionnel (seulement si
/// personne n'a écrit entre-temps) forment une seule transaction : pas de version orpheline si
/// l'échange échoue. `expect_before` : Markdown que la page doit encore avoir (suggestion périmée).
/// Le serveur de collaboration fusionne ensuite le nouvel état dans son exemplaire en mémoire.
pub async fn apply_change(pool: &PgPool, user: &AuthUser, page_id: Uuid, op: Op, markdown: &str, expect_before: Option<&str>) -> ApiResult<Option<Uuid>> {
    require_page_write(pool, user, page_id).await?;
    let mut tx = pool.begin().await?;
    let row: (Option<Vec<u8>>, Option<String>) = sqlx::query_as("SELECT yjs_state, content FROM pages WHERE id = $1").bind(page_id).fetch_one(&mut *tx).await?;
    let raw = row.0;
    let mut old = raw.clone().filter(|s| !s.is_empty());
    // Page jamais ouverte dans l'éditeur : son texte n'existe qu'en HTML. On le convertit en document
    // Yjs seulement s'il ne contient que des blocs reproductibles sans perte ; sinon on refuse.
    // Cette version convertie est l'instantané « avant » (la restauration redonne le texte d'origine).
    let raw_content = row.1;
    let html = raw_content.clone().unwrap_or_default();
    if old.is_none() && !html.trim().is_empty() {
        match legacy_state(&html) {
            Ok(state) => old = Some(state),
            Err(what) => {
                return Err(ApiError::Conflict(format!(
                    "page_not_convertible: this page was never opened in the Looty editor and its old content has blocks the assistant cannot convert without loss ({what}). Ask the user to open the page once in the editor (it is then converted), then retry this write."
                )))
            }
        }
    }
    if let Some(expected) = expect_before {
        let current = match old.as_deref() {
            Some(s) => ai_pages::state_to_markdown(s).map_err(|e| ApiError::Internal(format!("yjs decode: {e}")))?,
            None => String::new(),
        };
        if current != expected {
            return Err(ApiError::Conflict("suggestion_stale: the page changed since this suggestion was made".into()));
        }
    }
    let new_state = ai_pages::apply_markdown(old.as_deref().unwrap_or(&[]), op, markdown).map_err(|e| ApiError::Internal(format!("yjs write: {e}")))?;
    // `pages.content` (page publique, export, recherche) doit suivre le nouveau texte : le serveur de
    // collaboration ne l'écrit pas (c'est le navigateur qui l'envoie) ; ici personne ne le fera.
    // Si le nouvel état contient un bloc ou une marque que le HTML ne sait pas écrire (intégrations, tableaux de
    // bord, commentaires...), la copie HTML reste telle quelle plutôt que de les perdre.
    let new_html = ai_pages::state_to_html(&new_state).ok().or_else(|| raw_content.clone());
    let revision = match old.as_deref() {
        Some(s) => Some(crate::revisions::snapshot_before_write(&mut tx, page_id, user.id, "Avant modification par l'IA", s).await?),
        None => None,
    };
    let done = sqlx::query("UPDATE pages_all SET yjs_state = $1, content = $5, updated_at = NOW(), updated_by = $2 WHERE id = $3 AND yjs_state IS NOT DISTINCT FROM $4 AND content IS NOT DISTINCT FROM $6")
        .bind(&new_state)
        .bind(user.id)
        .bind(page_id)
        .bind(raw.as_deref())
        .bind(&new_html)
        .bind(&raw_content)
        .execute(&mut *tx)
        .await?;
    if done.rows_affected() != 1 {
        return Err(ApiError::Conflict("page_changed_meanwhile: read the page again and retry".into()));
    }
    tx.commit().await?;
    // Le serveur de collaboration fusionne ce nouvel état dans le document ouvert (aucune frappe perdue).
    let _ = sqlx::query("SELECT pg_notify('looty_page_reset', $1)").bind(page_id.to_string()).execute(pool).await;
    Ok(revision)
}

/// Accepte une suggestion, une seule fois : elle passe d'abord de `pending` à `accepted` (une
/// seconde acceptation échoue), puis revient à `pending` si l'écriture échoue.
pub async fn accept_suggestion(pool: &PgPool, user: &AuthUser, id: Uuid) -> ApiResult<(Uuid, Option<Uuid>)> {
    let claimed: Option<(Uuid, String, String, String)> = sqlx::query_as(
        "UPDATE ai_suggestions s SET status = 'accepted'
         WHERE s.id = $1 AND s.status = 'pending' AND EXISTS (SELECT 1 FROM ai_conversations c WHERE c.id = s.conversation_id AND c.user_id = $2)
         RETURNING s.page_id, s.apply_mode, s.payload_md, s.before_md",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(pool)
    .await?;
    let Some((page_id, mode, md, before)) = claimed else {
        return Err(ApiError::Conflict("suggestion_not_pending".into()));
    };
    let op = if mode == "append" { Op::Append } else { Op::Replace };
    let expect = (op == Op::Replace).then_some(before.as_str());
    match apply_change(pool, user, page_id, op, &md, expect).await {
        Ok(rev) => Ok((page_id, rev)),
        Err(e) => {
            let _ = sqlx::query("UPDATE ai_suggestions SET status = 'pending' WHERE id = $1 AND status = 'accepted'").bind(id).execute(pool).await;
            Err(e)
        }
    }
}

async fn write_page_tool(pool: &PgPool, user: &AuthUser, conv_id: Uuid, args: &Value) -> ApiResult<ToolOutput> {
    let page_id = uuid_arg(args, "page_id")?;
    let md = args["markdown"].as_str().ok_or_else(|| ApiError::bad_request("markdown is required"))?;
    let op = match args["operation"].as_str().unwrap_or("append") {
        "append" => Op::Append,
        "replace" => Op::Replace,
        _ => return Err(ApiError::bad_request("operation must be append or replace")),
    };
    Ok(match write_page(pool, user, conv_id, page_id, op, md).await? {
        WriteOutcome::Applied { .. } => ToolOutput { text: "ok: the page was updated (a restorable version was saved before).".into(), suggestion: None, changed_page: Some(page_id), study_set: None, study_revision: None },
        WriteOutcome::Suggested { suggestion_id, diff } => ToolOutput {
            text: "ok: the change was NOT applied; it was proposed to the user, who must accept it in the chat. Do not claim the page changed.".into(),
            suggestion: Some(json!({ "id": suggestion_id, "page_id": page_id, "diff": diff.iter().map(|(o, l)| json!({ "op": o.to_string(), "text": l })).collect::<Vec<_>>() })),
            changed_page: None,
            study_set: None,
            study_revision: None,
        },
    })
}

// ---------------------------------------------------------------------------
// Jeux de révision (docs/study-items.md, ADR 0010 addendum)
// ---------------------------------------------------------------------------

fn study_specs() -> Vec<ToolSpec> {
    let item_schema = json!({
        "type": "object",
        "description": "Study item, see the STUDY SETS section of the system prompt. Unknown fields are rejected.",
        "properties": {
            "id": { "type": "string", "description": "Only when updating an existing item (keeps its history)" },
            "type": { "type": "string", "enum": ["single", "multi", "text", "flashcard"] },
            "prompt": { "type": "string" },
            "choices": { "type": "array", "items": { "type": "object", "properties": { "id": { "type": "string" }, "text": { "type": "string" } }, "required": ["id", "text"] } },
            "answer": { "type": "object" },
            "explanation": { "type": "string" }
        },
        "required": ["type", "prompt", "answer"]
    });
    vec![
        ToolSpec {
            name: "create_study_set",
            description: "Create a PERSONAL entrainement (study set: qcm, questionnaire, flashcards or summary) visible to the user only. Only works in a workspace (or the personal space when workspace_id is omitted) for which the user granted 'create study sets' in this conversation. Items are validated strictly; fix and retry on error.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "kind": { "type": "string", "enum": ["qcm", "questionnaire", "flashcards", "summary"] },
                    "workspace_id": { "type": "string", "description": "Omit for the user's personal space" },
                    "items": { "type": "array", "items": item_schema.clone() }
                },
                "required": ["title", "kind", "items"]
            }),
        },
        ToolSpec { name: "list_study_sets", description: "List the study sets (entrainements) the user can access (personal ones and those of the user workspaces).", parameters: json!({ "type": "object", "properties": {} }) },
        ToolSpec {
            name: "read_study_set",
            description: "Read a study set and its items. Expected answers and explanations are only returned when the user can edit the set.",
            parameters: json!({ "type": "object", "properties": { "set_id": { "type": "string" } }, "required": ["set_id"] }),
        },
        ToolSpec {
            name: "update_study_set",
            description: "Replace the title and the whole item list of a study set (send ALL items; keep the id of the items you keep). Only works on a set the user granted for update in this conversation and can edit. A restorable version is saved first.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "set_id": { "type": "string" },
                    "title": { "type": "string", "description": "Default: unchanged" },
                    "kind": { "type": "string", "enum": ["qcm", "questionnaire", "flashcards", "summary"], "description": "Default: unchanged" },
                    "items": { "type": "array", "items": item_schema }
                },
                "required": ["set_id", "items"]
            }),
        },
        ToolSpec {
            name: "read_attempts",
            description: "Read the user's OWN attempts (never anyone else's) to find weak points: per set the scores, and with set_id the items most often missed. Without set_id, summarises all the user's sets.",
            parameters: json!({ "type": "object", "properties": { "set_id": { "type": "string" } } }),
        },
    ]
}

async fn own_conversation(pool: &PgPool, user: &AuthUser, conv_id: Uuid) -> ApiResult<()> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM ai_conversations WHERE id = $1").bind(conv_id).fetch_optional(pool).await?;
    if owner != Some(user.id) {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

/// Autorisation de CRÉER des jeux dans cet espace (`None` : espace perso), encore valable.
pub async fn require_create_grant(pool: &PgPool, conv_id: Uuid, workspace: Option<Uuid>) -> ApiResult<()> {
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM ai_study_grants WHERE conversation_id = $1 AND kind = 'create' AND revoked_at IS NULL AND expires_at > now()
                          AND (($2::uuid IS NOT NULL AND workspace_id = $2) OR ($2::uuid IS NULL AND personal)))",
    )
    .bind(conv_id)
    .bind(workspace)
    .fetch_one(pool)
    .await?;
    ok.then_some(()).ok_or(ApiError::Forbidden)
}

/// Autorisation de MODIFIER ce jeu, encore valable.
pub async fn require_update_grant(pool: &PgPool, conv_id: Uuid, set_id: Uuid) -> ApiResult<()> {
    let ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM ai_study_grants WHERE conversation_id = $1 AND kind = 'update' AND set_id = $2 AND revoked_at IS NULL AND expires_at > now())")
        .bind(conv_id)
        .bind(set_id)
        .fetch_one(pool)
        .await?;
    ok.then_some(()).ok_or(ApiError::Forbidden)
}

fn parse_items(args: &Value) -> ApiResult<Vec<study::ItemIn>> {
    serde_json::from_value(args["items"].clone()).map_err(|e| ApiError::bad_request(format!("study_items_schema: {e}")))
}

pub async fn create_study_set(pool: &PgPool, user: &AuthUser, conv_id: Uuid, title: &str, kind: &str, workspace: Option<Uuid>, items: &[study::ItemIn]) -> ApiResult<Uuid> {
    own_conversation(pool, user, conv_id).await?;
    require_create_grant(pool, conv_id, workspace).await?;
    // Droit propre de la personne : modifier les documents de l'espace (l'espace perso est le sien).
    let ws = match workspace {
        Some(ws) => {
            require_perm(pool, user, ws, |p| p.docs >= level::EDIT).await?;
            ws
        }
        None => crate::personal::personal_space(pool, user.id).await?,
    };
    let title = study::validate_title(title)?;
    if !study::KINDS.contains(&kind) {
        return Err(ApiError::bad_request("study_invalid_kind"));
    }
    study::validate_items(kind, items)?;
    let mut tx = pool.begin().await?;
    // Toujours personnel (private_to) : le partage reste un choix de la personne (ADR 0011).
    let position = study::next_position(pool, ws, None).await?;
    let id: Uuid = sqlx::query_scalar("INSERT INTO study_sets (owner_id, workspace_id, private_to, position, title, kind) VALUES ($1, $2, $1, $3, $4, $5) RETURNING id")
        .bind(user.id)
        .bind(ws)
        .bind(position)
        .bind(&title)
        .bind(kind)
        .fetch_one(&mut *tx)
        .await?;
    study::write_items(&mut tx, id, items).await?;
    tx.commit().await?;
    Ok(id)
}

/// Éléments d'un jeu au format d'écriture (avec id), pour l'instantané.
fn items_as_input(items: &[study::ItemRow]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|i| json!({ "id": i.id, "type": i.kind, "prompt": i.prompt, "choices": i.choices.clone().unwrap_or_else(|| json!([])), "answer": i.answer, "explanation": i.explanation }))
            .collect(),
    )
}

/// Verrouille le jeu puis garde son état courant (titre, type, éléments) comme version ; renvoie l'id de la version.
async fn snapshot_locked(tx: &mut sqlx::PgConnection, set_id: Uuid, user_id: Uuid, source: &str) -> ApiResult<Uuid> {
    let (title, kind): (String, String) = sqlx::query_as("SELECT title, kind FROM study_sets WHERE id = $1 FOR UPDATE").bind(set_id).fetch_optional(&mut *tx).await?.ok_or(ApiError::NotFound)?;
    let before: Vec<study::ItemRow> = sqlx::query_as("SELECT id, position, type, prompt, choices, answer, explanation FROM study_items WHERE set_id = $1 ORDER BY position, id").bind(set_id).fetch_all(&mut *tx).await?;
    Ok(sqlx::query_scalar("INSERT INTO study_set_revisions (set_id, user_id, title, kind, items, source) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id")
        .bind(set_id)
        .bind(user_id)
        .bind(&title)
        .bind(&kind)
        .bind(items_as_input(&before))
        .bind(source)
        .fetch_one(&mut *tx)
        .await?)
}

/// Remplace titre, type et éléments après avoir gardé la version d'avant ; renvoie l'id de cette version.
pub async fn update_study_set(pool: &PgPool, user: &AuthUser, conv_id: Uuid, set_id: Uuid, title: Option<&str>, kind: Option<&str>, items: &[study::ItemIn]) -> ApiResult<Uuid> {
    own_conversation(pool, user, conv_id).await?;
    require_update_grant(pool, conv_id, set_id).await?;
    let (set, _) = study::load(pool, user, set_id, study::SetAccess::Edit).await?;
    // Une liste vide effacerait tout le jeu : jamais par un modèle.
    if items.is_empty() {
        return Err(ApiError::bad_request("study_items_empty: send the full, non-empty item list"));
    }
    let title = study::validate_title(title.unwrap_or(&set.title))?;
    let kind = kind.unwrap_or(&set.kind);
    if !study::KINDS.contains(&kind) {
        return Err(ApiError::bad_request("study_invalid_kind"));
    }
    study::validate_items(kind, items)?;
    let mut tx = pool.begin().await?;
    let rev = snapshot_locked(&mut tx, set_id, user.id, "ai").await?;
    sqlx::query("UPDATE study_sets SET title = $2, kind = $3, updated_at = NOW() WHERE id = $1").bind(set_id).bind(&title).bind(kind).execute(&mut *tx).await?;
    study::write_items(&mut tx, set_id, items).await?;
    tx.commit().await?;
    Ok(rev)
}

/// Annule une modification de l'IA : remet titre, type et éléments d'avant. Réservé à qui peut éditer le jeu.
/// L'état remplacé est lui-même gardé comme version (la restauration s'annule aussi) ; renvoie (jeu, cette version).
pub async fn restore_study_revision(pool: &PgPool, user: &AuthUser, rev_id: Uuid) -> ApiResult<(Uuid, Uuid)> {
    let (set_id, title, kind, items): (Uuid, String, String, Value) = sqlx::query_as("SELECT set_id, title, kind, items FROM study_set_revisions WHERE id = $1").bind(rev_id).fetch_optional(pool).await?.ok_or(ApiError::NotFound)?;
    study::load(pool, user, set_id, study::SetAccess::Edit).await?;
    let items: Vec<study::ItemIn> = serde_json::from_value(items).map_err(|e| ApiError::Internal(format!("revision items: {e}")))?;
    let mut tx = pool.begin().await?;
    let undo = snapshot_locked(&mut tx, set_id, user.id, "restore").await?;
    sqlx::query("UPDATE study_sets SET title = $2, kind = $3, updated_at = NOW() WHERE id = $1").bind(set_id).bind(&title).bind(&kind).execute(&mut *tx).await?;
    study::write_items(&mut tx, set_id, &items).await?;
    tx.commit().await?;
    Ok((set_id, undo))
}

async fn create_study_set_tool(pool: &PgPool, user: &AuthUser, conv_id: Uuid, args: &Value) -> ApiResult<ToolOutput> {
    let title = args["title"].as_str().ok_or_else(|| ApiError::bad_request("title is required"))?;
    let kind = args["kind"].as_str().ok_or_else(|| ApiError::bad_request("kind is required"))?;
    let ws = match args["workspace_id"].as_str().filter(|s| !s.is_empty()) {
        Some(_) => Some(uuid_arg(args, "workspace_id")?),
        None => None,
    };
    let items = parse_items(args)?;
    let id = create_study_set(pool, user, conv_id, title, kind, ws, &items).await?;
    let mut out = ToolOutput::text(json!({ "set_id": id, "url": format!("/study/{id}"), "personal": true, "items": items.len(), "note": "created as a personal entrainement; only the user can see it until they make it shared" }).to_string());
    out.study_set = Some(id);
    Ok(out)
}

async fn list_study_sets(pool: &PgPool, user: &AuthUser) -> ApiResult<ToolOutput> {
    let mut sets = study::list_for(pool, user, None).await?;
    sets.truncate(200);
    Ok(ToolOutput::text(json!({ "study_sets": sets }).to_string()))
}

async fn read_study_set(pool: &PgPool, user: &AuthUser, args: &Value) -> ApiResult<ToolOutput> {
    let id = uuid_arg(args, "set_id")?;
    let (set, acc) = study::load(pool, user, id, study::SetAccess::View).await?;
    let can_edit = acc == study::SetAccess::Edit;
    let mut items = study::items_of(pool, id).await?;
    if !can_edit {
        for it in items.iter_mut().filter(|i| i.kind != "flashcard") {
            it.answer = Value::Null;
            it.explanation = String::new();
        }
    }
    Ok(ToolOutput::text(json!({ "set_id": id, "title": set.title, "kind": set.kind, "personal": set.private_to.is_some(), "workspace_id": set.workspace_id, "can_edit": can_edit, "items": items }).to_string()))
}

async fn update_study_set_tool(pool: &PgPool, user: &AuthUser, conv_id: Uuid, args: &Value) -> ApiResult<ToolOutput> {
    let id = uuid_arg(args, "set_id")?;
    let items = parse_items(args)?;
    let rev = update_study_set(pool, user, conv_id, id, args["title"].as_str(), args["kind"].as_str(), &items).await?;
    let mut out = ToolOutput::text(format!("ok: the study set was updated (the previous version was saved, revision_id {rev}, and the user can restore it)."));
    out.study_set = Some(id);
    out.study_revision = Some(rev);
    Ok(out)
}

/// Tentatives de la personne qui discute, et de personne d'autre : toutes les requêtes filtrent sur `user.id`.
async fn read_attempts(pool: &PgPool, user: &AuthUser, args: &Value) -> ApiResult<ToolOutput> {
    if args["set_id"].as_str().filter(|s| !s.is_empty()).is_none() {
        let sets = study::list_for(pool, user, None).await?;
        let mut out = Vec::new();
        for s in sets.iter().take(100) {
            let Some(id) = s["id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) else { continue };
            let (n, last, best): (i64, Option<f64>, Option<f64>) = sqlx::query_as(
                "SELECT COUNT(*), (ARRAY_AGG(score::float8 / NULLIF(max_score, 0) ORDER BY started_at DESC))[1], MAX(score::float8 / NULLIF(max_score, 0))
                 FROM study_attempts WHERE set_id = $1 AND user_id = $2",
            )
            .bind(id)
            .bind(user.id)
            .fetch_one(pool)
            .await?;
            if n > 0 {
                out.push(json!({ "set_id": id, "title": s["title"], "attempts": n, "last_score_ratio": last, "best_score_ratio": best }));
            }
        }
        return Ok(ToolOutput::text(json!({ "your_attempts_by_set": out }).to_string()));
    }
    let id = uuid_arg(args, "set_id")?;
    study::load(pool, user, id, study::SetAccess::View).await?;
    let rows: Vec<(Uuid, chrono::DateTime<chrono::Utc>, i32, i32, Value)> = sqlx::query_as("SELECT id, started_at, score, max_score, answers FROM study_attempts WHERE set_id = $1 AND user_id = $2 ORDER BY started_at DESC LIMIT 50")
        .bind(id)
        .bind(user.id)
        .fetch_all(pool)
        .await?;
    let items = study::items_of(pool, id).await?;
    // item -> (essais, erreurs)
    let mut stats: std::collections::HashMap<Uuid, (u32, u32)> = std::collections::HashMap::new();
    for (_, _, _, _, answers) in &rows {
        let Some(m) = answers.as_object() else { continue };
        for (k, v) in m {
            let (Ok(iid), Some(correct)) = (Uuid::parse_str(k), v["correct"].as_bool()) else { continue };
            let e = stats.entry(iid).or_default();
            e.0 += 1;
            if !correct {
                e.1 += 1;
            }
        }
    }
    let mut weak: Vec<Value> = items
        .iter()
        .filter_map(|it| stats.get(&it.id).filter(|s| s.1 > 0).map(|s| (it, *s)))
        .map(|(it, (n, miss))| json!({ "item_id": it.id, "prompt": it.prompt, "tries": n, "missed": miss }))
        .collect();
    weak.sort_by(|a, b| b["missed"].as_u64().cmp(&a["missed"].as_u64()));
    weak.truncate(30);
    let attempts: Vec<Value> = rows.iter().map(|(aid, at, s, m, _)| json!({ "attempt_id": aid, "at": at, "score": s, "max": m })).collect();
    Ok(ToolOutput::text(json!({ "set_id": id, "your_attempts": attempts, "weak_items": weak }).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    struct World {
        owner: AuthUser,
        outsider: AuthUser,
        conv: Uuid,
        page: Uuid,
    }

    async fn user(pool: &PgPool, name: &str) -> AuthUser {
        let id: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id").bind(format!("{name}@example.org")).bind(name).fetch_one(pool).await.unwrap();
        AuthUser { id, is_super_admin: false }
    }

    /// Un espace dont `owner` est propriétaire et une page avec un état Yjs ; `outsider` n'en est pas membre.
    async fn world(pool: &PgPool) -> World {
        let owner = user(pool, "owner").await;
        let outsider = user(pool, "outsider").await;
        let ws: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('W') RETURNING id").fetch_one(pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')").bind(ws).bind(owner.id.to_string()).execute(pool).await.unwrap();
        let state = ai_pages::apply_markdown(&[], Op::Replace, "Départ").unwrap();
        let page: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, created_by, yjs_state) VALUES ($1, 'P', $2, $3) RETURNING id").bind(ws).bind(owner.id).bind(state).fetch_one(pool).await.unwrap();
        let conv: Uuid = sqlx::query_scalar("INSERT INTO ai_conversations (user_id) VALUES ($1) RETURNING id").bind(owner.id).fetch_one(pool).await.unwrap();
        World { owner, outsider, conv, page }
    }

    async fn grant(pool: &PgPool, w: &World, mode: &str, minutes: i32) {
        sqlx::query("INSERT INTO ai_write_grants (conversation_id, page_id, mode, expires_at) VALUES ($1, $2, $3, now() + make_interval(mins => $4))")
            .bind(w.conv)
            .bind(w.page)
            .bind(mode)
            .bind(minutes)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn md(pool: &PgPool, page: Uuid) -> String {
        page_markdown(pool, page).await.unwrap().1
    }

    #[sqlx::test]
    async fn writing_is_denied_without_a_grant(pool: PgPool) {
        let w = world(&pool).await;
        let r = write_page(&pool, &w.owner, w.conv, w.page, Op::Append, "x").await;
        assert!(matches!(r, Err(ApiError::Forbidden)));
        assert_eq!(md(&pool, w.page).await, "Départ");
    }

    #[sqlx::test]
    async fn a_grant_for_another_page_does_not_help(pool: PgPool) {
        let w = world(&pool).await;
        let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        let other: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, created_by) VALUES ($1, 'Q', $2) RETURNING id").bind(ws).bind(w.owner.id).fetch_one(&pool).await.unwrap();
        grant(&pool, &w, "replace", 30).await;
        let r = write_page(&pool, &w.owner, w.conv, other, Op::Replace, "x").await;
        assert!(matches!(r, Err(ApiError::Forbidden)));
    }

    #[sqlx::test]
    async fn the_users_own_rights_still_apply(pool: PgPool) {
        let w = world(&pool).await;
        // Une conversation d'une personne sans droit sur la page, même avec une autorisation en base.
        let conv: Uuid = sqlx::query_scalar("INSERT INTO ai_conversations (user_id) VALUES ($1) RETURNING id").bind(w.outsider.id).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO ai_write_grants (conversation_id, page_id, mode, expires_at) VALUES ($1, $2, 'replace', now() + interval '1 hour')").bind(conv).bind(w.page).execute(&pool).await.unwrap();
        let r = write_page(&pool, &w.outsider, conv, w.page, Op::Replace, "pirate").await;
        assert!(matches!(r, Err(ApiError::NotFound) | Err(ApiError::Forbidden)));
        assert_eq!(md(&pool, w.page).await, "Départ");
        // Et la conversation d'autrui n'est pas utilisable.
        let r = write_page(&pool, &w.outsider, w.conv, w.page, Op::Replace, "pirate").await;
        assert!(matches!(r, Err(ApiError::NotFound)));
    }

    #[sqlx::test]
    async fn expired_and_revoked_grants_are_refused(pool: PgPool) {
        let w = world(&pool).await;
        grant(&pool, &w, "replace", -5).await;
        assert!(matches!(write_page(&pool, &w.owner, w.conv, w.page, Op::Replace, "x").await, Err(ApiError::Forbidden)));
        grant(&pool, &w, "replace", 30).await;
        assert!(write_page(&pool, &w.owner, w.conv, w.page, Op::Replace, "ok").await.is_ok());
        sqlx::query("UPDATE ai_write_grants SET revoked_at = now() WHERE conversation_id = $1").bind(w.conv).execute(&pool).await.unwrap();
        assert!(matches!(write_page(&pool, &w.owner, w.conv, w.page, Op::Replace, "x").await, Err(ApiError::Forbidden)));
    }

    #[sqlx::test]
    async fn append_mode_refuses_replace_and_writes_a_revision_first(pool: PgPool) {
        let w = world(&pool).await;
        grant(&pool, &w, "append", 30).await;
        assert!(matches!(write_page(&pool, &w.owner, w.conv, w.page, Op::Replace, "x").await, Err(ApiError::Forbidden)));
        let before: i64 = sqlx::query_scalar("SELECT count(*) FROM page_revisions WHERE page_id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        let out = write_page(&pool, &w.owner, w.conv, w.page, Op::Append, "Ajout").await.unwrap();
        assert!(matches!(out, WriteOutcome::Applied { revision: Some(_) }));
        let after: i64 = sqlx::query_scalar("SELECT count(*) FROM page_revisions WHERE page_id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        assert_eq!(after, before + 1);
        assert_eq!(md(&pool, w.page).await, "Départ\n\nAjout");
        // La version enregistrée est bien l'état d'avant.
        let rev: Vec<u8> = sqlx::query_scalar("SELECT yjs_state FROM page_revisions WHERE page_id = $1 ORDER BY created_at DESC LIMIT 1").bind(w.page).fetch_one(&pool).await.unwrap();
        assert_eq!(ai_pages::state_to_markdown(&rev).unwrap(), "Départ");
    }

    #[sqlx::test]
    async fn suggest_mode_changes_nothing_until_accepted(pool: PgPool) {
        let w = world(&pool).await;
        grant(&pool, &w, "suggest", 30).await;
        let out = write_page(&pool, &w.owner, w.conv, w.page, Op::Replace, "Nouveau").await.unwrap();
        let WriteOutcome::Suggested { diff, .. } = out else { panic!("expected a suggestion") };
        assert!(diff.contains(&('-', "Départ".into())) && diff.contains(&('+', "Nouveau".into())));
        assert_eq!(md(&pool, w.page).await, "Départ");
        let revs: i64 = sqlx::query_scalar("SELECT count(*) FROM page_revisions WHERE page_id = $1 AND kind = 'restore'").bind(w.page).fetch_one(&pool).await.unwrap();
        assert_eq!(revs, 0);
    }

    async fn suggest(pool: &PgPool, w: &World, op: Op, md: &str) -> Uuid {
        grant(pool, w, "suggest", 30).await;
        match write_page(pool, &w.owner, w.conv, w.page, op, md).await.unwrap() {
            WriteOutcome::Suggested { suggestion_id, .. } => suggestion_id,
            _ => panic!("expected a suggestion"),
        }
    }

    #[sqlx::test]
    async fn a_suggestion_can_only_be_accepted_once(pool: PgPool) {
        let w = world(&pool).await;
        let id = suggest(&pool, &w, Op::Append, "Ajout").await;
        assert!(accept_suggestion(&pool, &w.owner, id).await.is_ok());
        assert!(matches!(accept_suggestion(&pool, &w.owner, id).await, Err(ApiError::Conflict(_))));
        assert_eq!(md(&pool, w.page).await, "Départ\n\nAjout");
        // Quelqu'un d'autre ne peut pas l'accepter non plus.
        let id2 = suggest(&pool, &w, Op::Append, "Autre").await;
        assert!(accept_suggestion(&pool, &w.outsider, id2).await.is_err());
        let status: String = sqlx::query_scalar("SELECT status FROM ai_suggestions WHERE id = $1").bind(id2).fetch_one(&pool).await.unwrap();
        assert_eq!(status, "pending");
    }

    #[sqlx::test]
    async fn a_stale_replace_suggestion_is_refused_and_stays_pending(pool: PgPool) {
        let w = world(&pool).await;
        let id = suggest(&pool, &w, Op::Replace, "Tout neuf").await;
        // La page change avant l'acceptation.
        let s = sqlx::query_scalar::<_, Vec<u8>>("SELECT yjs_state FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        let changed = ai_pages::apply_markdown(&s, Op::Append, "Modif humaine").unwrap();
        sqlx::query("UPDATE pages_all SET yjs_state = $1 WHERE id = $2").bind(changed).bind(w.page).execute(&pool).await.unwrap();
        assert!(matches!(accept_suggestion(&pool, &w.owner, id).await, Err(ApiError::Conflict(_))));
        assert_eq!(md(&pool, w.page).await, "Départ\n\nModif humaine");
        let status: String = sqlx::query_scalar("SELECT status FROM ai_suggestions WHERE id = $1").bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(status, "pending");
        let revs: i64 = sqlx::query_scalar("SELECT count(*) FROM page_revisions WHERE page_id = $1 AND kind = 'restore'").bind(w.page).fetch_one(&pool).await.unwrap();
        assert_eq!(revs, 0);
    }

    #[sqlx::test]
    async fn list_and_read_only_show_what_the_user_can_access(pool: PgPool) {
        let w = world(&pool).await;
        let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        let out = execute(&pool, &w.outsider, w.conv, "list_pages", &json!({ "workspace_id": ws })).await;
        assert!(out.text.starts_with("error"), "{}", out.text);
        let out = execute(&pool, &w.outsider, w.conv, "read_page", &json!({ "page_id": w.page })).await;
        assert!(out.text.starts_with("error"), "{}", out.text);
        let out = execute(&pool, &w.owner, w.conv, "read_page", &json!({ "page_id": w.page })).await;
        assert!(out.text.contains("Départ"));
        let out = execute(&pool, &w.owner, w.conv, "list_pages", &json!({ "workspace_id": ws })).await;
        assert!(out.text.contains(&w.page.to_string()));
    }

    async fn legacy_page(pool: &PgPool, w: &World, html: &str) -> Uuid {
        let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(w.page).fetch_one(pool).await.unwrap();
        let page: Uuid = sqlx::query_scalar("INSERT INTO pages (workspace_id, title, created_by, content) VALUES ($1, 'Vieille', $2, $3) RETURNING id").bind(ws).bind(w.owner.id).bind(html).fetch_one(pool).await.unwrap();
        sqlx::query("INSERT INTO ai_write_grants (conversation_id, page_id, mode, expires_at) VALUES ($1, $2, 'replace', now() + interval '1 hour')").bind(w.conv).bind(page).execute(pool).await.unwrap();
        page
    }

    #[sqlx::test]
    async fn a_page_with_only_legacy_html_is_converted_snapshotted_and_written(pool: PgPool) {
        let w = world(&pool).await;
        let page = legacy_page(&pool, &w, "<h2>Cours</h2><p>Du <strong>gras</strong> et <a href=\"https://example.org\">un lien</a>.</p><ul><li><p>un</p></li><li><p>deux</p></li></ul>").await;
        assert_eq!(md(&pool, page).await, "## Cours\n\nDu **gras** et [un lien](https://example.org).\n\n- un\n- deux");
        let out = write_page(&pool, &w.owner, w.conv, page, Op::Append, "Fin").await.unwrap();
        let WriteOutcome::Applied { revision: Some(rev) } = out else { panic!("expected a revision") };
        // L'instantané est le texte d'origine, converti ; la page a maintenant un état Yjs et un HTML à jour.
        let snap: Vec<u8> = sqlx::query_scalar("SELECT yjs_state FROM page_revisions WHERE id = $1").bind(rev).fetch_one(&pool).await.unwrap();
        assert!(ai_pages::state_to_markdown(&snap).unwrap().starts_with("## Cours"));
        assert!(md(&pool, page).await.ends_with("- deux\n\nFin"));
        let content: String = sqlx::query_scalar("SELECT content FROM pages WHERE id = $1").bind(page).fetch_one(&pool).await.unwrap();
        assert!(content.contains("<h2>Cours</h2>") && content.contains("<strong>gras</strong>") && content.contains("<p>Fin</p>"), "{content}");
    }

    #[sqlx::test]
    async fn legacy_html_with_unsupported_blocks_is_still_refused(pool: PgPool) {
        let w = world(&pool).await;
        for html in ["<p>Avant</p><img src=\"/uploads/a.png\">", "<table><tbody><tr><td><p>x</p></td></tr></tbody></table>", "<p>a<br>b</p>"] {
            let page = legacy_page(&pool, &w, html).await;
            let r = write_page(&pool, &w.owner, w.conv, page, Op::Replace, "Neuf").await;
            assert!(matches!(&r, Err(ApiError::Conflict(m)) if m.starts_with("page_not_convertible")), "{html}");
            let (state, content): (Option<Vec<u8>>, String) = sqlx::query_as("SELECT yjs_state, content FROM pages WHERE id = $1").bind(page).fetch_one(&pool).await.unwrap();
            assert!(state.is_none());
            assert_eq!(content, html);
        }
    }

    #[sqlx::test]
    async fn the_html_copy_is_left_alone_when_it_cannot_represent_the_document(pool: PgPool) {
        let w = world(&pool).await;
        grant(&pool, &w, "replace", 30).await;
        let state = ai_pages::tests::state_with_unknown_block();
        sqlx::query("UPDATE pages SET yjs_state = $2, content = '<div data-type=\"excalidraw\"></div>' WHERE id = $1").bind(w.page).bind(state).execute(&pool).await.unwrap();
        write_page(&pool, &w.owner, w.conv, w.page, Op::Append, "Ajout").await.unwrap();
        let content: String = sqlx::query_scalar("SELECT content FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        assert_eq!(content, "<div data-type=\"excalidraw\"></div>");
        assert!(md(&pool, w.page).await.ends_with("Ajout"));
    }

    #[sqlx::test]
    async fn the_html_copy_follows_every_write_and_accepted_suggestion(pool: PgPool) {
        let w = world(&pool).await;
        grant(&pool, &w, "replace", 30).await;
        write_page(&pool, &w.owner, w.conv, w.page, Op::Replace, "# Titre\n\nTexte <b> & `code`\n\n- [x] fait\n\n$$\nx^2\n$$").await.unwrap();
        let content: String = sqlx::query_scalar("SELECT content FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        assert!(content.starts_with("<h1>Titre</h1><p>Texte &lt;b&gt; &amp; <code>code</code></p>"), "{content}");
        assert!(content.contains("data-checked=\"true\"") && content.contains("<div data-type=\"math-block\" latex=\"x^2\"></div>"), "{content}");
        // Aller-retour : le HTML produit se reconvertit en le même Markdown (sauf tâches, non converties volontairement).
        let plain = write_page(&pool, &w.owner, w.conv, w.page, Op::Replace, "Un\n\n1. a\n2. b").await;
        assert!(plain.is_ok());
        let content: String = sqlx::query_scalar("SELECT content FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        assert_eq!(content, "<p>Un</p><ol><li><p>a</p></li><li><p>b</p></li></ol>");
    }

    // --- Jeux de révision ---

    fn qcm_args(ws: Option<Uuid>) -> Value {
        let mut v = json!({
            "title": "Réseaux", "kind": "qcm",
            "items": [ { "type": "single", "prompt": "TCP ?", "choices": [ { "id": "a", "text": "Connexion" }, { "id": "b", "text": "Sans" } ], "answer": { "choice": "a" }, "explanation": "Poignée de main." } ]
        });
        if let Some(ws) = ws {
            v["workspace_id"] = json!(ws);
        }
        v
    }

    async fn ws_of(pool: &PgPool, w: &World) -> Uuid {
        sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(w.page).fetch_one(pool).await.unwrap()
    }

    async fn study_grant(pool: &PgPool, conv: Uuid, ws: Option<Uuid>, set: Option<Uuid>, minutes: i32) {
        let (kind, personal) = if set.is_some() { ("update", false) } else { ("create", ws.is_none()) };
        sqlx::query("INSERT INTO ai_study_grants (conversation_id, kind, workspace_id, personal, set_id, expires_at) VALUES ($1, $2, $3, $4, $5, now() + make_interval(mins => $6))")
            .bind(conv)
            .bind(kind)
            .bind(ws)
            .bind(personal)
            .bind(set)
            .bind(minutes)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn count_sets(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM study_sets").fetch_one(pool).await.unwrap()
    }

    #[sqlx::test]
    async fn creating_a_study_set_needs_a_matching_live_grant(pool: PgPool) {
        let w = world(&pool).await;
        let ws = ws_of(&pool, &w).await;
        // Sans autorisation.
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(Some(ws))).await;
        assert!(out.text.starts_with("error: forbidden"), "{}", out.text);
        // Autorisation pour un autre espace, ou pour l'espace perso seulement.
        let other: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('Autre') RETURNING id").fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')").bind(other).bind(w.owner.id.to_string()).execute(&pool).await.unwrap();
        study_grant(&pool, w.conv, Some(other), None, 30).await;
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(Some(ws))).await;
        assert!(out.text.starts_with("error: forbidden"), "{}", out.text);
        study_grant(&pool, w.conv, None, None, 30).await;
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(Some(ws))).await;
        assert!(out.text.starts_with("error: forbidden"), "{}", out.text);
        // Autorisation expirée pour le bon espace.
        study_grant(&pool, w.conv, Some(ws), None, -5).await;
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(Some(ws))).await;
        assert!(out.text.starts_with("error: forbidden"), "{}", out.text);
        assert_eq!(count_sets(&pool).await, 0);
    }

    #[sqlx::test]
    async fn a_created_set_is_private_and_owned_by_the_user(pool: PgPool) {
        let w = world(&pool).await;
        let ws = ws_of(&pool, &w).await;
        study_grant(&pool, w.conv, Some(ws), None, 30).await;
        study_grant(&pool, w.conv, None, None, 30).await;
        for args in [qcm_args(Some(ws)), qcm_args(None)] {
            let out = execute(&pool, &w.owner, w.conv, "create_study_set", &args).await;
            assert!(out.study_set.is_some(), "{}", out.text);
        }
        let rows: Vec<(Uuid, Option<Uuid>, Option<Uuid>)> = sqlx::query_as("SELECT owner_id, private_to, parent_id FROM study_sets").fetch_all(&pool).await.unwrap();
        assert_eq!(rows.len(), 2);
        // Personnels (private_to = l'autrice), à la racine ; sans espace précisé : dans l'espace perso.
        assert!(rows.iter().all(|r| r.0 == w.owner.id && r.1 == Some(w.owner.id) && r.2.is_none()));
        let in_personal: i64 = sqlx::query_scalar("SELECT count(*) FROM study_sets s JOIN workspaces w ON w.id = s.workspace_id WHERE w.personal_of = $1").bind(w.owner.id).fetch_one(&pool).await.unwrap();
        assert_eq!(in_personal, 1);
        // Un autre membre ne le voit pas ; sa liste est vide.
        let out = execute(&pool, &w.outsider, w.conv, "list_study_sets", &json!({})).await;
        assert!(!out.text.contains("Réseaux"), "{}", out.text);
        let id = out.study_set;
        assert!(id.is_none());
    }

    #[sqlx::test]
    async fn a_user_without_edit_rights_cannot_create_even_with_a_grant(pool: PgPool) {
        let w = world(&pool).await;
        let ws = ws_of(&pool, &w).await;
        // Lecteur de l'espace : peut lire les documents, pas les modifier.
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'viewer')").bind(ws).bind(w.outsider.id.to_string()).execute(&pool).await.unwrap();
        let conv: Uuid = sqlx::query_scalar("INSERT INTO ai_conversations (user_id) VALUES ($1) RETURNING id").bind(w.outsider.id).fetch_one(&pool).await.unwrap();
        study_grant(&pool, conv, Some(ws), None, 30).await;
        let out = execute(&pool, &w.outsider, conv, "create_study_set", &qcm_args(Some(ws))).await;
        assert!(out.text.starts_with("error"), "{}", out.text);
        assert_eq!(count_sets(&pool).await, 0);
        // Et la conversation d'autrui n'est pas utilisable.
        study_grant(&pool, w.conv, Some(ws), None, 30).await;
        let out = execute(&pool, &w.outsider, w.conv, "create_study_set", &qcm_args(None)).await;
        assert!(out.text.starts_with("error"), "{}", out.text);
        assert_eq!(count_sets(&pool).await, 0);
    }

    #[sqlx::test]
    async fn invalid_items_are_rejected_and_nothing_is_created(pool: PgPool) {
        let w = world(&pool).await;
        study_grant(&pool, w.conv, None, None, 30).await;
        let bad_answer = json!({ "title": "T", "kind": "qcm", "items": [ { "type": "single", "prompt": "?", "choices": [ { "id": "a", "text": "x" }, { "id": "b", "text": "y" } ], "answer": { "choice": "z" } } ] });
        let wrong_type = json!({ "title": "T", "kind": "qcm", "items": [ { "type": "flashcard", "prompt": "?", "answer": { "back": "x" } } ] });
        let unknown_field = json!({ "title": "T", "kind": "qcm", "items": [ { "type": "text", "prompt": "?", "answer": {}, "bogus": 1 } ] });
        let bad_kind = json!({ "title": "T", "kind": "quiz", "items": [] });
        for a in [bad_answer, wrong_type, unknown_field, bad_kind] {
            let out = execute(&pool, &w.owner, w.conv, "create_study_set", &a).await;
            assert!(out.text.starts_with("error"), "{}", out.text);
        }
        assert_eq!(count_sets(&pool).await, 0);
    }

    async fn make_set(pool: &PgPool, w: &World) -> Uuid {
        study_grant(pool, w.conv, None, None, 30).await;
        execute(pool, &w.owner, w.conv, "create_study_set", &qcm_args(None)).await.study_set.unwrap()
    }

    #[sqlx::test]
    async fn updating_needs_a_grant_on_that_set_and_keeps_an_undo_version(pool: PgPool) {
        let w = world(&pool).await;
        let set = make_set(&pool, &w).await;
        let args = json!({ "set_id": set, "title": "Nouveau", "items": [ { "type": "single", "prompt": "UDP ?", "choices": [ { "id": "a", "text": "x" }, { "id": "b", "text": "y" } ], "answer": { "choice": "b" } } ] });
        // Une autorisation de création ne permet pas de modifier.
        let out = execute(&pool, &w.owner, w.conv, "update_study_set", &args).await;
        assert!(out.text.starts_with("error: forbidden"), "{}", out.text);
        // Autorisation sur un autre jeu : refusée.
        let other = make_set(&pool, &w).await;
        study_grant(&pool, w.conv, None, Some(other), 30).await;
        let out = execute(&pool, &w.owner, w.conv, "update_study_set", &args).await;
        assert!(out.text.starts_with("error: forbidden"), "{}", out.text);
        // Avec la bonne autorisation : modifié, ancienne version gardée, puis annulation.
        study_grant(&pool, w.conv, None, Some(set), 30).await;
        let out = execute(&pool, &w.owner, w.conv, "update_study_set", &args).await;
        assert!(out.text.starts_with("ok"), "{}", out.text);
        let title: String = sqlx::query_scalar("SELECT title FROM study_sets WHERE id = $1").bind(set).fetch_one(&pool).await.unwrap();
        assert_eq!(title, "Nouveau");
        let rev: Uuid = sqlx::query_scalar("SELECT id FROM study_set_revisions WHERE set_id = $1").bind(set).fetch_one(&pool).await.unwrap();
        // L'identifiant renvoyé à l'interface (bouton « restaurer ») est cette version.
        assert_eq!(out.study_revision, Some(rev));
        assert!(restore_study_revision(&pool, &w.outsider, rev).await.is_err());
        restore_study_revision(&pool, &w.owner, rev).await.unwrap();
        let (title, prompt): (String, String) = sqlx::query_as("SELECT s.title, i.prompt FROM study_sets s JOIN study_items i ON i.set_id = s.id WHERE s.id = $1").bind(set).fetch_one(&pool).await.unwrap();
        assert_eq!((title.as_str(), prompt.as_str()), ("Réseaux", "TCP ?"));
    }

    #[sqlx::test]
    async fn restoring_is_undoable_and_empty_updates_are_refused(pool: PgPool) {
        let w = world(&pool).await;
        let set = make_set(&pool, &w).await;
        study_grant(&pool, w.conv, None, Some(set), 30).await;
        // Liste vide : refusée, le jeu reste intact et aucune version n'est créée.
        let out = execute(&pool, &w.owner, w.conv, "update_study_set", &json!({ "set_id": set, "items": [] })).await;
        assert!(out.text.starts_with("error"), "{}", out.text);
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM study_items WHERE set_id = $1").bind(set).fetch_one(&pool).await.unwrap();
        let revs: i64 = sqlx::query_scalar("SELECT count(*) FROM study_set_revisions WHERE set_id = $1").bind(set).fetch_one(&pool).await.unwrap();
        assert_eq!((n, revs), (1, 0));
        // Mise à jour puis restauration : la restauration garde l'état remplacé, qu'on peut remettre.
        let args = json!({ "set_id": set, "title": "Nouveau", "items": [ { "type": "single", "prompt": "UDP ?", "choices": [ { "id": "a", "text": "x" }, { "id": "b", "text": "y" } ], "answer": { "choice": "b" } } ] });
        assert!(execute(&pool, &w.owner, w.conv, "update_study_set", &args).await.text.starts_with("ok"));
        let rev: Uuid = sqlx::query_scalar("SELECT id FROM study_set_revisions WHERE set_id = $1").bind(set).fetch_one(&pool).await.unwrap();
        let (_, undo) = restore_study_revision(&pool, &w.owner, rev).await.unwrap();
        let title: String = sqlx::query_scalar("SELECT title FROM study_sets WHERE id = $1").bind(set).fetch_one(&pool).await.unwrap();
        assert_eq!(title, "Réseaux");
        restore_study_revision(&pool, &w.owner, undo).await.unwrap();
        let (title, prompt): (String, String) = sqlx::query_as("SELECT s.title, i.prompt FROM study_sets s JOIN study_items i ON i.set_id = s.id WHERE s.id = $1").bind(set).fetch_one(&pool).await.unwrap();
        assert_eq!((title.as_str(), prompt.as_str()), ("Nouveau", "UDP ?"));
    }

    #[sqlx::test]
    async fn listing_revisions_is_for_editors_only(pool: PgPool) {
        let w = world(&pool).await;
        let ws = ws_of(&pool, &w).await;
        let set = make_set(&pool, &w).await;
        study_grant(&pool, w.conv, None, Some(set), 30).await;
        let args = json!({ "set_id": set, "title": "Nouveau", "items": [ { "type": "single", "prompt": "UDP ?", "choices": [ { "id": "a", "text": "x" }, { "id": "b", "text": "y" } ], "answer": { "choice": "b" } } ] });
        assert!(execute(&pool, &w.owner, w.conv, "update_study_set", &args).await.text.starts_with("ok"));
        let rev: Uuid = sqlx::query_scalar("SELECT id FROM study_set_revisions WHERE set_id = $1").bind(set).fetch_one(&pool).await.unwrap();
        restore_study_revision(&pool, &w.owner, rev).await.unwrap();
        let list = study::revisions_of(&pool, &w.owner, set).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["source"], "restore");
        assert_eq!(list[1]["source"], "ai");
        assert_eq!(list[1]["item_count"], 1);
        // Un viewer de l'espace voit le jeu mais pas ses versions ; un tiers ne sait même pas qu'il existe.
        let viewer = user(&pool, "viewer").await;
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'viewer')").bind(ws).bind(viewer.id.to_string()).execute(&pool).await.unwrap();
        sqlx::query("UPDATE study_sets SET workspace_id = $2, private_to = NULL WHERE id = $1").bind(set).bind(ws).execute(&pool).await.unwrap();
        assert!(study::revisions_of(&pool, &viewer, set).await.is_err());
        assert!(study::revisions_of(&pool, &w.outsider, set).await.is_err());
    }

    #[sqlx::test]
    async fn another_user_cannot_read_or_update_a_private_set(pool: PgPool) {
        let w = world(&pool).await;
        let set = make_set(&pool, &w).await;
        let conv: Uuid = sqlx::query_scalar("INSERT INTO ai_conversations (user_id) VALUES ($1) RETURNING id").bind(w.outsider.id).fetch_one(&pool).await.unwrap();
        study_grant(&pool, conv, None, Some(set), 30).await;
        let out = execute(&pool, &w.outsider, conv, "read_study_set", &json!({ "set_id": set })).await;
        assert!(out.text.starts_with("error"), "{}", out.text);
        let out = execute(&pool, &w.outsider, conv, "update_study_set", &json!({ "set_id": set, "items": [] })).await;
        assert!(out.text.starts_with("error"), "{}", out.text);
        let out = execute(&pool, &w.outsider, conv, "read_attempts", &json!({ "set_id": set })).await;
        assert!(out.text.starts_with("error"), "{}", out.text);
    }

    #[sqlx::test]
    async fn answers_are_only_shown_to_editors(pool: PgPool) {
        let w = world(&pool).await;
        let ws = ws_of(&pool, &w).await;
        let set = make_set(&pool, &w).await;
        sqlx::query("UPDATE study_sets SET workspace_id = $2, private_to = NULL WHERE id = $1").bind(set).bind(ws).execute(&pool).await.unwrap();
        let out = execute(&pool, &w.owner, w.conv, "read_study_set", &json!({ "set_id": set })).await;
        assert!(out.text.contains("Poignée de main"), "{}", out.text);
        // Lecteur : voit le jeu, pas les réponses.
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'viewer')").bind(ws).bind(w.outsider.id.to_string()).execute(&pool).await.unwrap();
        let conv: Uuid = sqlx::query_scalar("INSERT INTO ai_conversations (user_id) VALUES ($1) RETURNING id").bind(w.outsider.id).fetch_one(&pool).await.unwrap();
        let out = execute(&pool, &w.outsider, conv, "read_study_set", &json!({ "set_id": set })).await;
        assert!(out.text.contains("TCP ?") && !out.text.contains("Poignée de main") && !out.text.contains("\"choice\""), "{}", out.text);
    }

    #[sqlx::test]
    async fn read_attempts_only_returns_the_callers_own(pool: PgPool) {
        let w = world(&pool).await;
        let ws = ws_of(&pool, &w).await;
        let set = make_set(&pool, &w).await;
        sqlx::query("UPDATE study_sets SET workspace_id = $2, private_to = NULL WHERE id = $1").bind(set).bind(ws).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'viewer')").bind(ws).bind(w.outsider.id.to_string()).execute(&pool).await.unwrap();
        let item: Uuid = sqlx::query_scalar("SELECT id FROM study_items WHERE set_id = $1").bind(set).fetch_one(&pool).await.unwrap();
        for (u, score, tag) in [(w.owner.id, 1, "MINE"), (w.outsider.id, 0, "THEIRS")] {
            let answers = json!({ item.to_string(): { "given": tag, "correct": score == 1 } });
            sqlx::query("INSERT INTO study_attempts (set_id, user_id, score, max_score, answers) VALUES ($1, $2, $3, 1, $4)").bind(set).bind(u).bind(score).bind(answers).execute(&pool).await.unwrap();
        }
        let conv: Uuid = sqlx::query_scalar("INSERT INTO ai_conversations (user_id) VALUES ($1) RETURNING id").bind(w.outsider.id).fetch_one(&pool).await.unwrap();
        // Le propriétaire du jeu ne voit pas la tentative de l'autre personne, et inversement.
        for (u, c, mine, theirs) in [(&w.owner, w.conv, 1, 0), (&w.outsider, conv, 0, 1)] {
            let by_set = execute(&pool, u, c, "read_attempts", &json!({})).await;
            let v: Value = serde_json::from_str(&by_set.text).unwrap();
            assert_eq!(v["your_attempts_by_set"][0]["attempts"], 1, "{}", by_set.text);
            let one = execute(&pool, u, c, "read_attempts", &json!({ "set_id": set })).await;
            let v: Value = serde_json::from_str(&one.text).unwrap();
            assert_eq!(v["your_attempts"].as_array().unwrap().len(), 1, "{}", one.text);
            assert_eq!(v["your_attempts"][0]["score"], mine, "{}", one.text);
            assert_eq!(v["weak_items"].as_array().unwrap().len(), theirs, "{}", one.text);
            assert!(!one.text.contains("MINE") && !one.text.contains("THEIRS"));
        }
    }
}
