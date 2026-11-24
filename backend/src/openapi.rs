//! Spécification OpenAPI de l'API publique, servie par `GET /api/openapi.json`.
//!
//! La table `OPS` est la liste des routes dont Looty garantit la stabilité pour les intégrations
//! (scripts, n8n, bots) ; un test vérifie que chacune existe bien dans le routeur, pour que la
//! spécification ne décrive jamais une route disparue. Les routes absentes de la table existent
//! (l'interface s'en sert) mais peuvent changer sans préavis.

use axum::Json;
use serde_json::{json, Value};

struct Op {
    method: &'static str,
    path: &'static str,
    tag: &'static str,
    summary: &'static str,
    /// Schéma du corps (dans `components.schemas`), s'il y en a un.
    body: Option<&'static str>,
}

const fn op(method: &'static str, path: &'static str, tag: &'static str, summary: &'static str, body: Option<&'static str>) -> Op {
    Op { method, path, tag, summary, body }
}

const OPS: &[Op] = &[
    op("get", "/api/auth/me", "Account", "The person the token acts for", None),
    op("get", "/api/workspaces", "Workspaces", "List workspaces (limited to the token's workspaces)", None),
    op("get", "/api/workspaces/{workspace_id}", "Workspaces", "Get a workspace", None),
    op("get", "/api/workspaces/{workspace_id}/members", "Workspaces", "List members", None),
    op("get", "/api/workspaces/{workspace_id}/search", "Workspaces", "Search documents and tasks (query parameter q)", None),
    op("get", "/api/workspaces/{workspace_id}/boards", "Boards", "List boards", None),
    op("post", "/api/workspaces/{workspace_id}/boards", "Boards", "Create a board, optionally from a template", Some("BoardCreate")),
    op("get", "/api/boards/{board_id}", "Boards", "Get a board with its columns, labels and tasks", None),
    op("post", "/api/boards/{board_id}/columns", "Boards", "Create a column", Some("ColumnCreate")),
    op("post", "/api/boards/columns/{column_id}/tasks", "Tasks", "Create a task in a column", Some("TaskCreate")),
    op("get", "/api/boards/tasks/{task_id}", "Tasks", "Get a task", None),
    op("put", "/api/boards/tasks/{task_id}", "Tasks", "Update a task (fields left out are unchanged)", Some("TaskUpdate")),
    op("delete", "/api/boards/tasks/{task_id}", "Tasks", "Delete a task", None),
    op("put", "/api/boards/tasks/{task_id}/move", "Tasks", "Move a task to a column and position", Some("TaskMove")),
    op("get", "/api/workspaces/{workspace_id}/pages", "Documents", "List documents", None),
    op("post", "/api/workspaces/{workspace_id}/pages", "Documents", "Create a document, optionally from a template", Some("PageCreate")),
    op("get", "/api/workspaces/{workspace_id}/pages/{page_id}", "Documents", "Get a document", None),
    op("put", "/api/workspaces/{workspace_id}/pages/{page_id}", "Documents", "Update a document", Some("PageUpdate")),
    op("delete", "/api/workspaces/{workspace_id}/pages/{page_id}", "Documents", "Move a document to the trash", None),
    op("post", "/api/workspaces/{workspace_id}/events", "Calendar", "Create a calendar event", Some("EventWrite")),
    op("get", "/api/workspaces/{workspace_id}/events/{id}", "Calendar", "Get a calendar event", None),
    op("put", "/api/workspaces/{workspace_id}/events/{id}", "Calendar", "Update a calendar event", Some("EventWrite")),
    op("delete", "/api/workspaces/{workspace_id}/events/{id}", "Calendar", "Delete a calendar event", None),
    op("get", "/api/workspaces/{workspace_id}/announcements", "Announcements", "List announcements", None),
    op("post", "/api/workspaces/{workspace_id}/announcements", "Announcements", "Publish an announcement", Some("AnnouncementWrite")),
    op("get", "/api/workspaces/{workspace_id}/webhooks", "Webhooks", "List webhooks (workspace admins)", None),
    op("post", "/api/workspaces/{workspace_id}/webhooks", "Webhooks", "Create a webhook; the signing secret is returned once", Some("WebhookWrite")),
    op("put", "/api/workspaces/{workspace_id}/webhooks/{id}", "Webhooks", "Update a webhook", Some("WebhookWrite")),
    op("delete", "/api/workspaces/{workspace_id}/webhooks/{id}", "Webhooks", "Delete a webhook", None),
    op("post", "/api/workspaces/{workspace_id}/webhooks/{id}/secret", "Webhooks", "Generate a new signing secret", None),
    op("post", "/api/workspaces/{workspace_id}/webhooks/{id}/test", "Webhooks", "Send a test event (ping)", None),
    op("get", "/api/workspaces/{workspace_id}/webhooks/{id}/deliveries", "Webhooks", "Delivery log of a webhook (last 100)", None),
    op("get", "/api/workspaces/{workspace_id}/templates", "Templates", "List board and document templates", None),
    op("post", "/api/workspaces/{workspace_id}/templates", "Templates", "Save a board or a document as a workspace template", Some("TemplateCreate")),
    op("delete", "/api/workspaces/{workspace_id}/templates/{id}", "Templates", "Delete a workspace template", None),
];

fn path_params(path: &str) -> Vec<Value> {
    path.split('/')
        .filter_map(|s| s.strip_prefix('{').and_then(|s| s.strip_suffix('}')))
        .map(|n| json!({ "name": n, "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }))
        .collect()
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

fn schemas() -> Value {
    let uuid = json!({ "type": "string", "format": "uuid" });
    let date = json!({ "type": "string", "format": "date-time" });
    json!({
        "Error": obj(json!({ "error": { "type": "string", "description": "Machine-readable code such as unauthorized, forbidden, not_found, token_read_only, token_expired, token_workspace_scope." } }), &["error"]),
        "BoardCreate": obj(json!({
            "name": { "type": "string" },
            "with_default_columns": { "type": "boolean" },
            "personal": { "type": "boolean" },
            "template_id": { "type": "string", "description": "Id of a workspace template, or the name of a built-in one: project-tracking, retrospective, course." }
        }), &["name"]),
        "ColumnCreate": obj(json!({ "name": { "type": "string" } }), &["name"]),
        "TaskCreate": obj(json!({
            "title": { "type": "string" }, "description": { "type": "string" },
            "start_date": date, "due_date": date, "priority": { "type": "integer", "minimum": 0, "maximum": 4 },
            "labels": { "type": "array", "items": uuid }, "assignees": { "type": "array", "items": uuid },
            "at_top": { "type": "boolean" }, "personal": { "type": "boolean" }
        }), &["title"]),
        "TaskUpdate": obj(json!({
            "title": { "type": "string" }, "description": { "type": ["string", "null"] },
            "start_date": { "type": ["string", "null"], "format": "date-time" }, "due_date": { "type": ["string", "null"], "format": "date-time" },
            "priority": { "type": "integer", "minimum": 0, "maximum": 4 }, "is_completed": { "type": "boolean" }
        }), &[]),
        "TaskMove": obj(json!({ "column_id": uuid, "position": { "type": "number" } }), &["column_id", "position"]),
        "PageCreate": obj(json!({
            "title": { "type": "string" }, "icon": { "type": "string" }, "parent_id": uuid, "content": { "type": "string", "description": "HTML" },
            "personal": { "type": "boolean" },
            "template_id": { "type": "string", "description": "Id of a workspace template, or the name of a built-in one: meeting-notes, course-notes." }
        }), &[]),
        "PageUpdate": obj(json!({ "title": { "type": "string" }, "icon": { "type": ["string", "null"] }, "parent_id": { "type": ["string", "null"], "format": "uuid" }, "content": { "type": "string" }, "position": { "type": "number" } }), &[]),
        "EventWrite": obj(json!({
            "title": { "type": "string" }, "description": { "type": "string" }, "location": { "type": "string" }, "all_day": { "type": "boolean" },
            "start": { "type": "string", "description": "RFC 3339 instant, or YYYY-MM-DD for an all-day event" },
            "end": { "type": "string", "description": "Same format as start (the end day is included for all-day events)" },
            "tz": { "type": "string", "description": "IANA time zone" }
        }), &["title", "all_day", "start", "end"]),
        "AnnouncementWrite": obj(json!({ "title": { "type": "string" }, "body": { "type": "string" }, "pinned": { "type": "boolean" }, "expires_at": { "type": ["string", "null"], "format": "date-time" } }), &["title"]),
        "WebhookWrite": obj(json!({
            "url": { "type": "string", "format": "uri", "description": "https address (http only if the instance allows private addresses)" },
            "events": { "type": "array", "items": { "type": "string", "enum": crate::webhooks::EVENTS } },
            "active": { "type": "boolean" }
        }), &["url", "events"]),
        "TemplateCreate": obj(json!({
            "kind": { "type": "string", "enum": ["board", "page"] },
            "source_id": { "type": "string", "format": "uuid", "description": "The board or the document to save" },
            "name": { "type": "string" }
        }), &["kind", "source_id"]),
    })
}

pub fn document() -> Value {
    let mut paths = serde_json::Map::new();
    for o in OPS {
        let mut operation = json!({
            "tags": [o.tag],
            "summary": o.summary,
            "operationId": format!("{}_{}", o.method, o.path.trim_start_matches("/api/").replace(['/', '{', '}', '-', '.'], "_").trim_matches('_')),
            "parameters": path_params(o.path),
            "responses": {
                "200": { "description": "Success (204 when there is nothing to return)" },
                "401": { "description": "Missing, invalid or expired token", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } } },
                "403": { "description": "The token or the person is not allowed", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } } },
                "404": { "description": "Not found", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } } }
            }
        });
        if let Some(b) = o.body {
            operation["requestBody"] = json!({ "required": true, "content": { "application/json": { "schema": { "$ref": format!("#/components/schemas/{b}") } } } });
        }
        paths.entry(o.path).or_insert_with(|| json!({}))[o.method] = operation;
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Looty API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Integrations with a personal access token (Settings > Security). Send it as `Authorization: Bearer looty_...`. A token acts as its owner and never exceeds their rights; it can also be read-only, limited to some workspaces, and expire. Webhook deliveries are described under /docs (signature header X-Looty-Signature)."
        },
        "servers": [{ "url": "/" }],
        "security": [{ "bearerAuth": [] }],
        "paths": paths,
        "components": {
            "securitySchemes": { "bearerAuth": { "type": "http", "scheme": "bearer", "description": "Personal access token" } },
            "schemas": schemas()
        }
    })
}

pub async fn spec() -> Json<Value> {
    Json(document())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[test]
    fn every_body_schema_exists_and_operations_are_unique() {
        let doc = document();
        let schemas = doc["components"]["schemas"].as_object().unwrap();
        let mut ids = std::collections::HashSet::new();
        for o in OPS {
            if let Some(b) = o.body {
                assert!(schemas.contains_key(b), "{b}");
            }
            assert!(ids.insert((o.method, o.path)), "{} {}", o.method, o.path);
        }
        assert_eq!(doc["openapi"], "3.1.0");
    }

    /// Chaque route documentée existe : sans connexion elle répond 401, jamais 404 (chemin inconnu) ni 405 (méthode inconnue).
    #[sqlx::test]
    async fn every_documented_route_exists(pool: sqlx::PgPool) {
        crate::session_store::PgSessionStore::new(pool.clone()).migrate().await.unwrap();
        let app = crate::app(crate::state::AppState::new(pool, "http://localhost".into()));
        for o in OPS {
            let mut path = o.path.to_string();
            while let (Some(a), Some(b)) = (path.find('{'), path.find('}')) {
                path.replace_range(a..=b, "00000000-0000-0000-0000-000000000001");
            }
            let req = Request::builder().method(o.method.to_uppercase().as_str()).uri(&path).header("content-type", "application/json").body(Body::from("{}")).unwrap();
            let status = app.clone().oneshot(req).await.unwrap().status().as_u16();
            // `/api/auth/me` répond 200 « non connecté » plutôt que 401.
            let expected = if o.path == "/api/auth/me" { 200 } else { 401 };
            assert_eq!(status, expected, "{} {} is documented but answers {status}", o.method, o.path);
        }
    }
}
