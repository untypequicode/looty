use axum::{
    body::Body,
    extract::{multipart::Field, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use sqlx::PgPool;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};

pub const UPLOAD_DIR: &str = "uploads";

/// Types affichables en ligne dans le navigateur. Tout le reste est servi en téléchargement.
const INLINE_MIME_PREFIXES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp", "image/avif",
    "application/pdf", "video/", "audio/", "text/plain"];

pub struct SavedFile {
    pub path: String,
    pub url: String,
    pub original_name: String,
    pub mime: String,
    pub size: usize,
}

#[derive(Serialize)]
pub struct UploadResponse {
    pub url: String,
    pub name: String,
    pub mime: String,
    pub size: usize,
}

/// Extension sûre : alphanumérique, 10 caractères max, en minuscules.
fn safe_extension(file_name: &str) -> String {
    std::path::Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| !e.is_empty() && e.len() <= 10 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "bin".into())
}

fn sanitize_display_name(name: &str) -> String {
    let n: String = name
        .chars()
        .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
        .take(200)
        .collect();
    if n.trim().is_empty() { "file".into() } else { n }
}

/// Enregistre un champ multipart sur disque avec un nom aléatoire.
pub async fn save_field(field: Field<'_>) -> ApiResult<SavedFile> {
    let original = sanitize_display_name(field.file_name().unwrap_or("file"));
    let ext = safe_extension(&original);
    let data = field.bytes().await.map_err(|e| ApiError::BadRequest(e.to_string()))?;
    if data.is_empty() {
        return Err(ApiError::bad_request("empty_file"));
    }
    // Démo : le nom commence par l'identifiant de la copie, pour que le ménage retrouve ses fichiers.
    let name = match crate::demo_mode::DEMO_COPY.try_with(|c| *c) {
        Ok(copy) => format!("{copy}_{}.{ext}", Uuid::new_v4()),
        Err(_) => format!("{}.{}", Uuid::new_v4(), ext),
    };
    let path = PathBuf::from(UPLOAD_DIR).join(&name);
    let mut f = tokio::fs::File::create(&path).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    f.write_all(&data).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    let mime = mime_guess::from_ext(&ext).first_or_octet_stream().essence_str().to_string();
    Ok(SavedFile {
        path: format!("{UPLOAD_DIR}/{name}"),
        url: format!("/uploads/{name}"),
        original_name: original,
        mime,
        size: data.len(),
    })
}

/// Supprime un fichier uploadé (ignore les chemins hors du dossier d'upload).
pub async fn remove_upload(path: &str) {
    let Some(name) = path.strip_prefix(&format!("{UPLOAD_DIR}/")) else { return };
    if is_safe_name(name) {
        let _ = tokio::fs::remove_file(PathBuf::from(UPLOAD_DIR).join(name)).await;
    }
}

pub(crate) fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
        && !name.contains("..")
}

#[derive(serde::Deserialize, Default)]
pub struct UploadCtx {
    /// Page où le fichier sera inséré (lecture par qui lit la page).
    pub page: Option<Uuid>,
    /// Espace (sans page) : lecture par ses membres.
    pub workspace: Option<Uuid>,
    /// Bloc-notes personnel : lisible par son auteur seulement, jamais lié à une page.
    pub personal: Option<String>,
}

impl SavedFile {
    pub fn name(&self) -> &str {
        self.url.rsplit('/').next().unwrap_or("")
    }
}

/// Upload générique (images de l'éditeur, avatars, favicon...). Le fichier est rattaché à la page
/// (`?page=`) ou à l'espace (`?workspace=`) d'où il part, sinon à la personne qui l'envoie ; voir `file_access`.
pub async fn upload_file(
    State(pool): State<PgPool>,
    user: AuthUser,
    Query(ctx): Query<UploadCtx>,
    mut multipart: Multipart,
) -> ApiResult<Json<UploadResponse>> {
    let mut binding = crate::file_access::Binding::default();
    if ctx.personal.is_some() {
        // Rien à lier : le fichier reste à son auteur.
    } else if let Some(page) = ctx.page {
        let (ws, _, _) = crate::access::require_page_level(&pool, &user, page, crate::access::level::COMMENT).await?;
        binding = crate::file_access::Binding { workspace_id: Some(ws), page_id: Some(page), task_id: None };
    } else if let Some(ws) = ctx.workspace {
        crate::access::require_member_or_guest(&pool, &user, ws).await?;
        binding.workspace_id = Some(ws);
    }
    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError::BadRequest(e.to_string()))? {
        if field.file_name().is_none() {
            continue;
        }
        let saved = save_field(field).await?;
        crate::file_access::register(&pool, saved.name(), user.id, "upload", binding).await?;
        return Ok(Json(UploadResponse { url: saved.url, name: saved.original_name, mime: saved.mime, size: saved.size }));
    }
    Err(ApiError::bad_request("no_file"))
}

/// CSP des fichiers servis sans confiance (envois, images d'un lien public) : un SVG ou un
/// HTML malveillant ne peut exécuter aucun script.
pub(crate) const SANDBOX_CSP: &str = "sandbox; default-src 'none'; img-src 'self'; media-src 'self'; style-src 'unsafe-inline'";

#[derive(serde::Deserialize, Default)]
pub struct ServeQuery {
    /// Jeton du lien public d'une page qui contient le fichier.
    pub share: Option<Uuid>,
    /// Lien signé à courte durée (`GET /api/uploads/{nom}/link`).
    pub sig: Option<String>,
}

/// Une plage d'octets `bytes=a-b` / `bytes=a-` / `bytes=-n` sur un fichier de `len` octets.
fn parse_range(h: &str, len: u64) -> Option<(u64, u64)> {
    let spec = h.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", n) => {
            let n: u64 = n.parse().ok()?;
            if n == 0 {
                return None;
            }
            (len.saturating_sub(n), len - 1)
        }
        (a, "") => (a.parse().ok()?, len - 1),
        (a, b) => (a.parse().ok()?, b.parse::<u64>().ok()?.min(len - 1)),
    };
    (start <= end && start < len).then_some((start, end))
}

fn cache_header(public: bool) -> HeaderValue {
    HeaderValue::from_static(if public { "public, max-age=86400" } else { "private, no-cache" })
}

/// Sert un fichier uploadé après contrôle d'accès (`file_access::decide`), avec des en-têtes défensifs :
/// - `nosniff` + type MIME déduit de l'extension ;
/// - CSP `sandbox` : un HTML/SVG malveillant ne peut exécuter aucun script ;
/// - téléchargement forcé pour les types non sûrs ;
/// - plages d'octets (audio, vidéo, PDF) ; `private, no-cache` + ETag : la lecture est revérifiée à chaque
///   fois (retirer un accès coupe aussi le fichier déjà chargé par le navigateur), jamais de cache partagé.
pub async fn serve_upload(
    State(pool): State<PgPool>,
    user: Option<AuthUser>,
    Path(name): Path<String>,
    Query(q): Query<ServeQuery>,
    headers: HeaderMap,
) -> Response {
    if !is_safe_name(&name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    // Le fichier doit exister : pas de recherche d'accès (coûteuse) pour un nom inventé.
    let path = PathBuf::from(UPLOAD_DIR).join(&name);
    let Ok(mut file) = tokio::fs::File::open(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let decision = match crate::file_access::decide(&pool, user.as_ref(), &name, q.share, q.sig.as_deref()).await {
        Ok(d) => d,
        Err(e) => return e.into_response(),
    };
    if decision == crate::file_access::Decision::Denied {
        let mut r = if user.is_some() { StatusCode::NOT_FOUND } else { StatusCode::UNAUTHORIZED }.into_response();
        r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
        return r;
    }
    let public = decision == crate::file_access::Decision::Public;
    let etag = format!("\"{name}\"");
    let matches_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|t| t.trim().trim_start_matches("W/") == etag));
    if matches_etag {
        let mut r = StatusCode::NOT_MODIFIED.into_response();
        let h = r.headers_mut();
        h.insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
        h.insert(header::CACHE_CONTROL, cache_header(public));
        h.insert(header::VARY, HeaderValue::from_static("Cookie, Authorization"));
        return r;
    }
    let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let mime_str = mime.essence_str().to_string();
    let inline = INLINE_MIME_PREFIXES.iter().any(|p| mime_str.starts_with(p));

    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok()).map(|h| parse_range(h, len));
    if matches!(range, Some(None)) {
        let mut r = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        r.headers_mut().insert(header::CONTENT_RANGE, HeaderValue::from_str(&format!("bytes */{len}")).unwrap());
        return r;
    }
    let (status, start, count) = match range.flatten() {
        Some((a, b)) => (StatusCode::PARTIAL_CONTENT, a, b - a + 1),
        None => (StatusCode::OK, 0, len),
    };
    if start > 0 {
        use tokio::io::AsyncSeekExt;
        if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    use tokio::io::AsyncReadExt;
    let stream = tokio_util_stream(file.take(count));
    let mut resp = Response::new(Body::from_stream(stream));
    *resp.status_mut() = status;
    let h = resp.headers_mut();
    if status == StatusCode::PARTIAL_CONTENT {
        h.insert(header::CONTENT_RANGE, HeaderValue::from_str(&format!("bytes {start}-{}/{len}", start + count - 1)).unwrap());
    }
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
    h.insert(header::VARY, HeaderValue::from_static("Cookie, Authorization"));
    h.insert(header::CONTENT_TYPE, HeaderValue::from_str(&mime_str).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(count));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    // Le lecteur PDF natif de Chrome refuse de s'afficher sous `sandbox` ; un PDF n'exécute pas le JS de la page.
    let csp = if mime_str == "application/pdf" {
        "default-src 'none'; object-src 'self'; plugin-types application/pdf"
    } else {
        SANDBOX_CSP
    };
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(csp));
    h.insert(header::CACHE_CONTROL, cache_header(public));
    h.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static(if inline { "inline" } else { "attachment" }));
    resp
}

/// `GET /api/uploads/{nom}/link` : lien signé de 10 minutes (clients qui n'envoient pas de cookies :
/// lecteur externe, téléchargement natif). Même contrôle d'accès que la lecture.
pub async fn upload_link(State(pool): State<PgPool>, user: AuthUser, Path(name): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    if !is_safe_name(&name) || !tokio::fs::try_exists(PathBuf::from(UPLOAD_DIR).join(&name)).await.unwrap_or(false) {
        return Err(ApiError::NotFound);
    }
    if crate::file_access::decide(&pool, Some(&user), &name, None, None).await? == crate::file_access::Decision::Denied {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({
        "url": format!("/uploads/{name}?sig={}", crate::file_access::signed_link(&name)),
        "expires_in": crate::file_access::LINK_TTL_SECS
    })))
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-9", 100), Some((0, 9)));
        assert_eq!(parse_range("bytes=90-", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=50-500", 100), Some((50, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=5-1", 100), None);
        assert_eq!(parse_range("bytes=0-1,5-6", 100), None);
    }
}

fn tokio_util_stream<R: tokio::io::AsyncRead + Unpin + Send + 'static>(file: R) -> impl futures::Stream<Item = std::io::Result<bytes::Bytes>> {
    use futures::StreamExt;
    use tokio::io::AsyncReadExt;
    futures::stream::unfold(file, |mut f| async move {
        let mut buf = vec![0u8; 64 * 1024];
        match f.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(bytes::Bytes::from(buf)), f))
            }
            Err(e) => Some((Err(e), f)),
        }
    })
    .boxed()
}
