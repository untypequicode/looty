//! Projet porté par une page publiée (GLOSSARY : *Lien public*). Le visiteur sans compte lit le
//! notebook tel qu'il est enregistré (le `.ipynb` en base, sans temps réel) et ses images, ou
//! le dernier PDF compilé d'un projet LaTeX. Il ne voit jamais les autres fichiers du projet et
//! ne déclenche jamais de compilation. Chaque route revérifie le jeton (`share_token` et
//! `is_public`) : un lien retiré ne sert plus rien. Les projets « site » ne sont pas concernés.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::projects::{clean_path, file_bytes, FileRow};

/// Ce que la page publique dit de son projet.
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct PublicProject {
    pub kind: String,
    /// Notebook ouvert d'abord (projet notebook).
    pub main: Option<String>,
    /// Chemins des notebooks du projet (projet notebook).
    pub notebooks: Vec<String>,
    /// Un PDF a déjà été compilé (projet LaTeX).
    pub has_pdf: bool,
}

/// Projet d'une page publiée : (id, type, réglages). Jeton inconnu, lien retiré, page à la
/// corbeille ou sans projet : introuvable.
async fn shared_project(pool: &PgPool, token: Uuid) -> ApiResult<(Uuid, String, Value)> {
    sqlx::query_as(
        "SELECT pr.id, pr.kind, pr.settings FROM pages p JOIN projects pr ON pr.id = p.project_id
         WHERE p.share_token = $1 AND p.is_public = true",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?
    .ok_or(ApiError::NotFound)
}

/// Projet d'une page publiée, d'un type donné.
async fn shared_project_of(pool: &PgPool, token: Uuid, kind: &str) -> ApiResult<Uuid> {
    let (id, k, _) = shared_project(pool, token).await?;
    if k != kind {
        return Err(ApiError::NotFound);
    }
    Ok(id)
}

/// Résumé du projet pour `get_public_page` (None : pas de projet, ou un site).
pub async fn summary(pool: &PgPool, token: Uuid) -> ApiResult<Option<PublicProject>> {
    let (id, kind, settings) = match shared_project(pool, token).await {
        Ok(p) => p,
        Err(ApiError::NotFound) => return Ok(None),
        Err(e) => return Err(e),
    };
    match kind.as_str() {
        "notebook" => {
            let notebooks: Vec<String> = sqlx::query_scalar("SELECT path FROM project_files WHERE project_id = $1 AND NOT is_dir AND lower(path) LIKE '%.ipynb' ORDER BY path")
                .bind(id)
                .fetch_all(pool)
                .await?;
            let main = settings
                .get("main")
                .and_then(Value::as_str)
                .unwrap_or("notebook.ipynb")
                .to_string();
            let main = if notebooks.contains(&main) {
                Some(main)
            } else {
                notebooks.first().cloned()
            };
            Ok(Some(PublicProject {
                kind,
                main,
                notebooks,
                has_pdf: false,
            }))
        }
        "latex" => {
            let has_pdf = tokio::fs::try_exists(crate::latex::output_path(id))
                .await
                .unwrap_or(false);
            Ok(Some(PublicProject {
                kind,
                main: None,
                notebooks: Vec::new(),
                has_pdf,
            }))
        }
        _ => Ok(None),
    }
}

/// Type d'une image servie par le lien public (png, jpg, jpeg, gif, webp, svg uniquement).
pub(crate) fn image_mime(path: &str) -> Option<&'static str> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

pub(crate) fn is_notebook(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".ipynb")
}

async fn file_at(pool: &PgPool, project: Uuid, path: &str) -> ApiResult<Option<FileRow>> {
    Ok(sqlx::query_as("SELECT id, path, is_dir, content, blob_path, mime FROM project_files WHERE project_id = $1 AND path = $2 AND NOT is_dir")
        .bind(project)
        .bind(path)
        .fetch_optional(pool)
        .await?)
}

#[derive(Deserialize)]
pub struct NotebookQuery {
    path: String,
    /// Téléchargement (fichier tel quel) plutôt qu'affichage.
    #[serde(default)]
    download: Option<String>,
}

/// `.ipynb` enregistré d'un projet notebook publié.
pub async fn notebook(
    State(pool): State<PgPool>,
    Path(token): Path<Uuid>,
    Query(q): Query<NotebookQuery>,
) -> ApiResult<Response> {
    let project = shared_project_of(&pool, token, "notebook").await?;
    let path = clean_path(&q.path).map_err(|_| ApiError::NotFound)?;
    if !is_notebook(&path) {
        return Err(ApiError::NotFound);
    }
    let f = file_at(&pool, project, &path)
        .await?
        .ok_or(ApiError::NotFound)?;
    let bytes = file_bytes(&f).await?;
    let name = path.rsplit('/').next().unwrap_or("notebook.ipynb");
    let disposition = if q.download.is_some() {
        "attachment"
    } else {
        "inline"
    };
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-ipynb+json".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!(
                    "{disposition}; filename*=UTF-8''{}",
                    crate::mailclient::pct(name)
                ),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
            (header::CONTENT_SECURITY_POLICY, "sandbox".into()),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        Body::from(bytes),
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct ImageQuery {
    /// Second chemin essayé si le premier n'existe pas (dossier du notebook, puis racine).
    #[serde(default)]
    alt: Option<String>,
}

/// Image d'un projet notebook publié, par chemin. SVG servi en bac à sable, comme les envois.
pub async fn image(
    State(pool): State<PgPool>,
    Path((token, raw)): Path<(Uuid, String)>,
    Query(q): Query<ImageQuery>,
) -> ApiResult<Response> {
    let project = shared_project_of(&pool, token, "notebook").await?;
    let mut found = None;
    for candidate in std::iter::once(raw.as_str()).chain(q.alt.as_deref()) {
        let Ok(path) = clean_path(candidate) else {
            continue;
        };
        let Some(mime) = image_mime(&path) else {
            continue;
        };
        if let Some(f) = file_at(&pool, project, &path).await? {
            found = Some((f, mime));
            break;
        }
    }
    let (f, mime) = found.ok_or(ApiError::NotFound)?;
    let bytes = file_bytes(&f).await?;
    Ok((
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (header::CONTENT_DISPOSITION, "inline".into()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
            (
                header::CONTENT_SECURITY_POLICY,
                crate::upload::SANDBOX_CSP.into(),
            ),
            // Court : un lien retiré cesse vite de servir l'image.
            (header::CACHE_CONTROL, "public, max-age=60".into()),
        ],
        Body::from(bytes),
    )
        .into_response())
}

/// Dernier PDF compilé d'un projet LaTeX publié (jamais de compilation ici).
pub async fn pdf(State(pool): State<PgPool>, Path(token): Path<Uuid>) -> ApiResult<Response> {
    let project = shared_project_of(&pool, token, "latex").await?;
    let Ok(bytes) = tokio::fs::read(crate::latex::output_path(project)).await else {
        return Ok((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "latex_no_pdf" })),
        )
            .into_response());
    };
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "inline; filename=\"document.pdf\"".into(),
            ),
            (header::CACHE_CONTROL, "no-store".into()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        ],
        Body::from(bytes),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;

    #[test]
    fn only_images_are_served() {
        for p in [
            "a.png",
            "images/b.JPG",
            "c.jpeg",
            "d.gif",
            "e.webp",
            "f.svg",
        ] {
            assert!(image_mime(p).is_some(), "{p}");
        }
        for p in [
            "notebook.ipynb",
            "data.csv",
            "main.tex",
            "index.html",
            "x.svgz",
            "png",
            "a.png.txt",
            "script.js",
        ] {
            assert!(image_mime(p).is_none(), "{p}");
        }
        assert_eq!(image_mime("x.svg"), Some("image/svg+xml"));
        assert!(is_notebook("sub/A.IPYNB") && !is_notebook("a.ipynb.bak"));
    }

    struct Fixture {
        app: axum::Router,
        token: Uuid,
        page: Uuid,
        project: Uuid,
    }

    async fn fixture(pool: &PgPool, kind: &str) -> Fixture {
        let user: Uuid =
            sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, 'Alice') RETURNING id")
                .bind(format!("{}@x.io", Uuid::new_v4()))
                .fetch_one(pool)
                .await
                .unwrap();
        let ws: Uuid =
            sqlx::query_scalar("INSERT INTO workspaces (name) VALUES ('Cours') RETURNING id")
                .fetch_one(pool)
                .await
                .unwrap();
        let project: Uuid = sqlx::query_scalar("INSERT INTO projects (workspace_id, kind, name) VALUES ($1, $2, 'Projet') RETURNING id").bind(ws).bind(kind).fetch_one(pool).await.unwrap();
        let token = Uuid::new_v4();
        let page: Uuid = sqlx::query_scalar("INSERT INTO pages_all (workspace_id, title, created_by, project_id, share_token, is_public) VALUES ($1, 'Page', $2, $3, $4, true) RETURNING id")
            .bind(ws)
            .bind(user)
            .bind(project)
            .bind(token)
            .fetch_one(pool)
            .await
            .unwrap();
        for (path, content) in [
            ("notebook.ipynb", "{\"cells\":[]}"),
            ("sub/autre.ipynb", "{}"),
            ("images/a.png", "PNG"),
            ("images/b.svg", "<svg/>"),
            ("data.csv", "a,b"),
            ("secret.txt", "chut"),
        ] {
            sqlx::query(
                "INSERT INTO project_files (project_id, path, content) VALUES ($1, $2, $3)",
            )
            .bind(project)
            .bind(path)
            .bind(content)
            .execute(pool)
            .await
            .unwrap();
        }
        let state = crate::state::AppState::new(pool.clone(), "http://localhost".into());
        Fixture {
            app: crate::app(state),
            token,
            page,
            project,
        }
    }

    async fn get(app: &axum::Router, uri: &str) -> (u16, String, String) {
        let res = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status().as_u16();
        let csp = res
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&body).into_owned(), csp)
    }

    #[sqlx::test]
    async fn the_public_page_describes_its_notebook(pool: PgPool) {
        let f = fixture(&pool, "notebook").await;
        let (status, body, _) = get(&f.app, &format!("/api/public/pages/{}", f.token)).await;
        assert_eq!(status, 200, "{body}");
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            v["project"],
            json!({ "kind": "notebook", "main": "notebook.ipynb", "notebooks": ["notebook.ipynb", "sub/autre.ipynb"], "has_pdf": false })
        );
        assert!(!body.contains("secret.txt") && !body.contains("data.csv"));
    }

    #[sqlx::test]
    async fn the_token_guards_every_route(pool: PgPool) {
        let f = fixture(&pool, "notebook").await;
        let t = f.token;
        assert_eq!(
            get(
                &f.app,
                &format!("/api/public/pages/{t}/notebook?path=notebook.ipynb")
            )
            .await
            .0,
            200
        );
        assert_eq!(
            get(&f.app, &format!("/api/public/pages/{t}/files/images/a.png"))
                .await
                .0,
            200
        );
        let other = Uuid::new_v4();
        assert_eq!(
            get(
                &f.app,
                &format!("/api/public/pages/{other}/notebook?path=notebook.ipynb")
            )
            .await
            .0,
            404
        );
        assert_eq!(
            get(
                &f.app,
                &format!("/api/public/pages/{other}/files/images/a.png")
            )
            .await
            .0,
            404
        );
        // Lien retiré, puis page à la corbeille : plus rien n'est servi.
        sqlx::query("UPDATE pages_all SET is_public = false WHERE id = $1")
            .bind(f.page)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            get(&f.app, &format!("/api/public/pages/{t}/files/images/a.png"))
                .await
                .0,
            404
        );
        assert_eq!(
            get(
                &f.app,
                &format!("/api/public/pages/{t}/notebook?path=notebook.ipynb")
            )
            .await
            .0,
            404
        );
        sqlx::query("UPDATE pages_all SET is_public = true, deleted_at = now() WHERE id = $1")
            .bind(f.page)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            get(&f.app, &format!("/api/public/pages/{t}/files/images/a.png"))
                .await
                .0,
            404
        );
    }

    #[sqlx::test]
    async fn only_notebooks_and_images_are_readable(pool: PgPool) {
        let f = fixture(&pool, "notebook").await;
        let t = f.token;
        let (status, body, _) = get(
            &f.app,
            &format!("/api/public/pages/{t}/notebook?path=sub/autre.ipynb"),
        )
        .await;
        assert_eq!((status, body.as_str()), (200, "{}"));
        for path in ["secret.txt", "data.csv", "../secret.txt", "images/a.png"] {
            assert_eq!(
                get(
                    &f.app,
                    &format!("/api/public/pages/{t}/notebook?path={path}")
                )
                .await
                .0,
                404,
                "{path}"
            );
        }
        for path in [
            "secret.txt",
            "data.csv",
            "notebook.ipynb",
            "images/missing.png",
        ] {
            assert_eq!(
                get(&f.app, &format!("/api/public/pages/{t}/files/{path}"))
                    .await
                    .0,
                404,
                "{path}"
            );
        }
        // Le second chemin ne contourne pas le contrôle du type.
        assert_eq!(
            get(
                &f.app,
                &format!("/api/public/pages/{t}/files/x.png?alt=secret.txt")
            )
            .await
            .0,
            404
        );
        assert_eq!(
            get(
                &f.app,
                &format!("/api/public/pages/{t}/files/sub/images/a.png?alt=images/a.png")
            )
            .await
            .0,
            200
        );
        let (status, body, csp) =
            get(&f.app, &format!("/api/public/pages/{t}/files/images/b.svg")).await;
        assert_eq!((status, body.as_str()), (200, "<svg/>"));
        assert!(csp.starts_with("sandbox"), "{csp}");
        // Pas de PDF pour un notebook.
        assert_eq!(
            get(&f.app, &format!("/api/public/pages/{t}/output.pdf"))
                .await
                .0,
            404
        );
    }

    #[sqlx::test]
    async fn a_latex_page_only_exposes_its_pdf(pool: PgPool) {
        let f = fixture(&pool, "latex").await;
        let t = f.token;
        let (_, body, _) = get(&f.app, &format!("/api/public/pages/{t}")).await;
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            v["project"],
            json!({ "kind": "latex", "main": null, "notebooks": [], "has_pdf": false })
        );
        let (status, body, _) = get(&f.app, &format!("/api/public/pages/{t}/output.pdf")).await;
        assert_eq!(
            (status, body.as_str()),
            (404, r#"{"error":"latex_no_pdf"}"#)
        );
        assert_eq!(
            get(&f.app, &format!("/api/public/pages/{t}/files/images/a.png"))
                .await
                .0,
            404
        );
        assert_eq!(
            get(
                &f.app,
                &format!("/api/public/pages/{t}/notebook?path=notebook.ipynb")
            )
            .await
            .0,
            404
        );
        // Dernier PDF compilé : servi tel quel, et signalé par la page.
        let out = crate::latex::output_path(f.project);
        tokio::fs::create_dir_all(out.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&out, b"%PDF-1.4").await.unwrap();
        let (status, body, _) = get(&f.app, &format!("/api/public/pages/{t}/output.pdf")).await;
        let (_, page, _) = get(&f.app, &format!("/api/public/pages/{t}")).await;
        let _ = tokio::fs::remove_dir_all(out.parent().unwrap()).await;
        assert_eq!((status, body.as_str()), (200, "%PDF-1.4"));
        assert!(page.contains(r#""has_pdf":true"#), "{page}");
    }

    #[sqlx::test]
    async fn a_site_page_is_unchanged(pool: PgPool) {
        let f = fixture(&pool, "site").await;
        let t = f.token;
        let (_, body, _) = get(&f.app, &format!("/api/public/pages/{t}")).await;
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(v["project"].is_null(), "{body}");
        assert_eq!(
            get(&f.app, &format!("/api/public/pages/{t}/files/images/a.png"))
                .await
                .0,
            404
        );
        assert_eq!(
            get(
                &f.app,
                &format!("/api/public/pages/{t}/notebook?path=notebook.ipynb")
            )
            .await
            .0,
            404
        );
    }
}
