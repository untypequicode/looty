//! Projets hébergés : mini-sites (HTML/CSS/JS) et projets LaTeX.
//!
//! - Un projet est une arborescence de fichiers. Les fichiers texte sont édités à
//!   plusieurs en direct (document Yjs `file:<id>` du serveur de collaboration, qui tient
//!   aussi `content` à jour) ; les fichiers binaires (images, polices, PDF…) sont envoyés.
//! - Droits : ceux des documents de l'espace (lecture, modification ; suppression d'un
//!   projet : gestion ; publication d'un site : droit de partage).
//! - Mini-site : aperçu privé (lien à jeton, sans cookie) et publication à
//!   `/pages/<adresse>/…`. Le code des sites est servi isolé (CSP `sandbox` sans
//!   `allow-same-origin`) : il ne peut ni lire ni utiliser la session Looty.
//! - LaTeX : compilation sur le serveur (voir `latex`), PDF et journal.

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::access::{level, member_perms, AuthUser, Perms};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn workspace_routes() -> Router<AppState> {
    Router::new().route("/", get(list_projects).post(create_project))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{id}", get(get_project).put(update_project).delete(delete_project))
        .route("/{id}/files", post(create_file))
        .route("/{id}/files/{fid}", put(update_file).delete(delete_file))
        .route("/{id}/files/{fid}/raw", get(raw_file))
        .route("/{id}/upload", post(upload).layer(DefaultBodyLimit::max(MAX_UPLOAD)))
        .route("/{id}/download", get(download_zip))
        .route("/{id}/bundle", get(bundle))
        .route("/{id}/preview-token", post(preview_token))
        .route("/{id}/page", post(attach_page))
        .route("/{id}/compile", post(crate::latex::compile))
        .route("/{id}/output.pdf", get(crate::latex::output_pdf))
        .route("/files/{fid}/access", get(file_access))
}

/// Sites publiés et aperçus (hors /api : adresses publiques).
pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/pages/_preview/{id}/{token}", get(serve_preview_root))
        .route("/pages/_preview/{id}/{token}/", get(serve_preview_index))
        .route("/pages/_preview/{id}/{token}/{*path}", get(serve_preview))
        .route("/pages/{slug}", get(serve_site_root))
        .route("/pages/{slug}/", get(serve_site_index))
        .route("/pages/{slug}/{*path}", get(serve_site))
}

const MAX_UPLOAD: usize = 50 * 1024 * 1024;
/// Fichiers texte stockés comme texte (éditables) jusqu'à cette taille.
const MAX_TEXT: usize = 2 * 1024 * 1024;
const MAX_FILES: i64 = 2000;

pub(crate) const TEXT_EXT: &[&str] = &[
    "html", "htm", "css", "js", "mjs", "json", "txt", "md", "svg", "xml", "csv", "tex", "bib", "sty", "cls", "bst", "bbx", "cbx", "ltx", "dtx", "ins", "cfg", "def",
    "fd", "tikz", "py", "ipynb", "ts", "yaml", "yml", "toml", "ini", "webmanifest", "map", "gitignore", "htaccess", "lua",
];

pub(crate) fn is_text_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default();
    TEXT_EXT.contains(&ext.as_str()) || matches!(name.as_str(), "makefile" | "readme" | "license" | "latexmkrc" | ".latexmkrc")
}

// ---------------------------------------------------------------------------
// Accès
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow, Serialize, Clone)]
pub(crate) struct Project {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: String,
    pub name: String,
    pub slug: Option<String>,
    pub published: bool,
    pub settings: Value,
    pub last_build_at: Option<DateTime<Utc>>,
    pub last_build_ok: Option<bool>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const PROJECT_COLS: &str = "id, workspace_id, kind, name, slug, published, settings, last_build_at, last_build_ok, created_by, created_at, updated_at";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Need {
    Read,
    Write,
    Manage,
}

fn allowed(p: &Perms, need: Need) -> bool {
    match need {
        Need::Read => p.docs >= level::READ,
        Need::Write => p.docs >= level::EDIT,
        Need::Manage => p.docs >= level::MANAGE || p.admin,
    }
}

async fn require_ws(state: &AppState, user: &AuthUser, ws: Uuid, need: Need) -> ApiResult<Perms> {
    let (_, perms) = member_perms(&state.pool, user.id, ws).await?.ok_or(ApiError::NotFound)?;
    if !allowed(&perms, Need::Read) {
        return Err(ApiError::NotFound);
    }
    if !allowed(&perms, need) {
        return Err(ApiError::Forbidden);
    }
    Ok(perms)
}

pub(crate) async fn project_for(state: &AppState, user: &AuthUser, id: Uuid, need: Need) -> ApiResult<(Project, Perms)> {
    let p: Project = sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {PROJECT_COLS} FROM projects WHERE id = $1")))
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    // Projet porté par une page : les droits de la page (permissions par page, invités).
    let page: Option<Uuid> = sqlx::query_scalar("SELECT id FROM pages_all WHERE project_id = $1").bind(id).fetch_optional(&state.pool).await?;
    if let Some(page_id) = page {
        let (_, _, access) = crate::access::page_access(&state.pool, user, page_id).await?;
        let mut perms = member_perms(&state.pool, user.id, p.workspace_id).await?.map(|(_, x)| x).unwrap_or(Perms::preset(crate::access::Role::Guest));
        perms.docs = match access {
            crate::access::PageAccess::None => return Err(ApiError::NotFound),
            crate::access::PageAccess::Read => level::READ,
            crate::access::PageAccess::Write => level::EDIT,
        };
        // Supprimer le projet d'une page : qui peut modifier la page (sinon on supprime la page).
        let ok = match need {
            Need::Read => true,
            Need::Write | Need::Manage => perms.docs >= level::EDIT,
        };
        if !ok {
            return Err(ApiError::Forbidden);
        }
        return Ok((p, perms));
    }
    let perms = require_ws(state, user, p.workspace_id, need).await?;
    Ok((p, perms))
}

/// Page qui porte le projet, s'il y en a une.
pub(crate) async fn page_of(state: &AppState, project: Uuid) -> ApiResult<Option<Uuid>> {
    Ok(sqlx::query_scalar("SELECT id FROM pages WHERE project_id = $1").bind(project).fetch_optional(&state.pool).await?)
}

/// Chemin relatif propre : segments non vides, sans `.`/`..`, caractères sûrs.
pub(crate) fn clean_path(raw: &str) -> ApiResult<String> {
    let mut parts = Vec::new();
    for seg in raw.replace('\\', "/").split('/') {
        let seg = seg.trim();
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." || seg.len() > 120 || seg.chars().any(|c| c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
            return Err(ApiError::bad_request("project_invalid_path"));
        }
        parts.push(seg.to_string());
    }
    let path = parts.join("/");
    if path.is_empty() || path.len() > 500 || parts.len() > 12 {
        return Err(ApiError::bad_request("project_invalid_path"));
    }
    Ok(path)
}

fn blob_dir(project: Uuid) -> PathBuf {
    PathBuf::from(crate::upload::UPLOAD_DIR).join("projects").join(project.to_string())
}

/// Dossiers de fichiers envoyés des projets supprimés (par exemple avec leur page, en base).
pub(crate) async fn prune_blob_dirs(pool: &sqlx::PgPool) {
    let Ok(mut dir) = tokio::fs::read_dir(PathBuf::from(crate::upload::UPLOAD_DIR).join("projects")).await else { return };
    while let Ok(Some(entry)) = dir.next_entry().await {
        let Some(id) = entry.file_name().to_str().and_then(|s| Uuid::parse_str(s).ok()) else { continue };
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)").bind(id).fetch_one(pool).await.unwrap_or(true);
        if !exists {
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Projets
// ---------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
struct ProjectSummary {
    id: Uuid,
    kind: String,
    name: String,
    slug: Option<String>,
    published: bool,
    updated_at: DateTime<Utc>,
    last_build_ok: Option<bool>,
    files: i64,
    created_by_name: Option<String>,
    /// Page qui porte le projet (dans les documents).
    page_id: Option<Uuid>,
    page_icon: Option<String>,
    /// Début du fichier principal (aperçu sur la carte).
    snippet: Option<String>,
}

/// Sources des premières cellules d'un notebook, séparées par une ligne vide.
fn notebook_snippet(json: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json).ok()?;
    let text = |s: &Value| match s {
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect::<String>(),
        Value::String(s) => s.clone(),
        _ => String::new(),
    };
    let out: Vec<String> = v["cells"].as_array()?.iter().take(3).map(|c| text(&c["source"])).filter(|s| !s.trim().is_empty()).collect();
    (!out.is_empty()).then(|| out.join("\n\n").chars().take(400).collect())
}

async fn list_projects(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>) -> ApiResult<Json<Vec<ProjectSummary>>> {
    let role = crate::access::require_member_or_guest(&state.pool, &user, ws).await?;
    let rows: Vec<ProjectSummary> = sqlx::query_as(
        "SELECT p.id, p.kind, p.name, p.slug, p.published, GREATEST(p.updated_at, COALESCE(MAX(f.updated_at), p.updated_at)) AS updated_at, p.last_build_ok,
                count(f.id) FILTER (WHERE NOT f.is_dir) AS files, u.name AS created_by_name, pg.id AS page_id, pg.icon AS page_icon,
                (SELECT left(m.content, 400) FROM project_files m WHERE m.project_id = p.id AND m.content IS NOT NULL
                   AND m.path = COALESCE(p.settings->>'main', p.settings->>'entry', 'index.html') LIMIT 1) AS snippet
         FROM projects p LEFT JOIN project_files f ON f.project_id = p.id LEFT JOIN users u ON u.id = p.created_by
         LEFT JOIN pages_all pg ON pg.project_id = p.id
         WHERE p.workspace_id = $1 AND pg.deleted_at IS NULL
         GROUP BY p.id, u.name, pg.id, pg.icon ORDER BY 6 DESC",
    )
    .bind(ws)
    .fetch_all(&state.pool)
    .await?;
    // Projets des pages : selon l'accès à la page ; les autres : droit de lecture des documents.
    // Notebook : aperçu = texte des premières cellules (pas le JSON du fichier).
    let mut rows = rows;
    for r in rows.iter_mut().filter(|r| r.kind == "notebook") {
        let content: Option<String> = sqlx::query_scalar(
            "SELECT f.content FROM project_files f JOIN projects p ON p.id = f.project_id
             WHERE f.project_id = $1 AND f.path = COALESCE(p.settings->>'main', 'notebook.ipynb')",
        )
        .bind(r.id)
        .fetch_optional(&state.pool)
        .await?
        .flatten();
        r.snippet = content.and_then(|c| notebook_snippet(&c));
    }
    let docs_read = crate::access::member_perms(&state.pool, user.id, ws).await?.is_some_and(|(_, p)| p.docs >= level::READ);
    let visible = crate::access::visible_pages(&state.pool, &user, ws, role).await?;
    Ok(Json(
        rows.into_iter()
            .filter(|r| match r.page_id {
                Some(pid) => visible.get(&pid).is_some_and(|a| *a != crate::access::PageAccess::None),
                None => docs_read,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct CreateProject {
    kind: String,
    name: String,
    /// Modèle de départ (site : blank|starter ; latex : article|report|beamer|letter|blank ; notebook : starter|data|blank).
    template: Option<String>,
}

/// Notebook Jupyter (nbformat 4.5) : cellules (type, source) sans sortie.
fn notebook_json(cells: &[(&str, &str)]) -> String {
    let cells: Vec<Value> = cells
        .iter()
        .map(|(kind, source)| {
            let lines: Vec<String> = source.split_inclusive('\n').map(str::to_string).collect();
            let id = Uuid::new_v4().simple().to_string()[..8].to_string();
            if *kind == "markdown" {
                json!({ "cell_type": "markdown", "id": id, "metadata": {}, "source": lines })
            } else {
                json!({ "cell_type": "code", "id": id, "metadata": {}, "execution_count": null, "outputs": [], "source": lines })
            }
        })
        .collect();
    let nb = json!({
        "cells": cells,
        "metadata": {
            "kernelspec": { "name": "python3", "display_name": "Python 3 (Pyodide)", "language": "python" },
            "language_info": { "name": "python" }
        },
        "nbformat": 4,
        "nbformat_minor": 5
    });
    serde_json::to_string_pretty(&nb).unwrap_or_default() + "\n"
}

fn template_files(kind: &str, template: &str, name: &str) -> Vec<(&'static str, String)> {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let tex = |s: &str| s.replace(['\\', '{', '}', '$', '&', '#', '^', '_', '%', '~'], "");
    match (kind, template) {
        ("site", "blank") => vec![("index.html", format!("<!doctype html>\n<html lang=\"fr\">\n<head>\n  <meta charset=\"utf-8\">\n  <title>{}</title>\n</head>\n<body>\n\n</body>\n</html>\n", esc(name)))],
        ("site", _) => vec![
            (
                "index.html",
                format!(
                    "<!doctype html>\n<html lang=\"fr\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>{t}</title>\n  <link rel=\"stylesheet\" href=\"css/style.css\">\n</head>\n<body>\n  <main>\n    <h1>{t}</h1>\n    <p>Modifiez <code>index.html</code>, <code>css/style.css</code> et <code>js/script.js</code> : l’aperçu se met à jour tout seul.</p>\n    <button id=\"hello\">Cliquez-moi</button>\n  </main>\n  <script src=\"js/script.js\"></script>\n</body>\n</html>\n",
                    t = esc(name)
                ),
            ),
            ("css/style.css", "body {\n  font-family: system-ui, sans-serif;\n  margin: 0;\n  color: #1f2023;\n  background: #f7f7f8;\n}\n\nmain {\n  max-width: 720px;\n  margin: 64px auto;\n  padding: 0 24px;\n}\n\nbutton {\n  padding: 8px 16px;\n  border: 0;\n  border-radius: 8px;\n  background: #4c5bd4;\n  color: white;\n  cursor: pointer;\n}\n".into()),
            ("js/script.js", "document.getElementById('hello').addEventListener('click', () => {\n  alert('Bonjour !')\n})\n".into()),
        ],
        ("notebook", "blank") => vec![("notebook.ipynb", notebook_json(&[("code", "")]))],
        ("notebook", "data") => vec![(
            "notebook.ipynb",
            notebook_json(&[
                ("markdown", &format!("# {name}\n\nUn notebook avec **NumPy** et **Matplotlib** : les bibliothèques se chargent toutes seules au premier import.")),
                ("code", "import numpy as np\nimport matplotlib.pyplot as plt\n\nx = np.linspace(0, 2 * np.pi, 200)\nplt.plot(x, np.sin(x), label='sin')\nplt.plot(x, np.cos(x), label='cos')\nplt.legend()\nplt.title('Fonctions trigonométriques')\nplt.show()"),
                ("code", "import pandas as pd\n\ndf = pd.DataFrame({'jour': ['lun', 'mar', 'mer'], 'valeur': [3, 5, 4]})\ndf"),
                ("markdown", "Les fichiers du projet (par exemple un `.csv` envoyé avec « Importer ») sont lisibles directement : `pd.read_csv('donnees.csv')`."),
            ]),
        )],
        ("notebook", _) => vec![(
            "notebook.ipynb",
            notebook_json(&[
                ("markdown", &format!("# {name}\n\nCliquez sur une cellule puis **Maj+Entrée** pour l'exécuter. Le code Python s'exécute dans votre navigateur.")),
                ("code", "print('Bonjour depuis Python !')"),
                ("code", "def fib(n):\n    a, b = 0, 1\n    for _ in range(n):\n        yield a\n        a, b = b, a + b\n\nlist(fib(12))"),
                ("markdown", "Une formule : $e^{i\\pi} + 1 = 0$.\n\nAjoutez des cellules avec les boutons **+ Code** et **+ Texte**."),
            ]),
        )],
        ("latex", "blank") => vec![("main.tex", "\\documentclass{article}\n\\begin{document}\n\n\\end{document}\n".into())],
        ("latex", "beamer") => vec![(
            "main.tex",
            format!("\\documentclass{{beamer}}\n\\usepackage[utf8]{{inputenc}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[french]{{babel}}\n\\usetheme{{Madrid}}\n\n\\title{{{}}}\n\\author{{}}\n\\date{{\\today}}\n\n\\begin{{document}}\n\\frame{{\\titlepage}}\n\n\\begin{{frame}}{{Introduction}}\n  \\begin{{itemize}}\n    \\item Premier point\n    \\item Second point\n  \\end{{itemize}}\n\\end{{frame}}\n\\end{{document}}\n", tex(name)),
        )],
        ("latex", "letter") => vec![(
            "main.tex",
            "\\documentclass[11pt]{letter}\n\\usepackage[utf8]{inputenc}\n\\usepackage[T1]{fontenc}\n\\usepackage[french]{babel}\n\\signature{Prénom Nom}\n\\address{Adresse}\n\n\\begin{document}\n\\begin{letter}{Destinataire \\\\ Adresse}\n\\opening{Madame, Monsieur,}\n\nTexte de la lettre.\n\n\\closing{Cordialement,}\n\\end{letter}\n\\end{document}\n".into(),
        )],
        ("latex", "report") => vec![
            (
                "main.tex",
                format!("\\documentclass[11pt,a4paper]{{report}}\n\\usepackage[utf8]{{inputenc}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[french]{{babel}}\n\\usepackage{{amsmath,amssymb}}\n\\usepackage{{graphicx}}\n\\usepackage{{hyperref}}\n\n\\title{{{}}}\n\\author{{}}\n\\date{{\\today}}\n\n\\begin{{document}}\n\\maketitle\n\\tableofcontents\n\n\\input{{chapitres/introduction}}\n\n\\bibliographystyle{{plain}}\n\\bibliography{{references}}\n\\end{{document}}\n", tex(name)),
            ),
            ("chapitres/introduction.tex", "\\chapter{Introduction}\n\nUn premier chapitre, avec une référence~\\cite{knuth1984}.\n".into()),
            ("references.bib", "@book{knuth1984,\n  author    = {Donald E. Knuth},\n  title     = {The {\\TeX}book},\n  publisher = {Addison-Wesley},\n  year      = {1984}\n}\n".into()),
        ],
        ("latex", _) => vec![
            (
                "main.tex",
                format!("\\documentclass[11pt,a4paper]{{article}}\n\\usepackage[utf8]{{inputenc}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[french]{{babel}}\n\\usepackage{{amsmath,amssymb}}\n\\usepackage{{graphicx}}\n\\usepackage{{hyperref}}\n\n\\title{{{}}}\n\\author{{}}\n\\date{{\\today}}\n\n\\begin{{document}}\n\\maketitle\n\n\\section{{Introduction}}\nVotre texte ici. Une formule : $e^{{i\\pi}} + 1 = 0$, et une référence~\\cite{{knuth1984}}.\n\n\\bibliographystyle{{plain}}\n\\bibliography{{references}}\n\\end{{document}}\n", tex(name)),
            ),
            ("references.bib", "@book{knuth1984,\n  author    = {Donald E. Knuth},\n  title     = {The {\\TeX}book},\n  publisher = {Addison-Wesley},\n  year      = {1984}\n}\n".into()),
        ],
        _ => Vec::new(),
    }
}

async fn create_project(State(state): State<AppState>, user: AuthUser, Path(ws): Path<Uuid>, Json(p): Json<CreateProject>) -> ApiResult<Json<Project>> {
    require_ws(&state, &user, ws, Need::Write).await?;
    Ok(Json(insert_project(&state, ws, user.id, &p.kind, &p.name, p.template.as_deref()).await?))
}

/// Nouveau projet à partir d'un modèle (droits vérifiés par l'appelant).
pub(crate) async fn insert_project(state: &AppState, ws: Uuid, user_id: Uuid, kind: &str, name: &str, template: Option<&str>) -> ApiResult<Project> {
    if !matches!(kind, "site" | "latex" | "notebook") {
        return Err(ApiError::bad_request("invalid_value"));
    }
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(ApiError::bad_request("invalid_value"));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM projects WHERE workspace_id = $1").bind(ws).fetch_one(&state.pool).await?;
    if count >= 200 {
        return Err(ApiError::bad_request("project_too_many"));
    }
    let settings = match kind {
        "latex" => json!({ "main": "main.tex", "engine": "pdflatex" }),
        "notebook" => json!({ "main": "notebook.ipynb" }),
        _ => json!({ "entry": "index.html" }),
    };
    let mut tx = state.pool.begin().await?;
    let project: Project = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO projects (workspace_id, kind, name, settings, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING {PROJECT_COLS}"
    )))
    .bind(ws)
    .bind(kind)
    .bind(name)
    .bind(&settings)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    let files = template_files(kind, template.unwrap_or("starter"), name);
    let mut dirs: Vec<String> = files.iter().filter_map(|(path, _)| path.rsplit_once('/').map(|(d, _)| d.to_string())).collect();
    dirs.sort();
    dirs.dedup();
    for d in dirs {
        sqlx::query("INSERT INTO project_files (project_id, path, is_dir, updated_by) VALUES ($1, $2, TRUE, $3) ON CONFLICT DO NOTHING").bind(project.id).bind(&d).bind(user_id).execute(&mut *tx).await?;
    }
    for (path, content) in files {
        sqlx::query("INSERT INTO project_files (project_id, path, content, mime, size, updated_by) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(project.id)
            .bind(path)
            .bind(&content)
            .bind(mime_guess::from_path(path).first_or_text_plain().essence_str())
            .bind(content.len() as i64)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(project)
}

#[derive(Serialize, sqlx::FromRow)]
struct FileDto {
    id: Uuid,
    path: String,
    is_dir: bool,
    text: bool,
    mime: Option<String>,
    size: i64,
    updated_at: DateTime<Utc>,
    updated_by_name: Option<String>,
}

async fn files_of(state: &AppState, project: Uuid) -> ApiResult<Vec<FileDto>> {
    Ok(sqlx::query_as(
        "SELECT f.id, f.path, f.is_dir, (f.blob_path IS NULL AND NOT f.is_dir) AS text, f.mime,
                CASE WHEN f.blob_path IS NULL THEN COALESCE(octet_length(f.content), 0)::bigint ELSE f.size END AS size,
                f.updated_at, u.name AS updated_by_name
         FROM project_files f LEFT JOIN users u ON u.id = f.updated_by WHERE f.project_id = $1 ORDER BY f.path",
    )
    .bind(project)
    .fetch_all(&state.pool)
    .await?)
}

async fn get_project(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let (p, perms) = project_for(&state, &user, id, Need::Read).await?;
    let files = files_of(&state, id).await?;
    let page_id = page_of(&state, id).await?;
    let public_url = p.slug.as_ref().filter(|_| p.published).map(|s| format!("{}/pages/{s}/", sites_base(&state)));
    Ok(Json(json!({
        "project": p,
        "page_id": page_id,
        "files": files,
        "public_url": public_url,
        "can_write": allowed(&perms, Need::Write),
        "can_manage": allowed(&perms, Need::Manage),
        "can_publish": perms.share || perms.admin,
    })))
}

/// Adresse des sites publiés : SITES_BASE_URL (domaine dédié) ou celle de Looty.
fn sites_base(state: &AppState) -> String {
    std::env::var("SITES_BASE_URL").ok().filter(|v| v.starts_with("http")).map(|v| v.trim_end_matches('/').to_string()).unwrap_or_else(|| state.base_url.trim_end_matches('/').to_string())
}

#[derive(Deserialize)]
struct UpdateProject {
    name: Option<String>,
    settings: Option<Value>,
    slug: Option<String>,
    published: Option<bool>,
}

fn clean_slug(s: &str) -> ApiResult<String> {
    let s = s.trim().to_lowercase();
    let ok = s.len() >= 2 && s.len() <= 64 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !s.starts_with('-') && !s.starts_with('_');
    if !ok {
        return Err(ApiError::bad_request("project_invalid_slug"));
    }
    Ok(s)
}

async fn update_project(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(b): Json<UpdateProject>) -> ApiResult<Json<Value>> {
    let (p, perms) = project_for(&state, &user, id, Need::Write).await?;
    if let Some(name) = b.name.as_deref().map(str::trim) {
        if name.is_empty() || name.chars().count() > 120 {
            return Err(ApiError::bad_request("invalid_value"));
        }
        sqlx::query("UPDATE projects SET name = $2, updated_at = NOW() WHERE id = $1").bind(id).bind(name).execute(&state.pool).await?;
    }
    if let Some(s) = b.settings.as_ref().and_then(Value::as_object) {
        let mut merged = p.settings.as_object().cloned().unwrap_or_default();
        for key in ["main", "engine", "entry"] {
            if let Some(v) = s.get(key).and_then(Value::as_str) {
                let v = v.trim();
                let ok = match key {
                    "engine" => matches!(v, "pdflatex" | "xelatex" | "lualatex"),
                    _ => clean_path(v).is_ok(),
                };
                if !ok {
                    return Err(ApiError::bad_request("invalid_value"));
                }
                merged.insert(key.into(), Value::String(v.into()));
            }
        }
        sqlx::query("UPDATE projects SET settings = $2, updated_at = NOW() WHERE id = $1").bind(id).bind(Value::Object(merged)).execute(&state.pool).await?;
    }
    if b.slug.is_some() || b.published.is_some() {
        if !(perms.share || perms.admin) || p.kind != "site" {
            return Err(ApiError::Forbidden);
        }
        let slug = match b.slug.as_deref() {
            Some(s) if !s.trim().is_empty() => Some(clean_slug(s)?),
            Some(_) => None,
            None => p.slug.clone(),
        };
        let published = b.published.unwrap_or(p.published) && slug.is_some();
        let r = sqlx::query("UPDATE projects SET slug = $2, published = $3, updated_at = NOW() WHERE id = $1").bind(id).bind(&slug).bind(published).execute(&state.pool).await;
        if let Err(sqlx::Error::Database(e)) = &r {
            if e.is_unique_violation() {
                return Err(ApiError::bad_request("project_slug_taken"));
            }
        }
        r?;
    }
    get_project(State(state), user, Path(id)).await
}

async fn delete_project(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    project_for(&state, &user, id, Need::Manage).await?;
    sqlx::query("DELETE FROM projects WHERE id = $1").bind(id).execute(&state.pool).await?;
    let _ = tokio::fs::remove_dir_all(blob_dir(id)).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Projet sans page : une page est créée à la racine des documents et le porte (il suit
/// alors les droits de cette page).
async fn attach_page(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let (p, perms) = project_for(&state, &user, id, Need::Write).await?;
    if page_of(&state, id).await?.is_some() {
        return Err(ApiError::Conflict("page_has_project".into()));
    }
    if perms.docs < level::EDIT {
        return Err(ApiError::Forbidden);
    }
    let position: f64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 65536 FROM pages WHERE workspace_id = $1 AND parent_id IS NULL")
        .bind(p.workspace_id)
        .fetch_one(&state.pool)
        .await?;
    let page: crate::page::Page = sqlx::query_as(
        "INSERT INTO pages (workspace_id, title, created_by, updated_by, position, project_id) VALUES ($1, $2, $3, $3, $4, $5) RETURNING *",
    )
    .bind(p.workspace_id)
    .bind(&p.name)
    .bind(user.id)
    .bind(position)
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    let mut lite = page.clone();
    lite.content = None;
    lite.project_kind = Some(p.kind.clone());
    state.emit(p.workspace_id, json!({ "type": "PAGE_CREATED", "page": lite })).await;
    Ok(Json(json!({ "page_id": page.id })))
}

// ---------------------------------------------------------------------------
// Fichiers
// ---------------------------------------------------------------------------

/// Contenu remplacé hors édition en direct : le serveur de collaboration oublie sa copie.
async fn reset_live(state: &AppState, fid: Uuid) {
    let _ = sqlx::query("SELECT pg_notify('looty_file_reset', $1)").bind(fid.to_string()).execute(&state.pool).await;
}

async fn touch(state: &AppState, project: Uuid) {
    let _ = sqlx::query("UPDATE projects SET updated_at = NOW() WHERE id = $1").bind(project).execute(&state.pool).await;
}

/// Prévient les éditeurs ouverts (arborescence modifiée).
async fn notify_tree(state: &AppState, p: &Project) {
    state.emit(p.workspace_id, json!({ "type": "PROJECT_FILES_CHANGED", "project_id": p.id })).await;
}

async fn ensure_parents(state: &AppState, project: Uuid, path: &str, user: Uuid) -> ApiResult<()> {
    let mut acc = String::new();
    let segs: Vec<&str> = path.split('/').collect();
    for seg in &segs[..segs.len().saturating_sub(1)] {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(seg);
        let existing: Option<bool> = sqlx::query_scalar("SELECT is_dir FROM project_files WHERE project_id = $1 AND path = $2").bind(project).bind(&acc).fetch_optional(&state.pool).await?;
        match existing {
            Some(true) => {}
            Some(false) => return Err(ApiError::bad_request("project_path_conflict")),
            None => {
                sqlx::query("INSERT INTO project_files (project_id, path, is_dir, updated_by) VALUES ($1, $2, TRUE, $3) ON CONFLICT DO NOTHING")
                    .bind(project)
                    .bind(&acc)
                    .bind(user)
                    .execute(&state.pool)
                    .await?;
            }
        }
    }
    Ok(())
}

async fn check_count(state: &AppState, project: Uuid) -> ApiResult<()> {
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM project_files WHERE project_id = $1").bind(project).fetch_one(&state.pool).await?;
    if n >= MAX_FILES {
        return Err(ApiError::bad_request("project_too_many_files"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct CreateFile {
    path: String,
    #[serde(default)]
    is_dir: bool,
    content: Option<String>,
}

async fn create_file(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(b): Json<CreateFile>) -> ApiResult<Json<Value>> {
    let (p, _) = project_for(&state, &user, id, Need::Write).await?;
    let path = clean_path(&b.path)?;
    check_count(&state, id).await?;
    ensure_parents(&state, id, &path, user.id).await?;
    let content = if b.is_dir { None } else { Some(b.content.unwrap_or_default()) };
    if content.as_ref().is_some_and(|c| c.len() > MAX_TEXT) {
        return Err(ApiError::bad_request("project_file_too_large"));
    }
    let fid: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO project_files (project_id, path, is_dir, content, mime, updated_by) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (project_id, path) DO NOTHING RETURNING id",
    )
    .bind(id)
    .bind(&path)
    .bind(b.is_dir)
    .bind(&content)
    .bind((!b.is_dir).then(|| mime_guess::from_path(&path).first_or_text_plain().essence_str().to_string()))
    .bind(user.id)
    .fetch_optional(&state.pool)
    .await?;
    let fid = fid.ok_or_else(|| ApiError::bad_request("project_path_exists"))?;
    touch(&state, id).await;
    notify_tree(&state, &p).await;
    Ok(Json(json!({ "id": fid, "path": path })))
}

#[derive(Deserialize)]
struct UpdateFile {
    /// Nouveau chemin (renommer / déplacer ; un dossier emporte son contenu).
    path: Option<String>,
    /// Contenu (clients sans édition en direct). Ignoré pour un fichier binaire.
    content: Option<String>,
}

async fn update_file(State(state): State<AppState>, user: AuthUser, Path((id, fid)): Path<(Uuid, Uuid)>, Json(b): Json<UpdateFile>) -> ApiResult<Json<Value>> {
    let (p, _) = project_for(&state, &user, id, Need::Write).await?;
    let (old, is_dir, blob): (String, bool, Option<String>) = sqlx::query_as("SELECT path, is_dir, blob_path FROM project_files WHERE id = $1 AND project_id = $2")
        .bind(fid)
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    let mut path = old.clone();
    if let Some(new) = b.path.as_deref() {
        let new = clean_path(new)?;
        if new != old {
            if is_dir && (new == old || new.starts_with(&format!("{old}/"))) {
                return Err(ApiError::bad_request("project_invalid_path"));
            }
            let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM project_files WHERE project_id = $1 AND path = $2)").bind(id).bind(&new).fetch_one(&state.pool).await?;
            if taken {
                return Err(ApiError::bad_request("project_path_exists"));
            }
            ensure_parents(&state, id, &new, user.id).await?;
            let mut tx = state.pool.begin().await?;
            sqlx::query("UPDATE project_files SET path = $3, updated_at = NOW(), updated_by = $4 WHERE id = $1 AND project_id = $2").bind(fid).bind(id).bind(&new).bind(user.id).execute(&mut *tx).await?;
            if is_dir {
                sqlx::query("UPDATE project_files SET path = $3 || substr(path, length($2) + 1) WHERE project_id = $1 AND path LIKE $4")
                    .bind(id)
                    .bind(&old)
                    .bind(&new)
                    .bind(format!("{}/%", old.replace('%', "\\%").replace('_', "\\_")))
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            path = new;
        }
    }
    if let Some(content) = b.content {
        if is_dir || blob.is_some() {
            return Err(ApiError::bad_request("invalid_value"));
        }
        if content.len() > MAX_TEXT {
            return Err(ApiError::bad_request("project_file_too_large"));
        }
        // Le document en direct éventuel est remplacé (état Yjs effacé, reconstruit au chargement).
        sqlx::query("UPDATE project_files SET content = $2, yjs_state = NULL, updated_at = NOW(), updated_by = $3 WHERE id = $1").bind(fid).bind(&content).bind(user.id).execute(&state.pool).await?;
        reset_live(&state, fid).await;
    }
    touch(&state, id).await;
    notify_tree(&state, &p).await;
    Ok(Json(json!({ "id": fid, "path": path })))
}

async fn delete_file(State(state): State<AppState>, user: AuthUser, Path((id, fid)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let (p, _) = project_for(&state, &user, id, Need::Write).await?;
    let (path, is_dir): (String, bool) = sqlx::query_as("SELECT path, is_dir FROM project_files WHERE id = $1 AND project_id = $2")
        .bind(fid)
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    let like = format!("{}/%", path.replace('%', "\\%").replace('_', "\\_"));
    let blobs: Vec<Option<String>> = sqlx::query_scalar("DELETE FROM project_files WHERE project_id = $1 AND (id = $2 OR ($3 AND path LIKE $4)) RETURNING blob_path")
        .bind(id)
        .bind(fid)
        .bind(is_dir)
        .bind(&like)
        .fetch_all(&state.pool)
        .await?;
    for b in blobs.into_iter().flatten() {
        let _ = tokio::fs::remove_file(PathBuf::from(crate::upload::UPLOAD_DIR).join(b)).await;
    }
    touch(&state, id).await;
    notify_tree(&state, &p).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(sqlx::FromRow)]
pub(crate) struct FileRow {
    pub id: Uuid,
    pub path: String,
    pub is_dir: bool,
    pub content: Option<String>,
    pub blob_path: Option<String>,
    pub mime: Option<String>,
}

pub(crate) async fn all_files(state: &AppState, project: Uuid) -> ApiResult<Vec<FileRow>> {
    Ok(sqlx::query_as("SELECT id, path, is_dir, content, blob_path, mime FROM project_files WHERE project_id = $1 ORDER BY path").bind(project).fetch_all(&state.pool).await?)
}

/// Contenu d'un fichier (texte ou binaire).
pub(crate) async fn file_bytes(f: &FileRow) -> ApiResult<Vec<u8>> {
    match &f.blob_path {
        Some(b) => tokio::fs::read(PathBuf::from(crate::upload::UPLOAD_DIR).join(b)).await.map_err(|_| ApiError::NotFound),
        None => Ok(f.content.clone().unwrap_or_default().into_bytes()),
    }
}

async fn raw_file(State(state): State<AppState>, user: AuthUser, Path((id, fid)): Path<(Uuid, Uuid)>) -> ApiResult<Response> {
    project_for(&state, &user, id, Need::Read).await?;
    let f: FileRow = sqlx::query_as("SELECT id, path, is_dir, content, blob_path, mime FROM project_files WHERE id = $1 AND project_id = $2 AND NOT is_dir")
        .bind(fid)
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    let bytes = file_bytes(&f).await?;
    let name = f.path.rsplit('/').next().unwrap_or("file").to_string();
    Ok((
        [
            (header::CONTENT_TYPE, f.mime.clone().unwrap_or_else(|| "application/octet-stream".into())),
            (header::CONTENT_DISPOSITION, format!("attachment; filename*=UTF-8''{}", crate::mailclient::pct(&name))),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
            (header::CONTENT_SECURITY_POLICY, "sandbox".into()),
        ],
        Body::from(bytes),
    )
        .into_response())
}

/// Envoi de fichiers (champ `folder` facultatif, puis un ou plusieurs fichiers).
/// Les fichiers texte deviennent éditables ; les autres sont rangés sous uploads/.
async fn upload(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, mut mp: Multipart) -> ApiResult<Json<Value>> {
    let (p, _) = project_for(&state, &user, id, Need::Write).await?;
    let mut folder = String::new();
    let mut saved = Vec::new();
    while let Some(field) = mp.next_field().await.map_err(|e| ApiError::BadRequest(e.to_string()))? {
        if field.name() == Some("folder") {
            let v = field.text().await.unwrap_or_default();
            folder = if v.trim().is_empty() { String::new() } else { clean_path(&v)? };
            continue;
        }
        let Some(fname) = field.file_name().map(str::to_string) else { continue };
        let rel = if folder.is_empty() { fname.clone() } else { format!("{folder}/{fname}") };
        let path = clean_path(&rel)?;
        let data = field.bytes().await.map_err(|e| ApiError::BadRequest(e.to_string()))?;
        check_count(&state, id).await?;
        ensure_parents(&state, id, &path, user.id).await?;
        let mime = mime_guess::from_path(&path).first_or_octet_stream().essence_str().to_string();
        let as_text = is_text_path(&path) && data.len() <= MAX_TEXT && std::str::from_utf8(&data).is_ok();
        // Remplacement d'un fichier existant : ancien binaire supprimé.
        let old: Option<(Uuid, bool, Option<String>)> = sqlx::query_as("SELECT id, is_dir, blob_path FROM project_files WHERE project_id = $1 AND path = $2").bind(id).bind(&path).fetch_optional(&state.pool).await?;
        if let Some((_, true, _)) = old {
            return Err(ApiError::bad_request("project_path_conflict"));
        }
        if let Some((_, _, Some(b))) = &old {
            let _ = tokio::fs::remove_file(PathBuf::from(crate::upload::UPLOAD_DIR).join(b)).await;
        }
        let (content, blob) = if as_text {
            (Some(String::from_utf8_lossy(&data).into_owned()), None)
        } else {
            let dir = blob_dir(id);
            tokio::fs::create_dir_all(&dir).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            let rel_blob = format!("projects/{id}/{}", Uuid::new_v4());
            tokio::fs::write(PathBuf::from(crate::upload::UPLOAD_DIR).join(&rel_blob), &data).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            (None, Some(rel_blob))
        };
        let fid: Uuid = sqlx::query_scalar(
            "INSERT INTO project_files (project_id, path, content, blob_path, mime, size, updated_by) VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (project_id, path) DO UPDATE SET content = $3, blob_path = $4, mime = $5, size = $6, updated_by = $7, yjs_state = NULL, updated_at = NOW()
             RETURNING id",
        )
        .bind(id)
        .bind(&path)
        .bind(&content)
        .bind(&blob)
        .bind(&mime)
        .bind(data.len() as i64)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
        if old.is_some() {
            reset_live(&state, fid).await;
        }
        saved.push(json!({ "id": fid, "path": path, "text": as_text }));
    }
    touch(&state, id).await;
    notify_tree(&state, &p).await;
    Ok(Json(json!({ "files": saved })))
}

/// Archive zip du projet.
async fn download_zip(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Response> {
    let (p, _) = project_for(&state, &user, id, Need::Read).await?;
    let files = all_files(&state, id).await?;
    let mut entries = Vec::new();
    for f in files.iter().filter(|f| !f.is_dir) {
        entries.push((f.path.clone(), file_bytes(f).await.unwrap_or_default()));
    }
    let zip = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<u8>> {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (path, data) in entries {
            w.start_file(path, opts).map_err(std::io::Error::other)?;
            w.write_all(&data)?;
        }
        Ok(w.finish().map_err(std::io::Error::other)?.into_inner())
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    let name: String = p.name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    Ok((
        [(header::CONTENT_TYPE, "application/zip".to_string()), (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}.zip\""))],
        Body::from(zip),
    )
        .into_response())
}

/// Tous les fichiers (base64) : compilation LaTeX sur l'ordinateur (application de bureau).
async fn bundle(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    use base64::Engine;
    let (p, _) = project_for(&state, &user, id, Need::Read).await?;
    let mut out = Vec::new();
    for f in all_files(&state, id).await? {
        let data = if f.is_dir { String::new() } else { base64::engine::general_purpose::STANDARD.encode(file_bytes(&f).await.unwrap_or_default()) };
        out.push(json!({ "id": f.id, "path": f.path, "is_dir": f.is_dir, "text": f.blob_path.is_none(), "data": data }));
    }
    Ok(Json(json!({ "main": p.settings["main"].as_str().unwrap_or("main.tex"), "engine": p.settings["engine"].as_str().unwrap_or("pdflatex"), "files": out })))
}

/// Accès d'une personne à un fichier texte (serveur de collaboration).
async fn file_access(State(state): State<AppState>, user: AuthUser, Path(fid): Path<Uuid>) -> ApiResult<Json<Value>> {
    let project: Uuid = sqlx::query_scalar("SELECT project_id FROM project_files WHERE id = $1 AND NOT is_dir AND blob_path IS NULL")
        .bind(fid)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::NotFound)?;
    let (_, perms) = project_for(&state, &user, project, Need::Read).await?;
    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = $1").bind(user.id).fetch_one(&state.pool).await?;
    Ok(Json(json!({ "can_write": allowed(&perms, Need::Write), "user_id": user.id, "name": name, "project_id": project })))
}

// ---------------------------------------------------------------------------
// Sites : aperçu et publication
// ---------------------------------------------------------------------------

/// Jetons d'aperçu (en mémoire, 2 h, prolongés à l'usage) : l'aperçu est servi isolé,
/// sans cookie de session.
static PREVIEW: LazyLock<Mutex<HashMap<String, (Uuid, Instant)>>> = LazyLock::new(Default::default);
const PREVIEW_TTL: Duration = Duration::from_secs(2 * 3600);

async fn preview_token(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    project_for(&state, &user, id, Need::Read).await?;
    let token: String = (0..32).map(|_| format!("{:x}", rand::random::<u8>() % 16)).collect();
    let mut map = PREVIEW.lock().unwrap();
    map.retain(|_, (_, at)| at.elapsed() < PREVIEW_TTL);
    map.insert(token.clone(), (id, Instant::now()));
    Ok(Json(json!({ "base": format!("/pages/_preview/{id}/{token}/") })))
}

fn preview_ok(id: Uuid, token: &str) -> bool {
    let mut map = PREVIEW.lock().unwrap();
    match map.get_mut(token) {
        Some((pid, at)) if *pid == id && at.elapsed() < PREVIEW_TTL => {
            *at = Instant::now();
            true
        }
        _ => false,
    }
}

async fn serve_preview_root(State(state): State<AppState>, Path((id, token)): Path<(Uuid, String)>) -> Response {
    if !preview_ok(id, &token) {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_path(&state, id, "", &format!("/pages/_preview/{id}/{token}/")).await
}

async fn serve_preview_index(State(state): State<AppState>, Path((id, token)): Path<(Uuid, String)>) -> Response {
    if !preview_ok(id, &token) {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_path(&state, id, "", "").await
}

async fn serve_preview(State(state): State<AppState>, Path((id, token, path)): Path<(Uuid, String, String)>) -> Response {
    if !preview_ok(id, &token) {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_path(&state, id, &path, "").await
}

async fn site_id(state: &AppState, slug: &str) -> Option<Uuid> {
    sqlx::query_scalar("SELECT id FROM projects WHERE slug = $1 AND published AND kind = 'site'").bind(slug.to_lowercase()).fetch_optional(&state.pool).await.ok().flatten()
}

async fn serve_site_root(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let Some(id) = site_id(&state, &slug).await else { return not_found_page() };
    serve_path(&state, id, "", &format!("/pages/{slug}/")).await
}

async fn serve_site_index(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let Some(id) = site_id(&state, &slug).await else { return not_found_page() };
    serve_path(&state, id, "", "").await
}

async fn serve_site(State(state): State<AppState>, Path((slug, path)): Path<(String, String)>) -> Response {
    let Some(id) = site_id(&state, &slug).await else { return not_found_page() };
    serve_path(&state, id, &path, "").await
}

fn not_found_page() -> Response {
    (StatusCode::NOT_FOUND, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], "<!doctype html><meta charset=utf-8><title>404</title><p style=\"font:16px system-ui;margin:40px\">Page introuvable.</p>").into_response()
}

/// En-têtes communs : contenu isolé (opaque origin), pas de reniflage, pas de référent.
fn site_headers(mime: &str) -> Vec<(header::HeaderName, String)> {
    let sandbox = std::env::var("SITES_SANDBOX").map(|v| v != "false").unwrap_or(true);
    let mut h = vec![
        (header::CONTENT_TYPE, if mime.starts_with("text/") || mime == "application/javascript" || mime == "application/json" { format!("{mime}; charset=utf-8") } else { mime.to_string() }),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        (header::REFERRER_POLICY, "no-referrer".into()),
        (header::CACHE_CONTROL, "no-cache".into()),
        // Aperçu dans Looty (cadre) ; ailleurs, le site publié reste affichable seul.
        (header::X_FRAME_OPTIONS, "SAMEORIGIN".into()),
    ];
    if sandbox {
        h.push((header::CONTENT_SECURITY_POLICY, "sandbox allow-scripts allow-forms allow-popups allow-modals allow-downloads allow-popups-to-escape-sandbox".into()));
    }
    h
}

/// Sert un fichier du projet ; un dossier ou la racine donnent leur index.html. Sans
/// barre finale sur la racine, redirection (les liens relatifs en dépendent).
async fn serve_path(state: &AppState, id: Uuid, raw: &str, redirect_root: &str) -> Response {
    if !redirect_root.is_empty() {
        return (StatusCode::MOVED_PERMANENTLY, [(header::LOCATION, redirect_root.to_string())]).into_response();
    }
    let entry: String = sqlx::query_scalar("SELECT COALESCE(settings->>'entry', 'index.html') FROM projects WHERE id = $1").bind(id).fetch_one(&state.pool).await.unwrap_or_else(|_| "index.html".into());
    let decoded = percent_decode(raw);
    let trimmed = decoded.trim_matches('/');
    let candidates: Vec<String> = if trimmed.is_empty() {
        vec![entry.clone(), "index.html".into()]
    } else {
        match clean_path(trimmed) {
            Ok(p) => vec![p.clone(), format!("{p}/index.html"), format!("{p}.html")],
            Err(_) => return not_found_page(),
        }
    };
    for c in candidates {
        let f: Option<FileRow> = sqlx::query_as("SELECT id, path, is_dir, content, blob_path, mime FROM project_files WHERE project_id = $1 AND path = $2 AND NOT is_dir")
            .bind(id)
            .bind(&c)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();
        if let Some(f) = f {
            // Dossier demandé sans barre finale : redirection pour garder les liens relatifs.
            if !trimmed.is_empty() && c.ends_with("/index.html") && !raw.ends_with('/') {
                let last = trimmed.rsplit('/').next().unwrap_or(trimmed);
                return (StatusCode::MOVED_PERMANENTLY, [(header::LOCATION, format!("{last}/"))]).into_response();
            }
            let mime = mime_guess::from_path(&f.path).first_or_octet_stream().essence_str().to_string();
            let Ok(bytes) = file_bytes(&f).await else { return not_found_page() };
            let mut res = Response::new(Body::from(bytes));
            for (k, v) in site_headers(&mime) {
                if let Ok(v) = v.parse() {
                    res.headers_mut().insert(k, v);
                }
            }
            return res;
        }
    }
    not_found_page()
}

use crate::text::percent_decode;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(clean_path("css//style.css").unwrap(), "css/style.css");
        assert_eq!(clean_path("/a/./b/").unwrap(), "a/b");
        assert!(clean_path("../etc/passwd").is_err());
        assert!(clean_path("a/../../b").is_err());
        assert!(clean_path("").is_err());
        assert!(clean_path("a:b").is_err());
        assert_eq!(percent_decode("caf%C3%A9.html"), "café.html");
        let nb = notebook_json(&[("markdown", "# Titre\n"), ("code", "print(1)")]);
        assert_eq!(notebook_snippet(&nb).as_deref(), Some("# Titre\n\n\nprint(1)"));
        assert_eq!(notebook_snippet("{"), None);
        assert_eq!(percent_decode("%€x"), "%€x");
        assert_eq!(percent_decode("100%"), "100%");
        assert!(is_text_path("chap/intro.TEX") && is_text_path("refs.bib") && !is_text_path("photo.png"));
    }
}
