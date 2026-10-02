use axum::{
    body::Body,
    extract::{multipart::Field, Multipart, Path},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
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
    let name = format!("{}.{}", Uuid::new_v4(), ext);
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

fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
        && !name.contains("..")
}

/// Upload générique (images de l'éditeur, avatars, favicon...).
pub async fn upload_file(_user: AuthUser, mut multipart: Multipart) -> ApiResult<Json<UploadResponse>> {
    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError::BadRequest(e.to_string()))? {
        if field.file_name().is_none() {
            continue;
        }
        let saved = save_field(field).await?;
        return Ok(Json(UploadResponse { url: saved.url, name: saved.original_name, mime: saved.mime, size: saved.size }));
    }
    Err(ApiError::bad_request("no_file"))
}

/// Sert un fichier uploadé avec des en-têtes défensifs :
/// - `nosniff` + type MIME déduit de l'extension ;
/// - CSP `sandbox` : un HTML/SVG malveillant ne peut exécuter aucun script ;
/// - téléchargement forcé pour les types non sûrs.
pub async fn serve_upload(Path(name): Path<String>) -> Response {
    if !is_safe_name(&name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = PathBuf::from(UPLOAD_DIR).join(&name);
    let Ok(file) = tokio::fs::File::open(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let mime_str = mime.essence_str().to_string();
    let inline = INLINE_MIME_PREFIXES.iter().any(|p| mime_str.starts_with(p));

    let stream = tokio_util_stream(file);
    let mut resp = Response::new(Body::from_stream(stream));
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_str(&mime_str).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    // Le lecteur PDF natif de Chrome refuse de s'afficher sous `sandbox` ; un PDF n'exécute pas le JS de la page.
    let csp = if mime_str == "application/pdf" {
        "default-src 'none'; object-src 'self'; plugin-types application/pdf"
    } else {
        "sandbox; default-src 'none'; img-src 'self'; media-src 'self'; style-src 'unsafe-inline'"
    };
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(csp));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=31536000, immutable"));
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static(if inline { "inline" } else { "attachment" }),
    );
    resp
}

fn tokio_util_stream(file: tokio::fs::File) -> impl futures::Stream<Item = std::io::Result<bytes::Bytes>> {
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
