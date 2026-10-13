use super::*;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;

fn cfg(dir: &std::path::Path) -> Config {
    std::fs::create_dir_all(dir.join("web")).unwrap();
    std::fs::write(dir.join("web/index.html"), "<h1>Looty</h1>").unwrap();
    std::fs::write(dir.join("web/404.html"), "<h1>404</h1>").unwrap();
    Config { web: dir.join("web"), data: dir.join("data"), token: Some("secret".into()), site_url: None }
}

fn tmp() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("looty-site-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

async fn call(app: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, String) {
    let r = app.clone().oneshot(req).await.unwrap();
    let (s, h) = (r.status(), r.headers().clone());
    (s, h, String::from_utf8_lossy(&r.into_body().collect().await.unwrap().to_bytes()).into_owned())
}

fn put(path: &str, token: Option<&str>, body: &str) -> Request<Body> {
    let mut b = Request::builder().method("PUT").uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

#[test]
fn safe_paths() {
    assert!(safe_rel("releases/android/looty-0.12.0.apk").is_some());
    assert!(safe_rel("apt/dists/stable/main/binary-amd64/Packages.gz").is_some());
    for bad in ["", "releases", "other/x/y", "releases/../etc/passwd", "/releases/a/b", "releases/a/.hidden", "releases/a/b c", "releases/a/b;rm", "releases//a/b", "releases/a/b/c/d/e/f/g/h", "apt/x\\y"] {
        assert!(safe_rel(bad).is_none(), "{bad} devrait être refusé");
    }
}

#[tokio::test]
async fn upload_needs_the_token() {
    let d = tmp();
    let app = app(cfg(&d));
    assert_eq!(call(&app, put("/_upload/releases/android/a.apk", None, "x")).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, put("/_upload/releases/android/a.apk", Some("wrong"), "x")).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, put("/_upload/releases/android/a.apk", Some("secret"), "hello")).await.0, StatusCode::NO_CONTENT);
    assert_eq!(std::fs::read_to_string(d.join("data/releases/android/a.apk")).unwrap(), "hello");
    // chemin hors des dossiers publiés
    assert_eq!(call(&app, put("/_upload/web/index.html", Some("secret"), "x")).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&app, put("/_upload/releases/android/..%2Fx", Some("secret"), "x")).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn uploads_disabled_without_a_token() {
    let d = tmp();
    let mut c = cfg(&d);
    c.token = None;
    let app = app(c);
    assert_eq!(call(&app, put("/_upload/releases/android/a.apk", Some("x"), "x")).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn latest_json_and_download_links() {
    let d = tmp();
    let app = app(cfg(&d));
    // rien publié : objet sans plateforme, téléchargement introuvable
    let (s, _, body) = call(&app, Request::get("/latest.json").header("host", "looty.test").body(Body::empty()).unwrap()).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["site"], "http://looty.test");
    assert_eq!(call(&app, Request::get("/download/android").body(Body::empty()).unwrap()).await.0, StatusCode::NOT_FOUND);
    // publication d'un APK et de sa fiche
    call(&app, put("/_upload/releases/android/looty-0.12.0.apk", Some("secret"), "APKDATA")).await;
    let meta = r#"{"version":"0.12.0","date":"2026-10-13","file":"looty-0.12.0.apk","sha256":"abc","size":7,"title":{"en":"T","fr":"T"},"notes":{"en":["n"],"fr":["n"]}}"#;
    assert_eq!(call(&app, put("/_upload/releases/android/latest.json", Some("secret"), meta)).await.0, StatusCode::NO_CONTENT);
    let req = Request::get("/latest.json").header("host", "looty.test").header("x-forwarded-proto", "https").body(Body::empty()).unwrap();
    let (_, h, body) = call(&app, req).await;
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["android"]["version"], "0.12.0");
    assert_eq!(v["android"]["url"], "https://looty.test/releases/android/looty-0.12.0.apk");
    assert_eq!(h[header::CACHE_CONTROL], "no-cache");
    // lien stable → redirection vers le dernier fichier ; le fichier est servi
    let (s, h, _) = call(&app, Request::get("/download/android").header("host", "looty.test").body(Body::empty()).unwrap()).await;
    assert_eq!(s, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(h[header::LOCATION], "http://looty.test/releases/android/looty-0.12.0.apk");
    let (s, _, body) = call(&app, Request::get("/releases/android/looty-0.12.0.apk").body(Body::empty()).unwrap()).await;
    assert_eq!((s, body.as_str()), (StatusCode::OK, "APKDATA"));
    // une fiche qui désigne un chemin est ignorée
    let evil = r#"{"version":"9","file":"../../etc/passwd"}"#;
    call(&app, put("/_upload/releases/windows/latest.json", Some("secret"), evil)).await;
    let (_, _, body) = call(&app, Request::get("/latest.json").body(Body::empty()).unwrap()).await;
    assert!(serde_json::from_str::<Value>(&body).unwrap().get("windows").is_none());
}

#[tokio::test]
async fn pages_headers_and_listing() {
    let d = tmp();
    let app = app(cfg(&d));
    let (s, h, body) = call(&app, Request::get("/").body(Body::empty()).unwrap()).await;
    assert_eq!((s, body.as_str()), (StatusCode::OK, "<h1>Looty</h1>"));
    assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(h.contains_key(header::CONTENT_SECURITY_POLICY));
    assert_eq!(call(&app, Request::get("/nope").body(Body::empty()).unwrap()).await.0, StatusCode::NOT_FOUND);
    call(&app, put("/_upload/releases/android/b.apk", Some("secret"), "1")).await;
    call(&app, put("/_upload/releases/android/a.apk", Some("secret"), "1")).await;
    let ls = |tok: &str| Request::get("/_ls/releases/android").header("authorization", format!("Bearer {tok}")).body(Body::empty()).unwrap();
    let (s, _, body) = call(&app, ls("secret")).await;
    assert_eq!((s, body.as_str()), (StatusCode::OK, r#"["a.apk","b.apk"]"#));
    assert_eq!(call(&app, ls("nope")).await.0, StatusCode::UNAUTHORIZED);
    let del = Request::builder().method("DELETE").uri("/_upload/releases/android/a.apk").header("authorization", "Bearer secret").body(Body::empty()).unwrap();
    assert_eq!(call(&app, del).await.0, StatusCode::NO_CONTENT);
    assert!(!d.join("data/releases/android/a.apk").exists());
}
