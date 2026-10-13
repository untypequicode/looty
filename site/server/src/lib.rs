//! Serveur du site de Looty : pages statiques (Astro), fichiers publiés (`/releases`, `/apt`),
//! métadonnées de mise à jour (`/latest.json`), liens de téléchargement stables (`/download/<plateforme>`)
//! et dépôt des fichiers par l'intégration continue (`PUT /_upload/…`, protégé par un jeton).
//!
//! Volontairement petit : un seul fil d'exécution, pas de base de données, tout est dans `DATA_DIR`.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, put},
    Json, Router,
};
use futures_util::TryStreamExt;
use serde_json::{json, Value};
use std::path::{Component, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

/// Plateformes publiées (dossier `releases/<plateforme>/`), dans l'ordre de `latest.json`.
pub const PLATFORMS: [&str; 3] = ["android", "linux", "windows"];
/// Taille maximale d'un fichier déposé.
const MAX_UPLOAD: u64 = 1 << 30;

#[derive(Clone)]
pub struct Config {
    /// Pages du site (sortie d'Astro).
    pub web: PathBuf,
    /// Fichiers publiés : `releases/…` et `apt/…`.
    pub data: PathBuf,
    /// Jeton des dépôts de fichiers ; absent = dépôts désactivés.
    pub token: Option<String>,
    /// Adresse publique du site (https://…), pour les liens absolus ; sinon déduite de la requête.
    pub site_url: Option<String>,
}

type Shared = Arc<Config>;

pub fn app(cfg: Config) -> Router {
    let cfg: Shared = Arc::new(cfg);
    let nosniff = SetResponseHeaderLayer::if_not_present(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    let referrer = SetResponseHeaderLayer::if_not_present(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    let frame = SetResponseHeaderLayer::if_not_present(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    let csp = SetResponseHeaderLayer::if_not_present(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; base-uri 'none'; form-action 'none'"),
    );
    let short_cache = SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=300"));
    let files_cache = SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=3600"));
    let page = ServeDir::new(&cfg.web).not_found_service(ServeFile::new(cfg.web.join("404.html")));
    Router::new()
        .route("/latest.json", get(latest))
        .route("/download/:platform", get(download))
        .route("/_upload/*path", put(upload).delete(remove))
        .route("/_ls/*dir", get(list))
        .route("/healthz", get(|| async { "ok" }))
        .nest_service("/releases", ServeDir::new(cfg.data.join("releases")).precompressed_gzip().layer_files_cache(files_cache.clone()))
        .nest_service("/apt", ServeDir::new(cfg.data.join("apt")).layer_files_cache(files_cache))
        // Fichiers dont le nom change avec le contenu (CSS, polices) : cache d’un an ; captures d’écran : un jour.
        .nest_service("/_astro", ServeDir::new(cfg.web.join("_astro")).layer_files_cache(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"))))
        .nest_service("/shots", ServeDir::new(cfg.web.join("shots")).layer_files_cache(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400"))))
        .fallback_service(page)
        .layer(short_cache)
        .layer(nosniff)
        .layer(referrer)
        .layer(frame)
        .layer(csp)
        .with_state(cfg)
}

/// Petit confort : applique un en-tête de cache à un service de fichiers.
trait LayerFilesCache: Sized {
    fn layer_files_cache(self, l: SetResponseHeaderLayer<HeaderValue>) -> tower_http::set_header::SetResponseHeader<Self, HeaderValue>;
}
impl<S> LayerFilesCache for S {
    fn layer_files_cache(self, l: SetResponseHeaderLayer<HeaderValue>) -> tower_http::set_header::SetResponseHeader<Self, HeaderValue> {
        use tower::Layer as _;
        l.layer(self)
    }
}

// ---------------------------------------------------------------------------------------------
// Chemins et jeton
// ---------------------------------------------------------------------------------------------

/// Chemin relatif sûr dans `DATA_DIR` : sous `releases/` ou `apt/`, composants simples uniquement.
pub fn safe_rel(path: &str) -> Option<PathBuf> {
    if path.is_empty() || path.len() > 240 || path.contains("//") || path.ends_with('/') {
        return None;
    }
    let p = PathBuf::from(path);
    let mut depth = 0;
    for (i, c) in p.components().enumerate() {
        match c {
            Component::Normal(n) => {
                let n = n.to_str()?;
                if n.starts_with('.') || n.is_empty() || !n.chars().all(|ch| ch.is_ascii_alphanumeric() || "._+~-".contains(ch)) {
                    return None;
                }
                if i == 0 && n != "releases" && n != "apt" {
                    return None;
                }
                depth += 1;
            }
            _ => return None,
        }
    }
    (2..=8).contains(&depth).then_some(p)
}

fn authorized(cfg: &Config, headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(token) = cfg.token.as_deref().filter(|t| !t.is_empty()) else { return Err(StatusCode::NOT_FOUND) };
    let given = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
    // Comparaison en temps constant.
    let (a, b) = (token.as_bytes(), given.as_bytes());
    let mut diff = (a.len() ^ b.len()) as u8 | (a.len() != b.len()) as u8;
    for i in 0..a.len().min(b.len()) {
        diff |= a[i] ^ b[i];
    }
    if diff == 0 { Ok(()) } else { Err(StatusCode::UNAUTHORIZED) }
}

// ---------------------------------------------------------------------------------------------
// Dépôt de fichiers (intégration continue)
// ---------------------------------------------------------------------------------------------

async fn upload(State(cfg): State<Shared>, Path(path): Path<String>, headers: HeaderMap, body: Body) -> Result<StatusCode, StatusCode> {
    authorized(&cfg, &headers)?;
    let rel = safe_rel(&path).ok_or(StatusCode::BAD_REQUEST)?;
    if headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok()).is_some_and(|n| n > MAX_UPLOAD) {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let dest = cfg.data.join(&rel);
    let dir = dest.parent().ok_or(StatusCode::BAD_REQUEST)?;
    tokio::fs::create_dir_all(dir).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // Écriture dans un fichier temporaire puis renommage : un téléchargement ne voit jamais un fichier à moitié écrit.
    let tmp = dir.join(format!(".{}.part", dest.file_name().and_then(|n| n.to_str()).unwrap_or("f")));
    let mut file = tokio::fs::File::create(&tmp).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut stream = body.into_data_stream().map_err(|_| std::io::Error::other("lecture"));
    let mut total: u64 = 0;
    while let Some(chunk) = stream.try_next().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        total += chunk.len() as u64;
        if total > MAX_UPLOAD {
            drop(file);
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        file.write_all(&chunk).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    file.flush().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    drop(file);
    tokio::fs::rename(&tmp, &dest).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tracing::info!("déposé {} ({} octets)", rel.display(), total);
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(State(cfg): State<Shared>, Path(path): Path<String>, headers: HeaderMap) -> Result<StatusCode, StatusCode> {
    authorized(&cfg, &headers)?;
    let rel = safe_rel(&path).ok_or(StatusCode::BAD_REQUEST)?;
    match tokio::fs::remove_file(cfg.data.join(&rel)).await {
        Ok(()) => {
            tracing::info!("supprimé {}", rel.display());
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Noms des fichiers d'un dossier (pour que l'intégration continue retire les anciens paquets).
async fn list(State(cfg): State<Shared>, Path(dir): Path<String>, headers: HeaderMap) -> Result<Json<Vec<String>>, StatusCode> {
    authorized(&cfg, &headers)?;
    // « releases/android » : deux composants au moins, comme pour un fichier.
    let rel = safe_rel(&format!("{}/x", dir.trim_end_matches('/'))).ok_or(StatusCode::BAD_REQUEST)?;
    let dir = cfg.data.join(rel.parent().ok_or(StatusCode::BAD_REQUEST)?);
    let mut out = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if e.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                if let Some(n) = e.file_name().to_str().filter(|n| !n.starts_with('.')) {
                    out.push(n.to_string());
                }
            }
        }
    }
    out.sort();
    Ok(Json(out))
}

// ---------------------------------------------------------------------------------------------
// Dernières versions
// ---------------------------------------------------------------------------------------------

/// Adresse publique : `SITE_URL`, sinon déduite de l'hôte de la requête (derrière un proxy : X-Forwarded-*).
fn base_url(cfg: &Config, headers: &HeaderMap) -> String {
    if let Some(u) = &cfg.site_url {
        return u.trim_end_matches('/').to_string();
    }
    let h = |n: &str| headers.get(n).and_then(|v| v.to_str().ok()).map(|v| v.split(',').next().unwrap_or("").trim().to_string());
    let proto = h("x-forwarded-proto").filter(|p| p == "http" || p == "https").unwrap_or_else(|| "http".into());
    let host = h("x-forwarded-host").or_else(|| h("host")).filter(|x| x.chars().all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))).unwrap_or_else(|| "localhost".into());
    format!("{proto}://{host}")
}

/// Fiche d'une plateforme (`releases/<plateforme>/latest.json`, déposée par l'intégration continue) + adresse du fichier.
async fn platform(cfg: &Config, name: &str, base: &str) -> Option<Value> {
    let raw = tokio::fs::read_to_string(cfg.data.join("releases").join(name).join("latest.json")).await.ok()?;
    let mut v: Value = serde_json::from_str(&raw).ok()?;
    let file = v.get("file")?.as_str()?.to_string();
    // Nom de fichier simple seulement (jamais de chemin) : la fiche ne peut pas désigner autre chose qu'un fichier du dossier.
    if file.contains('/') || file.starts_with('.') || safe_rel(&format!("releases/{name}/{file}")).is_none() {
        return None;
    }
    v["url"] = Value::String(format!("{base}/releases/{name}/{file}"));
    Some(v)
}

async fn latest(State(cfg): State<Shared>, headers: HeaderMap) -> Response {
    let base = base_url(&cfg, &headers);
    let mut out = serde_json::Map::new();
    for p in PLATFORMS {
        if let Some(v) = platform(&cfg, p, &base).await {
            out.insert(p.to_string(), v);
        }
    }
    out.insert("site".into(), json!(base));
    ([(header::CACHE_CONTROL, "no-cache")], Json(Value::Object(out))).into_response()
}

async fn download(State(cfg): State<Shared>, Path(p): Path<String>, headers: HeaderMap) -> Response {
    let name = match p.as_str() {
        "deb" | "debian" | "ubuntu" => "linux",
        other => other,
    };
    if !PLATFORMS.contains(&name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match platform(&cfg, name, &base_url(&cfg, &headers)).await.and_then(|v| v["url"].as_str().map(str::to_string)) {
        Some(url) => ([(header::CACHE_CONTROL, "no-cache")], Redirect::temporary(&url)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests;
