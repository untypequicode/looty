use axum::{
    extract::Multipart,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::path::PathBuf;
use uuid::Uuid;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tower_sessions::Session;

#[derive(Serialize)]
pub struct UploadResponse {
    pub url: String,
}

pub async fn upload_file(
    session: Session,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Basic auth check
    let user_id: Option<String> = session.get("user_id").await.unwrap_or(None);
    if user_id.is_none() {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()));
    }

    let mut url = String::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
        // We only care about the "file" or "image" field, but we can accept any first file
        if let Some(file_name) = field.file_name() {
            let extension = std::path::Path::new(file_name)
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("bin")
                .to_string();
            
            let data = field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

            let unique_id = Uuid::new_v4().to_string();
            let new_filename = format!("{}.{}", unique_id, extension);
            let filepath = PathBuf::from("uploads").join(&new_filename);

            let mut file = File::create(&filepath).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            file.write_all(&data).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            url = format!("/uploads/{}", new_filename);
            break; // Stop after first file
        }
    }

    if url.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No file uploaded".to_string()));
    }

    Ok(Json(UploadResponse { url }))
}
