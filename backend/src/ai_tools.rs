//! Outils offerts aux modèles et garde-fous d'écriture (docs/adr/0009).
//!
//! Tout est vérifié ici, côté serveur, avec les droits de la PERSONNE qui discute : le texte
//! de l'invite ou du modèle n'accorde jamais rien. Lecture seule par défaut ; écrire exige
//! une autorisation (`ai_write_grants`) valide pour cette conversation et cette page, plus le
//! droit d'écriture de la personne sur la page (`require_page_write`).

use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::access::{require_member_or_guest, require_page_read, require_page_write, visible_pages, AuthUser, PageAccess};
use crate::ai_pages::{self, Op};
use crate::ai_provider::ToolSpec;
use crate::error::{ApiError, ApiResult};

pub const MODES: [&str; 3] = ["suggest", "append", "replace"];

pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
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
    ]
}

pub struct ToolOutput {
    /// Texte renvoyé au modèle.
    pub text: String,
    /// Proposition à afficher (mode « suggest »).
    pub suggestion: Option<Value>,
    /// Page modifiée (l'interface recharge).
    pub changed_page: Option<Uuid>,
}

impl ToolOutput {
    fn text(t: impl Into<String>) -> Self {
        ToolOutput { text: t.into(), suggestion: None, changed_page: None }
    }
}

/// Message d'erreur renvoyé au modèle (sans détail interne).
fn error_text(e: &ApiError) -> String {
    match e {
        ApiError::Forbidden => "error: forbidden. Either no write grant is active for this page in this conversation (ask the user to grant it in the chat), the grant mode does not allow this operation, or the user has no edit right on the page.".into(),
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

/// Markdown courant d'une page : l'état Yjs, ou à défaut l'ancien HTML.
pub async fn page_markdown(pool: &PgPool, page_id: Uuid) -> ApiResult<(String, String)> {
    let row: (String, Option<Vec<u8>>, Option<String>) = sqlx::query_as("SELECT title, yjs_state, content FROM pages WHERE id = $1").bind(page_id).fetch_one(pool).await?;
    let md = match row.1.as_deref().filter(|s| !s.is_empty()) {
        Some(state) => ai_pages::state_to_markdown(state).map_err(|e| ApiError::Internal(format!("yjs decode: {e}")))?,
        None => ai_pages::html_to_text(row.2.as_deref().unwrap_or("")).trim().to_string(),
    };
    Ok((row.0, md))
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
    let old = row.0.filter(|s| !s.is_empty());
    // Page jamais ouverte dans l'éditeur : son texte n'existe qu'en HTML, qu'on ne sait pas convertir sans perte.
    if old.is_none() && !ai_pages::html_to_text(row.1.as_deref().unwrap_or("")).trim().is_empty() {
        return Err(ApiError::Conflict("page_not_opened_in_editor: open this page once in the editor, then retry".into()));
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
    let revision = match old.as_deref() {
        Some(s) => Some(crate::revisions::snapshot_before_write(&mut tx, page_id, user.id, "Avant modification par l'IA", s).await?),
        None => None,
    };
    let done = sqlx::query("UPDATE pages_all SET yjs_state = $1, updated_at = NOW(), updated_by = $2 WHERE id = $3 AND yjs_state IS NOT DISTINCT FROM $4")
        .bind(&new_state)
        .bind(user.id)
        .bind(page_id)
        .bind(old.as_deref())
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
        WriteOutcome::Applied { .. } => ToolOutput { text: "ok: the page was updated (a restorable version was saved before).".into(), suggestion: None, changed_page: Some(page_id) },
        WriteOutcome::Suggested { suggestion_id, diff } => ToolOutput {
            text: "ok: the change was NOT applied; it was proposed to the user, who must accept it in the chat. Do not claim the page changed.".into(),
            suggestion: Some(json!({ "id": suggestion_id, "page_id": page_id, "diff": diff.iter().map(|(o, l)| json!({ "op": o.to_string(), "text": l })).collect::<Vec<_>>() })),
            changed_page: None,
        },
    })
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
}
