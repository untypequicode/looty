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
use crate::ai_files;
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
        ToolSpec {
            name: "read_item",
            description: "Read a task, a board, a calendar event or a project (read-only, with the user's rights). Use it for items the user attached or mentioned; pages and study sets have their own tools.",
            parameters: json!({
                "type": "object",
                "properties": { "kind": { "type": "string", "enum": ["task", "board", "event", "project"] }, "id": { "type": "string" } },
                "required": ["kind", "id"]
            }),
        },
    ];
    v.extend(file_specs());
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
    /// Écriture refusée faute d'autorisation : l'interface montre une carte « Autoriser » dans la discussion.
    pub grant_needed: Option<Value>,
    /// Version de la page gardée avant une écriture directe : l'interface propose de l'annuler.
    pub page_revision: Option<Uuid>,
}

impl ToolOutput {
    pub(crate) fn text(t: impl Into<String>) -> Self {
        ToolOutput { text: t.into(), suggestion: None, changed_page: None, study_set: None, study_revision: None, grant_needed: None, page_revision: None }
    }
}

/// Message d'erreur renvoyé au modèle (sans détail interne).
fn error_text(e: &ApiError) -> String {
    match e {
        ApiError::Forbidden => "error: forbidden. Either no write grant is active for this page in this conversation (an authorization card was shown to the user in the chat: tell them to approve it, then retry only when they say so), the grant mode does not allow this operation, or the user has no edit right on the page or study set. For study sets: ask the user to grant 'create study sets' in that workspace (or personal space), or an update grant on that set.".into(),
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
        "read_item" => read_item_tool(pool, user, args).await,
        "read_attachment" => ai_files::read_attachment(pool, user, conv_id, args).await.map(ToolOutput::text),
        "search_attachment" => ai_files::search_attachment(pool, user, conv_id, args).await.map(ToolOutput::text),
        "create_study_set" => create_study_set_tool(pool, user, conv_id, args).await,
        "list_study_sets" => list_study_sets(pool, user).await,
        "read_study_set" => read_study_set(pool, user, args).await,
        "update_study_set" => update_study_set_tool(pool, user, conv_id, args).await,
        "read_attempts" => read_attempts(pool, user, args).await,
        other => Ok(ToolOutput::text(format!("error: unknown tool {other}"))),
    };
    match r {
        Ok(o) => o,
        Err(e) => {
            let mut o = ToolOutput::text(error_text(&e));
            if matches!(e, ApiError::Forbidden) && (name == "create_study_set" || name == "update_study_set") {
                o.text = forbidden_study_text(pool, conv_id).await;
            }
            if matches!(e, ApiError::Forbidden) {
                o.grant_needed = grant_card(pool, user, conv_id, name, args).await;
            }
            o
        }
    }
}

/// Carte d'autorisation à proposer après un refus : seulement si la personne a elle-même le droit
/// d'écrire là (sinon l'autorisation serait refusée aussi). Ne contient que des titres et des identifiants.
async fn grant_card(pool: &PgPool, user: &AuthUser, conv_id: Uuid, name: &str, args: &Value) -> Option<Value> {
    match name {
        "write_page" => {
            let page_id = uuid_arg(args, "page_id").ok()?;
            let (ws, _, access) = require_page_read(pool, user, page_id).await.ok()?;
            if access != PageAccess::Write {
                return None;
            }
            let title: String = sqlx::query_scalar("SELECT title FROM pages WHERE id = $1").bind(page_id).fetch_optional(pool).await.ok()??;
            Some(json!({ "kind": "page", "page_id": page_id, "workspace_id": ws, "title": title, "operation": args["operation"].as_str().unwrap_or("append") }))
        }
        "create_study_set" => {
            let ws = study_target(pool, user, conv_id, args).await.ok()?;
            if let Some(ws) = ws {
                require_perm(pool, user, ws, |p| p.docs >= level::EDIT).await.ok()?;
            }
            Some(json!({ "kind": "study_create", "workspace_id": ws, "personal": ws.is_none(), "title": args["title"].as_str().unwrap_or("") }))
        }
        "update_study_set" => {
            let set_id = uuid_arg(args, "set_id").ok()?;
            let (set, _) = study::load(pool, user, set_id, study::SetAccess::Edit).await.ok()?;
            Some(json!({ "kind": "study_update", "set_id": set_id, "title": set.title }))
        }
        _ => None,
    }
}

/// Annule une écriture directe de l'IA sur une page : remet l'état gardé avant. L'état remplacé est
/// lui-même gardé (l'annulation s'annule aussi). Réservé à qui peut écrire sur la page.
pub async fn restore_page_revision(pool: &PgPool, user: &AuthUser, rev_id: Uuid) -> ApiResult<(Uuid, Option<Uuid>)> {
    let (page_id, state): (Uuid, Vec<u8>) = sqlx::query_as("SELECT page_id, yjs_state FROM page_revisions WHERE id = $1").bind(rev_id).fetch_optional(pool).await?.ok_or(ApiError::NotFound)?;
    require_page_write(pool, user, page_id).await?;
    let mut tx = pool.begin().await?;
    let row: (Option<Vec<u8>>, Option<String>) = sqlx::query_as("SELECT yjs_state, content FROM pages WHERE id = $1 FOR UPDATE").bind(page_id).fetch_optional(&mut *tx).await?.ok_or(ApiError::NotFound)?;
    let undo = match row.0.as_deref().filter(|s| !s.is_empty()) {
        Some(cur) => Some(crate::revisions::snapshot_before_write(&mut tx, page_id, user.id, "Avant annulation d'une modification de l'IA", cur).await?),
        None => None,
    };
    let html = ai_pages::state_to_html(&state).ok().or(row.1);
    sqlx::query("UPDATE pages_all SET yjs_state = $1, content = $2, updated_at = NOW(), updated_by = $3 WHERE id = $4").bind(&state).bind(&html).bind(user.id).bind(page_id).execute(&mut *tx).await?;
    tx.commit().await?;
    let _ = sqlx::query("SELECT pg_notify('looty_page_reset', $1)").bind(page_id.to_string()).execute(pool).await;
    Ok((page_id, undo))
}

fn uuid_arg(args: &Value, key: &str) -> ApiResult<Uuid> {
    args[key].as_str().and_then(|s| Uuid::parse_str(s.trim()).ok()).ok_or_else(|| ApiError::bad_request(format!("{key} must be a UUID")))
}

/// Cible d'un entraînement créé par l'IA : `None` = espace personnel. Un identifiant donné par le modèle
/// l'emporte (l'identifiant de l'espace perso vaut « personnel ») ; sinon l'espace de la conversation
/// (page, projet ou espace en contexte), puis l'espace courant de la personne, puis l'espace perso.
async fn study_target(pool: &PgPool, user: &AuthUser, conv_id: Uuid, args: &Value) -> ApiResult<Option<Uuid>> {
    let ws = match args["workspace_id"].as_str().filter(|s| !s.is_empty()) {
        Some(_) => Some(uuid_arg(args, "workspace_id")?),
        None => match context_workspace(pool, conv_id).await? {
            Some(w) => Some(w),
            None => sqlx::query_scalar::<_, Option<Uuid>>("SELECT default_workspace_id FROM ai_conversations WHERE id = $1").bind(conv_id).fetch_optional(pool).await?.flatten(),
        },
    };
    Ok(match ws {
        Some(w) if w == crate::personal::personal_space(pool, user.id).await? => None,
        other => other,
    })
}

/// Refus d'écrire un jeu : dit au modèle quelles autorisations sont actives pour qu'il se corrige.
async fn forbidden_study_text(pool: &PgPool, conv_id: Uuid) -> String {
    type Row = (String, Option<Uuid>, bool, Option<String>, Option<Uuid>, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT g.kind, g.workspace_id, g.personal, w.name, g.set_id, s.title FROM ai_study_grants g
           LEFT JOIN workspaces w ON w.id = g.workspace_id LEFT JOIN study_sets s ON s.id = g.set_id
          WHERE g.conversation_id = $1 AND g.revoked_at IS NULL AND g.expires_at > now() ORDER BY g.created_at",
    )
    .bind(conv_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let active: Vec<String> = rows
        .into_iter()
        .map(|(kind, ws, personal, ws_name, set, title)| match (kind.as_str(), ws, set) {
            ("update", _, Some(id)) => format!("update of study set \"{}\" (set_id {id})", title.unwrap_or_default()),
            (_, Some(id), _) => format!("create in workspace \"{}\" (workspace_id {id})", ws_name.unwrap_or_default()),
            _ if personal => "create in the personal space (omit workspace_id)".to_string(),
            _ => "create".to_string(),
        })
        .collect();
    let listed = if active.is_empty() { "none".to_string() } else { active.join("; ") };
    format!(
        "error: forbidden. No active authorization matches this exact target in this conversation. Active authorizations: {listed}. Do not change the target between attempts: either retry with exactly one of the active targets above, or (if none fits) tell the user an authorization card was shown in the chat, ask them to approve it, and call this tool again only after they say they did. It may also have expired, or the user may lack edit rights there."
    )
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

/// Titre d'une page tirée d'une réponse : premier titre Markdown, sinon première ligne de texte.
pub fn title_from_markdown(md: &str) -> String {
    let line = md
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with('#') && l.trim_start_matches('#').starts_with(' '))
        .or_else(|| md.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("```") && !l.starts_with(":::") && !l.starts_with("$$")))
        .unwrap_or("");
    let plain: String = line.trim_start_matches('#').chars().filter(|c| !matches!(c, '*' | '_' | '`' | '>' | '$')).collect();
    let t: String = plain.trim().chars().take(80).collect();
    if t.is_empty() { "Réponse de l'IA".into() } else { t }
}

/// Markdown → (état Yjs, copie HTML). Le texte vient d'un modèle : la conversion tourne hors de la
/// tâche de la requête, et une panique du convertisseur devient une erreur propre, jamais un plantage.
pub async fn markdown_to_state(md: String) -> ApiResult<(Vec<u8>, Option<String>)> {
    let job = tokio::task::spawn_blocking(move || {
        let state = ai_pages::apply_markdown(&[], Op::Replace, &md)?;
        let html = ai_pages::state_to_html(&state).ok();
        Ok::<_, String>((state, html))
    });
    match job.await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(ApiError::Internal(format!("yjs write: {e}"))),
        Err(_) => Err(ApiError::Internal("markdown_conversion_failed".into())),
    }
}

/// « Créer une page avec cette réponse » : la personne (pas l'IA) crée une page à partir d'un
/// message de l'assistant de l'une de ses conversations. Il faut le droit d'éditer les documents de
/// l'espace cible (aucune autorisation d'écriture IA : c'est la personne qui agit). Le Markdown passe
/// par le convertisseur des pages ; ce qu'il ne sait pas écrire reste en blocs de code ou en texte.
/// `workspace` par défaut : celui de la page de contexte de la conversation. Renvoie (page, espace).
pub async fn page_from_message(pool: &PgPool, user: &AuthUser, message_id: Uuid, workspace: Option<Uuid>, personal: bool) -> ApiResult<(Uuid, Uuid)> {
    let row: Option<(String, String, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT m.role, m.content, c.context_kind, c.context_id FROM ai_messages m JOIN ai_conversations c ON c.id = m.conversation_id WHERE m.id = $1 AND c.user_id = $2",
    )
    .bind(message_id)
    .bind(user.id)
    .fetch_optional(pool)
    .await?;
    let (role, content, ctx_kind, ctx_id) = row.ok_or(ApiError::NotFound)?;
    if role != "assistant" || content.trim().is_empty() {
        return Err(ApiError::NotFound);
    }
    let ws = match (workspace, ctx_kind.as_str(), ctx_id) {
        (Some(w), _, _) => w,
        (None, "page", Some(p)) => sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(p).fetch_optional(pool).await?.ok_or(ApiError::NotFound)?,
        _ => return Err(ApiError::bad_request("workspace_required")),
    };
    let can_edit = crate::access::member_perms(pool, user.id, ws).await?.is_some_and(|(_, p)| p.docs >= level::EDIT);
    if !can_edit {
        return Err(ApiError::Forbidden);
    }
    let state = ai_pages::apply_markdown(&[], Op::Replace, &content).map_err(|e| ApiError::Internal(format!("yjs write: {e}")))?;
    let html = ai_pages::state_to_html(&state).ok();
    let title = title_from_markdown(&content);
    let position: f64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 65536 FROM pages WHERE workspace_id = $1 AND parent_id IS NULL").bind(ws).fetch_one(pool).await?;
    let mut tx = pool.begin().await?;
    let page: Uuid = sqlx::query_scalar(
        "INSERT INTO pages (workspace_id, parent_id, title, content, yjs_state, created_by, updated_by, position) VALUES ($1, NULL, $2, $3, $4, $5, $5, $6) RETURNING id",
    )
    .bind(ws)
    .bind(&title)
    .bind(&html)
    .bind(&state)
    .bind(user.id)
    .bind(position)
    .fetch_one(&mut *tx)
    .await?;
    if personal {
        sqlx::query("UPDATE pages_all SET private_to = $2 WHERE id = $1").bind(page).bind(user.id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok((page, ws))
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
        WriteOutcome::Applied { revision } => ToolOutput { text: "ok: the page was updated (a restorable version was saved before).".into(), suggestion: None, changed_page: Some(page_id), study_set: None, study_revision: None, grant_needed: None, page_revision: revision },
        WriteOutcome::Suggested { suggestion_id, diff } => ToolOutput {
            text: "ok: the change was NOT applied; it was proposed to the user, who must accept it in the chat. Do not claim the page changed.".into(),
            suggestion: Some(json!({ "id": suggestion_id, "page_id": page_id, "diff": diff.iter().map(|(o, l)| json!({ "op": o.to_string(), "text": l })).collect::<Vec<_>>() })),
            changed_page: None,
            study_set: None,
            study_revision: None,
            grant_needed: None,
            page_revision: None,
        },
    })
}

// ---------------------------------------------------------------------------
// Jeux de révision (docs/study-items.md, ADR 0010 addendum)
// ---------------------------------------------------------------------------

/// Lecture des fichiers joints (ADR 0012) : texte extrait par le serveur, budget par conversation.
fn file_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "read_attachment",
            description: "Read the text of a file the user attached (PDF, Word, PowerPoint, text, code) page by page. `id` is the file id shown in the attached item. Pass `page`, or `start` and `end` for several pages (page 1 by default). One call returns at most about 12,000 characters and counts against a reading budget for the conversation; the reply says where to continue. Use the outline in the attached summary to pick pages, or search_attachment to find them.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Attached file id" },
                    "page": { "type": "integer", "description": "Page (or slide) number, from 1" },
                    "start": { "type": "integer" },
                    "end": { "type": "integer" }
                },
                "required": ["id"]
            }),
        },
        ToolSpec {
            name: "search_attachment",
            description: "Search keywords in the attached files of this conversation (or in one file with `id`). Returns the best pages with the matching lines, accents and case ignored. It is a keyword search, not a semantic one: try synonyms and word variants. Then read the pages with read_attachment.",
            parameters: json!({
                "type": "object",
                "properties": { "query": { "type": "string", "description": "A few keywords" }, "id": { "type": "string", "description": "Restrict to one attached file" } },
                "required": ["query"]
            }),
        },
    ]
}

fn study_specs() -> Vec<ToolSpec> {
    let item_schema = json!({
        "type": "object",
        "description": "Study item, see the STUDY SETS section of the system prompt. Unknown fields are rejected.",
        "properties": {
            "id": { "type": "string", "description": "Only when updating an existing item (keeps its history)" },
            "type": { "type": "string", "enum": ["single", "multi", "text", "flashcard"] },
            "prompt": { "type": "string", "description": "Markdown: LaTeX with $...$, code, tables, mermaid and looty-graph blocks" },
            "choices": { "type": "array", "items": { "type": "object", "properties": { "id": { "type": "string" }, "text": { "type": "string" } }, "required": ["id", "text"] } },
            "answer": { "type": "object" },
            "explanation": { "type": "string", "description": "Markdown, same syntax as the prompt" }
        },
        "required": ["type", "prompt", "answer"]
    });
    vec![
        ToolSpec {
            name: "create_study_set",
            description: "Create a PERSONAL entrainement (study set: qcm, questionnaire, flashcards or summary) visible to the user only. Only works in a workspace (or the personal space) for which the user granted 'create study sets' in this conversation. Items are validated strictly; fix and retry on error. Item text (prompt, choices, explanation, flashcard back) supports Markdown: LaTeX ($x^2$), tables, code, callouts, ```mermaid and ```looty-graph blocks.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "kind": { "type": "string", "enum": ["qcm", "questionnaire", "flashcards", "summary"] },
                    "workspace_id": { "type": "string", "description": "Omit to use the user's current workspace (their personal space if none). After the user approves the card, always reuse exactly the same target" },
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
    // Le modèle désigne souvent l'espace perso par son identifiant (list_pages le montre) : c'est la même cible que « espace personnel ».
    let workspace = match workspace {
        Some(ws) if ws == crate::personal::personal_space(pool, user.id).await? => None,
        other => other,
    };
    match require_create_grant(pool, conv_id, workspace).await {
        Ok(()) => {}
        // Une autorisation donnée à l'espace perso par son identifiant (cartes de la 0.37.0) reste valable.
        Err(ApiError::Forbidden) if workspace.is_none() => {
            let personal = crate::personal::personal_space(pool, user.id).await?;
            require_create_grant(pool, conv_id, Some(personal)).await?;
        }
        Err(e) => return Err(e),
    }
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
    let ws = study_target(pool, user, conv_id, args).await?;
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

pub async fn read_study_set(pool: &PgPool, user: &AuthUser, args: &Value) -> ApiResult<ToolOutput> {
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

pub const READ_ITEM_KINDS: [&str; 4] = ["task", "board", "event", "project"];

/// Lit une tâche, un tableau, un événement ou un projet avec les droits de la personne :
/// (titre, texte). Un élément absent ou invisible pour elle donne `NotFound`, comme ailleurs.
pub async fn read_item(pool: &PgPool, user: &AuthUser, kind: &str, id: Uuid) -> ApiResult<(String, String)> {
    if !READ_ITEM_KINDS.contains(&kind) {
        return Err(ApiError::bad_request("unsupported_kind"));
    }
    let (_, title) = study::resolve_target(pool, user, kind, id).await?.ok_or(ApiError::NotFound)?;
    let text = match kind {
        "task" => {
            type When = Option<chrono::DateTime<chrono::Utc>>;
            let row: (String, String, String, When, When, i32, bool) = sqlx::query_as(
                "SELECT regexp_replace(coalesce(t.description, ''), '<[^>]*>', ' ', 'g'), c.name, b.name, t.start_date, t.due_date, t.priority, t.is_completed
                 FROM tasks t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id WHERE t.id = $1",
            )
            .bind(id)
            .fetch_one(pool)
            .await?;
            let subs: Vec<(String, bool)> = sqlx::query_as("SELECT title, is_completed FROM tasks WHERE parent_id = $1 AND (private_to IS NULL OR private_to = $2) ORDER BY position LIMIT 50")
                .bind(id)
                .bind(user.id)
                .fetch_all(pool)
                .await?;
            let fmt = |d: Option<chrono::DateTime<chrono::Utc>>| d.map(|d| d.to_rfc3339()).unwrap_or_default();
            let mut s = format!("Board: {}\nColumn: {}\nPriority: {}\nCompleted: {}\nStart: {}\nDue: {}\n\n{}", row.2, row.1, row.5, row.6, fmt(row.3), fmt(row.4), row.0.trim());
            if !subs.is_empty() {
                s.push_str("\n\nSubtasks:");
                for (t, done) in subs {
                    s.push_str(&format!("\n- [{}] {t}", if done { "x" } else { " " }));
                }
            }
            s
        }
        "board" => {
            let cols: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, name FROM board_columns WHERE board_id = $1 ORDER BY position").bind(id).fetch_all(pool).await?;
            let mut s = String::new();
            for (cid, name) in cols {
                let tasks: Vec<(String, bool)> = sqlx::query_as("SELECT title, is_completed FROM tasks WHERE column_id = $1 AND parent_id IS NULL AND (private_to IS NULL OR private_to = $2) ORDER BY position LIMIT 40")
                    .bind(cid)
                    .bind(user.id)
                    .fetch_all(pool)
                    .await?;
                s.push_str(&format!("## {name}\n"));
                for (t, done) in tasks {
                    s.push_str(&format!("- [{}] {t}\n", if done { "x" } else { " " }));
                }
            }
            s
        }
        "event" => {
            let ics: String = sqlx::query_scalar("SELECT ics FROM workspace_events WHERE id = $1").bind(id).fetch_one(pool).await?;
            match crate::calendar::master_event(&ics) {
                Some(e) => format!(
                    "Start: {}\nEnd: {}\nAll day: {}\nLocation: {}\nRecurrence: {}\n\n{}",
                    e.start.to_rfc3339(),
                    e.end.to_rfc3339(),
                    e.all_day,
                    e.location.unwrap_or_default(),
                    e.rrule.unwrap_or_default(),
                    e.description.unwrap_or_default()
                ),
                None => String::new(),
            }
        }
        _ => {
            // Projet : s'il est porté par une page, il suit l'accès à cette page.
            let page: Option<(Uuid, bool)> = sqlx::query_as("SELECT id, deleted_at IS NOT NULL FROM pages_all WHERE project_id = $1").bind(id).fetch_optional(pool).await?;
            if let Some((p, trashed)) = page {
                if trashed {
                    return Err(ApiError::NotFound);
                }
                require_page_read(pool, user, p).await?;
            }
            let (kind, settings): (String, Value) = sqlx::query_as("SELECT kind::text, settings FROM projects WHERE id = $1").bind(id).fetch_one(pool).await?;
            // Seules les clés utiles à la lecture : pas le reste des réglages.
            let shown: Vec<String> = ["main", "engine", "entry"].iter().filter_map(|k| settings[k].as_str().map(|v| format!("{k}: {v}"))).collect();
            format!("Kind: {kind}\n{}{}", shown.join("\n"), page.map(|(p, _)| format!("\nPage: {p} (read it with read_page)")).unwrap_or_default())
        }
    };
    Ok((title, text))
}

async fn read_item_tool(pool: &PgPool, user: &AuthUser, args: &Value) -> ApiResult<ToolOutput> {
    let id = uuid_arg(args, "id")?;
    let kind = args["kind"].as_str().unwrap_or_default();
    let (title, text) = read_item(pool, user, kind, id).await?;
    Ok(ToolOutput::text(json!({ "kind": kind, "id": id, "title": title, "content": text }).to_string()))
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
    async fn a_refused_write_offers_an_authorization_card_only_to_someone_who_may_write(pool: PgPool) {
        let w = world(&pool).await;
        let args = json!({ "page_id": w.page, "markdown": "x" });
        let o = execute(&pool, &w.owner, w.conv, "write_page", &args).await;
        assert!(o.text.starts_with("error: forbidden"));
        let card = o.grant_needed.expect("card");
        assert_eq!(card["kind"], "page");
        assert_eq!(card["page_id"], json!(w.page));
        assert_eq!(card["title"], "P");
        // Quelqu'un qui ne peut pas écrire (ni lire) la page n'obtient aucune carte.
        let conv2: Uuid = sqlx::query_scalar("INSERT INTO ai_conversations (user_id) VALUES ($1) RETURNING id").bind(w.outsider.id).fetch_one(&pool).await.unwrap();
        assert!(execute(&pool, &w.outsider, conv2, "write_page", &args).await.grant_needed.is_none());
        // Une écriture autorisée n'en produit pas.
        grant(&pool, &w, "replace", 30).await;
        let ok = execute(&pool, &w.owner, w.conv, "write_page", &args).await;
        assert!(ok.grant_needed.is_none() && ok.page_revision.is_some());
    }

    #[sqlx::test]
    async fn undoing_a_direct_write_restores_the_page_and_stays_undoable(pool: PgPool) {
        let w = world(&pool).await;
        grant(&pool, &w, "replace", 30).await;
        let o = execute(&pool, &w.owner, w.conv, "write_page", &json!({ "page_id": w.page, "markdown": "Nouveau", "operation": "replace" })).await;
        let rev = o.page_revision.expect("revision");
        assert_eq!(md(&pool, w.page).await, "Nouveau");
        assert!(matches!(restore_page_revision(&pool, &w.outsider, rev).await, Err(ApiError::NotFound) | Err(ApiError::Forbidden)));
        let (page, undo) = restore_page_revision(&pool, &w.owner, rev).await.unwrap();
        assert_eq!(page, w.page);
        assert_eq!(md(&pool, w.page).await, "Départ");
        restore_page_revision(&pool, &w.owner, undo.unwrap()).await.unwrap();
        assert_eq!(md(&pool, w.page).await, "Nouveau");
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
    async fn the_personal_space_uuid_is_the_personal_target(pool: PgPool) {
        let w = world(&pool).await;
        let personal = crate::personal::personal_space(&pool, w.owner.id).await.unwrap();
        // Sans autorisation : refus, message clair.
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(Some(personal))).await;
        assert!(out.text.starts_with("error: forbidden") && out.text.contains("Active authorizations: none"), "{}", out.text);
        // La carte proposée est personnelle.
        let card = out.grant_needed.expect("card");
        assert_eq!(card["personal"], json!(true));
        assert!(card["workspace_id"].is_null());
        // Autorisation personnelle (workspace NULL) + identifiant de l'espace perso : créé.
        study_grant(&pool, w.conv, None, None, 30).await;
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(Some(personal))).await;
        assert!(out.study_set.is_some(), "{}", out.text);
        assert_eq!(count_sets(&pool).await, 1);
    }

    #[sqlx::test]
    async fn the_current_workspace_is_the_default_target(pool: PgPool) {
        let w = world(&pool).await;
        let ws = ws_of(&pool, &w).await;
        let personal = crate::personal::personal_space(&pool, w.owner.id).await.unwrap();
        sqlx::query("UPDATE ai_conversations SET default_workspace_id = $2 WHERE id = $1").bind(w.conv).bind(ws).execute(&pool).await.unwrap();
        // Sans workspace_id : la carte nomme l'espace courant, pas l'espace perso.
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(None)).await;
        let card = out.grant_needed.expect("card");
        assert_eq!(card["personal"], json!(false));
        assert_eq!(card["workspace_id"], json!(ws));
        // Une autorisation pour l'espace perso n'élargit rien.
        study_grant(&pool, w.conv, None, None, 30).await;
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(None)).await;
        assert!(out.text.starts_with("error: forbidden"), "{}", out.text);
        assert!(out.text.contains("personal space"), "{}", out.text);
        // Autorisation pour l'espace courant : créé là, en personnel.
        study_grant(&pool, w.conv, Some(ws), None, 30).await;
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(None)).await;
        assert!(out.study_set.is_some(), "{}", out.text);
        let row: (Uuid, Option<Uuid>) = sqlx::query_as("SELECT workspace_id, private_to FROM study_sets").fetch_one(&pool).await.unwrap();
        assert_eq!(row, (ws, Some(w.owner.id)));
        // Un autre espace ou l'espace perso explicites : toujours refusés, la liste des autorisations actives est donnée.
        let other: Uuid = sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('Autre') RETURNING id").fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')").bind(other).bind(w.owner.id.to_string()).execute(&pool).await.unwrap();
        for target in [other, personal] {
            sqlx::query("DELETE FROM ai_study_grants WHERE personal").execute(&pool).await.unwrap();
            let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(Some(target))).await;
            assert!(out.text.starts_with("error: forbidden"), "{}", out.text);
            assert!(out.text.contains(&format!("workspace_id {ws}")) && out.text.contains("retry with exactly"), "{}", out.text);
        }
        assert_eq!(count_sets(&pool).await, 1);
    }

    #[sqlx::test]
    async fn a_grant_stored_with_the_personal_space_uuid_still_works(pool: PgPool) {
        let w = world(&pool).await;
        let personal = crate::personal::personal_space(&pool, w.owner.id).await.unwrap();
        sqlx::query("INSERT INTO ai_study_grants (conversation_id, kind, workspace_id, personal, expires_at) VALUES ($1, 'create', $2, false, now() + interval '30 minutes')")
            .bind(w.conv)
            .bind(personal)
            .execute(&pool)
            .await
            .unwrap();
        let out = execute(&pool, &w.owner, w.conv, "create_study_set", &qcm_args(None)).await;
        assert!(out.study_set.is_some(), "{}", out.text);
        assert_eq!(count_sets(&pool).await, 1);
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

    /// Un tableau (avec une colonne et une tâche) dans l'espace du monde.
    async fn board_with_task(pool: &PgPool, w: &World, private_to: Option<Uuid>) -> (Uuid, Uuid) {
        let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(w.page).fetch_one(pool).await.unwrap();
        let board: Uuid = sqlx::query_scalar("INSERT INTO boards (workspace_id, name, created_by, private_to) VALUES ($1, 'Sprint', $2, $3) RETURNING id").bind(ws).bind(w.owner.id).bind(private_to).fetch_one(pool).await.unwrap();
        let col: Uuid = sqlx::query_scalar("INSERT INTO board_columns (board_id, name, position) VALUES ($1, 'Todo', 1) RETURNING id").bind(board).fetch_one(pool).await.unwrap();
        let task: Uuid = sqlx::query_scalar("INSERT INTO tasks (column_id, title, description, position) VALUES ($1, 'Écrire le rapport', '<p>Plan détaillé</p>', 1) RETURNING id").bind(col).fetch_one(pool).await.unwrap();
        (board, task)
    }

    #[sqlx::test]
    async fn read_item_reads_tasks_and_boards_with_the_users_rights(pool: PgPool) {
        let w = world(&pool).await;
        let (board, task) = board_with_task(&pool, &w, None).await;
        let (title, text) = read_item(&pool, &w.owner, "task", task).await.unwrap();
        assert_eq!(title, "Écrire le rapport");
        assert!(text.contains("Plan détaillé") && text.contains("Sprint") && !text.contains("<p>"), "{text}");
        let (_, text) = read_item(&pool, &w.owner, "board", board).await.unwrap();
        assert!(text.contains("## Todo") && text.contains("Écrire le rapport"), "{text}");
        // Hors de l'espace : introuvable.
        assert!(matches!(read_item(&pool, &w.outsider, "task", task).await, Err(ApiError::NotFound)));
        assert!(matches!(read_item(&pool, &w.outsider, "board", board).await, Err(ApiError::NotFound)));
        assert!(read_item(&pool, &w.owner, "page", w.page).await.is_err());
    }

    #[sqlx::test]
    async fn a_project_whose_page_is_trashed_cannot_be_read(pool: PgPool) {
        let w = world(&pool).await;
        let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        let project: Uuid = sqlx::query_scalar("INSERT INTO projects (workspace_id, kind, name, settings) VALUES ($1, 'latex', 'Thèse', '{\"main\":\"main.tex\",\"secret\":\"x\"}') RETURNING id").bind(ws).fetch_one(&pool).await.unwrap();
        sqlx::query("UPDATE pages_all SET project_id = $2 WHERE id = $1").bind(w.page).bind(project).execute(&pool).await.unwrap();
        let (_, text) = read_item(&pool, &w.owner, "project", project).await.unwrap();
        assert!(text.contains("main: main.tex") && !text.contains("secret"), "{text}");
        sqlx::query("UPDATE pages_all SET deleted_at = now() WHERE id = $1").bind(w.page).execute(&pool).await.unwrap();
        assert!(matches!(read_item(&pool, &w.owner, "project", project).await, Err(ApiError::NotFound)));
    }

    #[sqlx::test]
    async fn a_personal_board_of_someone_else_cannot_be_read(pool: PgPool) {
        let w = world(&pool).await;
        let (board, task) = board_with_task(&pool, &w, Some(w.outsider.id)).await;
        assert!(matches!(read_item(&pool, &w.owner, "board", board).await, Err(ApiError::NotFound)));
        assert!(matches!(read_item(&pool, &w.owner, "task", task).await, Err(ApiError::NotFound)));
        let o = execute(&pool, &w.owner, w.conv, "read_item", &json!({ "kind": "task", "id": task })).await;
        assert!(o.text.starts_with("error"), "{}", o.text);
    }

    async fn assistant_message(pool: &PgPool, conv: Uuid, role: &str, content: &str) -> Uuid {
        sqlx::query_scalar("INSERT INTO ai_messages (conversation_id, role, content) VALUES ($1, $2, $3) RETURNING id").bind(conv).bind(role).bind(content).fetch_one(pool).await.unwrap()
    }

    #[sqlx::test]
    async fn odd_markdown_converts_without_crashing(pool: PgPool) {
        let w = world(&pool).await;
        let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        let big_table = format!("| a | b |\n| - | - |\n{}", "| 1 | 2 |\n".repeat(5_000));
        let wide_table = format!("|{}\n|{}\n|{}\n", " h |".repeat(300), " - |".repeat(300), " c |".repeat(300));
        let nested = format!("{}x", "> ".repeat(300));
        let nested_lists = (0..60).map(|i| format!("{}- n{i}\n", "  ".repeat(i))).collect::<String>();
        let cases = [
            "```mermaid\ngraph TD\nA-->B".to_string(),
            "~~~\n```\n~~~~\n```looty-graph\n".to_string(),
            ":::info\n:::warning\n:::\n:::".to_string(),
            "$$ unclosed $ \\( \\[".to_string(),
            "**a *b ~~c `d".to_string(),
            "|\n|-|\n||||".to_string(),
            "\u{202e}\u{1f600}\r\n\r\n\t- [ ]\n- [x".to_string(),
            big_table,
            wide_table,
            nested,
            nested_lists,
        ];
        for md in cases {
            let msg = assistant_message(&pool, w.conv, "assistant", &md).await;
            let r = page_from_message(&pool, &w.owner, msg, Some(ws), true).await;
            assert!(r.is_ok(), "{:?} for {:.40}", r.err(), md);
        }
    }

    #[test]
    fn page_titles_come_from_the_answer() {
        assert_eq!(title_from_markdown("Intro\n\n## Complexité **amortie**\ntexte"), "Complexité amortie");
        assert_eq!(title_from_markdown("\n  Une phrase `simple`.\nsuite"), "Une phrase simple.");
        assert_eq!(title_from_markdown("  \n"), "Réponse de l'IA");
        assert_eq!(title_from_markdown(&"x".repeat(300)).chars().count(), 80);
    }

    #[sqlx::test]
    async fn an_answer_becomes_a_page_for_whoever_can_edit_documents(pool: PgPool) {
        let w = world(&pool).await;
        let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        let md = "# Résumé\n\nSoit $O(n^2)$.\n\n:::info\nAttention\n:::\n\n```mermaid\ngraph TD\nA-->B\n```\n\n- [x] fait\n";
        let msg = assistant_message(&pool, w.conv, "assistant", md).await;
        let (page, got_ws) = page_from_message(&pool, &w.owner, msg, Some(ws), true).await.unwrap();
        assert_eq!(got_ws, ws);
        let (title, private_to): (String, Option<Uuid>) = sqlx::query_as("SELECT title, private_to FROM pages_all WHERE id = $1").bind(page).fetch_one(&pool).await.unwrap();
        assert_eq!(title, "Résumé");
        assert_eq!(private_to, Some(w.owner.id), "personal by default");
        let back = page_markdown(&pool, page).await.unwrap().1;
        assert!(back.contains("$O(n^2)$") && back.contains(":::info") && back.contains("```mermaid") && back.contains("- [x] fait"), "{back}");
        // Partagée sur demande.
        let (shared, _) = page_from_message(&pool, &w.owner, msg, Some(ws), false).await.unwrap();
        let pt: Option<Uuid> = sqlx::query_scalar("SELECT private_to FROM pages_all WHERE id = $1").bind(shared).fetch_one(&pool).await.unwrap();
        assert!(pt.is_none());
    }

    #[sqlx::test]
    async fn to_page_needs_ownership_an_assistant_message_and_edit_rights(pool: PgPool) {
        let w = world(&pool).await;
        let ws: Uuid = sqlx::query_scalar("SELECT workspace_id FROM pages WHERE id = $1").bind(w.page).fetch_one(&pool).await.unwrap();
        let msg = assistant_message(&pool, w.conv, "assistant", "Bonjour").await;
        let user_msg = assistant_message(&pool, w.conv, "user", "Question").await;
        // Conversation d'une autre personne : introuvable.
        assert!(matches!(page_from_message(&pool, &w.outsider, msg, Some(ws), true).await, Err(ApiError::NotFound)));
        // Message de la personne, pas de l'assistant.
        assert!(matches!(page_from_message(&pool, &w.owner, user_msg, Some(ws), true).await, Err(ApiError::NotFound)));
        // Lecteur de l'espace : pas le droit d'éditer les documents.
        let viewer = user(&pool, "viewer").await;
        sqlx::query("INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'viewer')").bind(ws).bind(viewer.id.to_string()).execute(&pool).await.unwrap();
        let conv: Uuid = sqlx::query_scalar("INSERT INTO ai_conversations (user_id) VALUES ($1) RETURNING id").bind(viewer.id).fetch_one(&pool).await.unwrap();
        let vmsg = assistant_message(&pool, conv, "assistant", "Bonjour").await;
        assert!(matches!(page_from_message(&pool, &viewer, vmsg, Some(ws), true).await, Err(ApiError::Forbidden)));
        // Sans espace ni page de contexte : demande invalide.
        assert!(page_from_message(&pool, &w.owner, msg, None, true).await.is_err());
        // Page de contexte : son espace sert de défaut.
        sqlx::query("UPDATE ai_conversations SET context_kind = 'page', context_id = $2 WHERE id = $1").bind(w.conv).bind(w.page).execute(&pool).await.unwrap();
        assert_eq!(page_from_message(&pool, &w.owner, msg, None, true).await.unwrap().1, ws);
    }
}
