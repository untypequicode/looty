//! `looty-latex-service`: the LaTeX compile service of the PC tunnel (docs/tunnel-protocol.md, section 13).
//!
//! `POST /compile` takes the project as a zip and answers a zip with `output.pdf` and `compile.log`. The
//! service runs `latexmk` (or the engine itself) in a throw-away directory with shell escape off and file
//! access limited to that directory, under a time limit, and removes the directory whatever happens.
//! Closing the connection (the instance cancelled) kills the compilation and its child processes.

#![allow(clippy::field_reassign_with_default)]
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path as AxPath, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{watch, Semaphore};

pub const ENGINES: [&str; 3] = ["pdflatex", "xelatex", "lualatex"];

#[derive(Clone)]
pub struct Config {
    /// Where latexmk and the engines are looked for before the PATH.
    pub bin_dir: Option<PathBuf>,
    /// Also search the PATH (tests turn it off).
    pub use_path: bool,
    /// Parent of the job directories.
    pub work_dir: PathBuf,
    /// Compilations at the same time; the next ones wait up to `queue_wait`, then get 503.
    pub slots: usize,
    pub queue_wait: Duration,
    /// Request body (the zip), in bytes.
    pub max_request: usize,
    /// Sum of the unpacked files, in bytes, and their number.
    pub max_unpacked: u64,
    pub max_files: usize,
    /// Largest PDF returned, in bytes.
    pub max_pdf: u64,
    pub default_timeout: Duration,
    pub max_timeout: Duration,
    /// Tail of the log kept, in bytes.
    pub log_limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bin_dir: None,
            use_path: true,
            work_dir: std::env::temp_dir(),
            slots: 2,
            queue_wait: Duration::from_secs(15),
            max_request: 16 * 1024 * 1024,
            max_unpacked: 128 * 1024 * 1024,
            max_files: 5000,
            max_pdf: 48 * 1024 * 1024,
            default_timeout: Duration::from_secs(120),
            max_timeout: Duration::from_secs(300),
            log_limit: 400_000,
        }
    }
}

struct Done {
    archive: Vec<u8>,
    pdf: bool,
    engine: String,
    ms: u64,
}

type Outcome = Result<Done, (StatusCode, &'static str)>;

/// A compilation started with `?async=1`: polled by `GET /jobs/{id}`, cancelled by `DELETE`.
struct Entry {
    abort: tokio::task::AbortHandle,
    rx: watch::Receiver<Option<Arc<Outcome>>>,
    polled: Instant,
}

struct Shared {
    cfg: Config,
    slots: Semaphore,
    jobs: Mutex<HashMap<String, Entry>>,
}

/// Jobs kept at once (running, queued, or finished and not collected yet).
const MAX_JOBS: usize = 16;
/// A running job nobody polls for this long is cancelled (the instance went away).
const ABANDONED: Duration = Duration::from_secs(60);
/// A finished job nobody collected is forgotten after this long.
const KEEP_FINISHED: Duration = Duration::from_secs(120);
/// Longest `wait` of a poll, below the 30 s the tunnel allows before the first byte of a response.
const MAX_WAIT: u64 = 20;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn router(cfg: Config) -> Router {
    let limit = cfg.max_request;
    let shared = Arc::new(Shared {
        slots: Semaphore::new(cfg.slots.max(1)),
        cfg,
        jobs: Mutex::new(HashMap::new()),
    });
    // Reaper of abandoned and stale jobs; it stops when the router is dropped.
    let weak = Arc::downgrade(&shared);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let Some(s) = weak.upgrade() else { return };
            lock(&s.jobs).retain(|_, e| {
                let idle = e.polled.elapsed();
                let done = e.rx.borrow().is_some();
                let keep = if done {
                    idle < KEEP_FINISHED
                } else {
                    idle < ABANDONED
                };
                if !keep {
                    e.abort.abort();
                }
                keep
            });
        }
    });
    Router::new()
        .route("/health", get(health))
        .route("/compile", post(compile))
        .route("/jobs/{id}", get(job_get).delete(job_delete))
        .layer(DefaultBodyLimit::max(limit))
        .with_state(shared)
}

fn reply(o: &Outcome) -> Response {
    match o {
        Err((status, code)) => error(*status, code),
        Ok(d) => {
            let mut h = HeaderMap::new();
            h.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/zip"),
            );
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            h.insert(
                "x-looty-pdf",
                HeaderValue::from_static(if d.pdf { "1" } else { "0" }),
            );
            h.insert(
                "x-looty-engine",
                HeaderValue::from_str(&d.engine).unwrap_or(HeaderValue::from_static("")),
            );
            h.insert("x-looty-duration-ms", HeaderValue::from(d.ms));
            (StatusCode::OK, h, d.archive.clone()).into_response()
        }
    }
}

fn random_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        h.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

async fn compile(State(s): State<Arc<Shared>>, uri: Uri, body: Bytes) -> Response {
    let q = params(&uri);
    if !q.iter().any(|(k, v)| k == "async" && v == "1") {
        return reply(&execute(s, q, body).await);
    }
    // Asynchronous: answer at once, the compilation goes on after the request, which keeps every
    // request of the tunnel short (the tunnel waits 30 s for a first byte).
    let id = random_id();
    let (tx, rx) = watch::channel(None);
    let mut jobs = lock(&s.jobs);
    if jobs.len() >= MAX_JOBS {
        return error(StatusCode::SERVICE_UNAVAILABLE, "latex_busy");
    }
    let s2 = s.clone();
    let task = tokio::spawn(async move {
        let r = execute(s2, q, body).await;
        let _ = tx.send(Some(Arc::new(r)));
    });
    jobs.insert(
        id.clone(),
        Entry {
            abort: task.abort_handle(),
            rx,
            polled: Instant::now(),
        },
    );
    (
        StatusCode::ACCEPTED,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        format!("{{\"id\":\"{id}\",\"status\":\"running\"}}"),
    )
        .into_response()
}

/// `GET /jobs/{id}?wait=N`: the result (200 zip, or the error of the job), or 202 `running` after at most `wait` seconds.
async fn job_get(State(s): State<Arc<Shared>>, AxPath(id): AxPath<String>, uri: Uri) -> Response {
    let wait = params(&uri)
        .iter()
        .find(|(k, _)| k == "wait")
        .and_then(|(_, v)| v.parse::<u64>().ok())
        .unwrap_or(0)
        .min(MAX_WAIT);
    let mut rx = {
        let mut jobs = lock(&s.jobs);
        let Some(e) = jobs.get_mut(&id) else {
            return error(StatusCode::NOT_FOUND, "latex_job_unknown");
        };
        e.polled = Instant::now();
        e.rx.clone()
    };
    let _ = tokio::time::timeout(Duration::from_secs(wait), rx.wait_for(|v| v.is_some())).await;
    let current = rx.borrow().clone();
    if let Some(o) = current {
        return reply(&o);
    }
    if rx.has_changed().is_err() {
        // The task is gone without a result: cancelled or crashed.
        return error(StatusCode::INTERNAL_SERVER_ERROR, "latex_internal");
    }
    if let Some(e) = lock(&s.jobs).get_mut(&id) {
        e.polled = Instant::now();
    }
    (
        StatusCode::ACCEPTED,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        "{\"status\":\"running\"}",
    )
        .into_response()
}

/// `DELETE /jobs/{id}`: cancels the compilation (processes killed, directory removed) and forgets the job.
async fn job_delete(State(s): State<Arc<Shared>>, AxPath(id): AxPath<String>) -> Response {
    match lock(&s.jobs).remove(&id) {
        Some(e) => {
            e.abort.abort();
            StatusCode::NO_CONTENT.into_response()
        }
        None => error(StatusCode::NOT_FOUND, "latex_job_unknown"),
    }
}

fn error(status: StatusCode, code: &str) -> Response {
    (
        status,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        format!("{{\"error\":\"{code}\"}}"),
    )
        .into_response()
}

fn find(cfg: &Config, bin: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = cfg.bin_dir.iter().cloned().collect();
    if let Some(path) = std::env::var_os("PATH").filter(|_| cfg.use_path) {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter().map(|d| d.join(bin)).find(|p| p.is_file())
}

async fn health(State(s): State<Arc<Shared>>) -> Response {
    let engines: Vec<&str> = ENGINES
        .iter()
        .copied()
        .filter(|e| find(&s.cfg, e).is_some())
        .collect();
    let body = format!(
        "{{\"service\":\"looty-latex\",\"version\":\"{}\",\"api\":1,\"engines\":{},\"latexmk\":{},\"max_request\":{},\"max_timeout\":{}}}",
        env!("CARGO_PKG_VERSION"),
        serde_json::to_string(&engines).unwrap_or_else(|_| "[]".into()),
        find(&s.cfg, "latexmk").is_some(),
        s.cfg.max_request,
        s.cfg.max_timeout.as_secs()
    );
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// A relative path made of plain names only: no `..`, no root, no backslash, no control character.
pub fn safe_rel(p: &str) -> Option<PathBuf> {
    if p.is_empty() || p.len() > 512 || p.contains('\\') || p.chars().any(|c| c.is_control()) {
        return None;
    }
    let path = Path::new(p);
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(n) => out.push(n),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// Throw-away directory of one job, removed on drop (also when the request is cancelled).
struct Job {
    dir: PathBuf,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

impl Job {
    fn new(base: &Path) -> std::io::Result<Job> {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = base.join(format!("looty-latex-{}-{t:x}-{n}", std::process::id()));
        std::fs::create_dir_all(dir.join("src"))?;
        Ok(Job { dir })
    }
    fn src(&self) -> PathBuf {
        self.dir.join("src")
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Kills the whole process group of a tool when dropped (timeout, cancellation).
struct Reaper(Option<u32>);

impl Drop for Reaper {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.0 {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", "--", &format!("-{pid}")])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
}

enum Fail {
    Timeout,
    Spawn,
}

/// Runs one tool in `dir`, its output appended to `out`. Returns whether it exited with success.
async fn run(
    dir: &Path,
    out: &Path,
    program: &Path,
    args: &[&str],
    deadline: Instant,
) -> Result<bool, Fail> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out)
        .map_err(|_| Fail::Spawn)?;
    let err = file.try_clone().map_err(|_| Fail::Spawn)?;
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .current_dir(dir)
        .env("openin_any", "p")
        .env("openout_any", "p")
        .env("shell_escape", "f")
        .env("HOME", dir)
        .env("TEXMFVAR", dir.join(".texmf-var"))
        .env("TEXMFCACHE", dir.join(".texmf-var"))
        .env("max_print_line", "1000")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(file))
        .stderr(std::process::Stdio::from(err))
        .kill_on_drop(true);
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd.spawn().map_err(|_| Fail::Spawn)?;
    let _reaper = Reaper(child.id());
    let left = deadline.saturating_duration_since(Instant::now());
    match tokio::time::timeout(left, child.wait()).await {
        Ok(Ok(st)) => Ok(st.success()),
        Ok(Err(_)) => Err(Fail::Spawn),
        Err(_) => {
            let _ = child.kill().await;
            Err(Fail::Timeout)
        }
    }
}

/// Query parameters (`main`, `engine`, `timeout`); unknown ones are ignored.
fn params(uri: &Uri) -> Vec<(String, String)> {
    url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// Unpacks the project zip into `dir`; every limit is checked on the bytes really read, not on the sizes the zip claims.
fn unpack(bytes: &[u8], dir: &Path, cfg: &Config) -> Result<(), &'static str> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "latex_bad_request")?;
    if zip.len() > cfg.max_files {
        return Err("latex_too_large");
    }
    let mut total: u64 = 0;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|_| "latex_bad_request")?;
        let Some(rel) = safe_rel(
            f.name()
                .map_err(|_| "latex_bad_request")?
                .trim_end_matches('/'),
        ) else {
            return Err("latex_bad_request");
        };
        // Links are never created: a link could point outside the job directory.
        if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            continue;
        }
        let target = dir.join(&rel);
        if f.is_dir() {
            std::fs::create_dir_all(&target).map_err(|_| "latex_internal")?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|_| "latex_internal")?;
        }
        let mut data = Vec::new();
        f.by_ref()
            .take(cfg.max_unpacked - total + 1)
            .read_to_end(&mut data)
            .map_err(|_| "latex_bad_request")?;
        total += data.len() as u64;
        if total > cfg.max_unpacked {
            return Err("latex_too_large");
        }
        std::fs::write(&target, data).map_err(|_| "latex_internal")?;
    }
    Ok(())
}

fn tail(mut s: String, limit: usize) -> String {
    if s.len() > limit {
        let mut cut = s.len() - limit;
        while !s.is_char_boundary(cut) {
            cut += 1;
        }
        s = s[cut..].to_string();
    }
    s
}

async fn needs_rerun(src: &Path, base: &str) -> bool {
    let log = tokio::fs::read_to_string(src.join(format!("{base}.log")))
        .await
        .unwrap_or_default();
    [
        "Rerun to get",
        "Label(s) may have changed",
        "Please rerun",
        "Rerun LaTeX",
    ]
    .iter()
    .any(|k| log.contains(k))
}

async fn execute(s: Arc<Shared>, q: Vec<(String, String)>, body: Bytes) -> Outcome {
    let cfg = &s.cfg;
    let get = |k: &str| q.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    let main = get("main").unwrap_or("main.tex");
    let engine = get("engine").unwrap_or("pdflatex");
    let (Some(main_rel), true) = (safe_rel(main), ENGINES.contains(&engine)) else {
        return Err((StatusCode::BAD_REQUEST, "latex_bad_request"));
    };
    if main_rel.extension().and_then(|e| e.to_str()) != Some("tex") {
        return Err((StatusCode::BAD_REQUEST, "latex_bad_request"));
    }
    let timeout = match get("timeout").map(str::parse::<u64>) {
        None => cfg.default_timeout,
        Some(Ok(t)) if t >= 1 => Duration::from_secs(t).min(cfg.max_timeout),
        Some(_) => return Err((StatusCode::BAD_REQUEST, "latex_bad_request")),
    };
    let Some(engine_bin) = find(cfg, engine) else {
        return Err((StatusCode::NOT_IMPLEMENTED, "latex_engine_missing"));
    };
    let _slot = match tokio::time::timeout(cfg.queue_wait, s.slots.acquire()).await {
        Ok(Ok(p)) => p,
        _ => return Err((StatusCode::SERVICE_UNAVAILABLE, "latex_busy")),
    };
    let started = Instant::now();
    let deadline = started + timeout;
    let Ok(job) = Job::new(&cfg.work_dir) else {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "latex_internal"));
    };
    let src = job.src();
    let out_file = job.dir.join("out.txt");

    let (bytes, dir, c) = (body, src.clone(), cfg.clone());
    match tokio::task::spawn_blocking(move || unpack(&bytes, &dir, &c)).await {
        Ok(Ok(())) => {}
        Ok(Err(code)) => {
            let status = if code == "latex_too_large" {
                StatusCode::PAYLOAD_TOO_LARGE
            } else if code == "latex_internal" {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::BAD_REQUEST
            };
            return Err((status, code));
        }
        Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "latex_internal")),
    }
    if !src.join(&main_rel).is_file() {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "latex_no_main"));
    }
    let main_s = main_rel.to_string_lossy().replace('\\', "/");
    let base = main_rel
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let flags = [
        "-interaction=nonstopmode",
        "-file-line-error",
        "-no-shell-escape",
        "-halt-on-error",
    ];

    let result: Result<(), Fail> = async {
        if let Some(latexmk) = find(cfg, "latexmk") {
            let mode = match engine {
                "xelatex" => "-xelatex",
                "lualatex" => "-lualatex",
                _ => "-pdf",
            };
            let mut args = vec![mode];
            args.extend_from_slice(&flags);
            args.push(&main_s);
            run(&src, &out_file, &latexmk, &args, deadline).await?;
        } else {
            let mut args: Vec<&str> = flags.to_vec();
            args.push(&main_s);
            let mut ok = run(&src, &out_file, &engine_bin, &args, deadline).await?;
            let aux = tokio::fs::read_to_string(src.join(format!("{base}.aux")))
                .await
                .unwrap_or_default();
            let mut extra = 0;
            if ok && src.join(format!("{base}.bcf")).exists() {
                if let Some(biber) = find(cfg, "biber") {
                    run(&src, &out_file, &biber, &[&base], deadline).await?;
                    extra = 2;
                }
            } else if ok && aux.contains("\\bibdata") {
                if let Some(bibtex) = find(cfg, "bibtex") {
                    run(&src, &out_file, &bibtex, &[&base], deadline).await?;
                    extra = 2;
                }
            }
            let mut passes = 0;
            while ok && passes < 3 && (extra > 0 || needs_rerun(&src, &base).await) {
                ok = run(&src, &out_file, &engine_bin, &args, deadline).await?;
                extra = u32::saturating_sub(extra, 1);
                passes += 1;
            }
        }
        Ok(())
    }
    .await;
    match result {
        Err(Fail::Timeout) => return Err((StatusCode::REQUEST_TIMEOUT, "latex_timeout")),
        Err(Fail::Spawn) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "latex_internal")),
        Ok(()) => {}
    }

    let log = match tokio::fs::read_to_string(src.join(format!("{base}.log"))).await {
        Ok(l) if !l.is_empty() => l,
        _ => String::from_utf8_lossy(&tokio::fs::read(&out_file).await.unwrap_or_default())
            .into_owned(),
    };
    let log = tail(log, cfg.log_limit);
    let pdf = match tokio::fs::metadata(src.join(format!("{base}.pdf"))).await {
        Ok(m) if m.len() > cfg.max_pdf => {
            return Err((StatusCode::PAYLOAD_TOO_LARGE, "latex_too_large"))
        }
        Ok(m) if m.len() > 0 => tokio::fs::read(src.join(format!("{base}.pdf"))).await.ok(),
        _ => None,
    };
    let produced = pdf.is_some();
    let archive = (|| -> Option<Vec<u8>> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        if let Some(pdf) = &pdf {
            w.start_file(
                "output.pdf",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored)
                    .large_file(true),
            )
            .ok()?;
            w.write_all(pdf).ok()?;
        }
        w.start_file(
            "compile.log",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .ok()?;
        w.write_all(log.as_bytes()).ok()?;
        Some(w.finish().ok()?.into_inner())
    })();
    let Some(archive) = archive else {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "latex_internal"));
    };
    Ok(Done {
        archive,
        pdf: produced,
        engine: engine.to_string(),
        ms: started.elapsed().as_millis() as u64,
    })
}

#[cfg(all(test, unix))]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (n, d) in files {
            w.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(d).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    /// Fake engine: writes `<main stem>.pdf` and `.log` from the main file, fails if it contains FAIL, sleeps if it contains SLEEP.
    fn fake_bin(dir: &Path, name: &str) {
        let script = "#!/bin/sh\nfor last; do :; done\nstem=${last%.tex}\nstem=${stem##*/}\nif grep -q SLEEP \"$last\"; then sleep 30; fi\necho \"This is fake $0 $*\"\nif grep -q FAIL \"$last\"; then echo \"./$last:1: Undefined control sequence.\" > \"$stem.log\"; exit 1; fi\necho '%PDF-fake' > \"$stem.pdf\"\necho \"ok $*\" > \"$stem.log\"\n";
        let p = dir.join(name);
        std::fs::write(&p, script).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    async fn serve(cfg: Config) -> String {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(l, router(cfg)).await.unwrap() });
        format!("127.0.0.1:{}", addr.port())
    }

    async fn post(addr: &str, query: &str, body: &[u8]) -> (u16, Vec<u8>) {
        call(addr, "POST", &format!("/compile{query}"), body).await
    }

    async fn call(addr: &str, method: &str, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        let head = format!("{method} {path} HTTP/1.1\r\nhost: x\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
        s.write_all(head.as_bytes()).await.unwrap();
        s.write_all(body).await.unwrap();
        let mut resp = Vec::new();
        s.read_to_end(&mut resp).await.unwrap();
        let split = resp.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let status: u16 = String::from_utf8_lossy(&resp[..split])
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        (status, resp[split + 4..].to_vec())
    }

    fn setup() -> (tempdir::Dir, Config) {
        let d = tempdir::Dir::new();
        fake_bin(&d.0, "pdflatex");
        fake_bin(&d.0, "latexmk");
        let mut cfg = Config::default();
        cfg.bin_dir = Some(d.0.clone());
        cfg.use_path = false;
        cfg.work_dir = d.0.join("work");
        std::fs::create_dir_all(&cfg.work_dir).unwrap();
        (d, cfg)
    }

    mod tempdir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new() -> Dir {
                let p = std::env::temp_dir().join(format!(
                    "looty-latex-test-{}-{:x}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    fn entries(zipped: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut z = zip::ZipArchive::new(Cursor::new(zipped)).unwrap();
        (0..z.len())
            .map(|i| {
                let mut f = z.by_index(i).unwrap();
                let mut d = Vec::new();
                f.read_to_end(&mut d).unwrap();
                (f.name().unwrap().to_string(), d)
            })
            .collect()
    }

    #[test]
    fn paths() {
        assert!(safe_rel("main.tex").is_some());
        assert!(safe_rel("./a/b.tex").is_some());
        for bad in [
            "",
            "../x.tex",
            "a/../../x",
            "/etc/passwd",
            "a\\b",
            "a\0b",
            "..",
        ] {
            assert!(safe_rel(bad).is_none(), "{bad}");
        }
    }

    #[tokio::test]
    async fn compiles_and_cleans_up() {
        let (d, cfg) = setup();
        let work = cfg.work_dir.clone();
        let addr = serve(cfg).await;
        let (st, body) = post(
            &addr,
            "?main=chap/main.tex&engine=pdflatex",
            &zip_of(&[("chap/main.tex", b"hello"), ("a.bib", b"x")]),
        )
        .await;
        assert_eq!(st, 200);
        let e = entries(&body);
        assert_eq!(e[0].0, "output.pdf");
        assert!(e[0].1.starts_with(b"%PDF"));
        assert!(String::from_utf8_lossy(&e[1].1).contains("-no-shell-escape"));
        assert_eq!(
            std::fs::read_dir(&work).unwrap().count(),
            0,
            "job directory removed"
        );
        // A failing compilation still answers 200 with the log and no PDF.
        let (st, body) = post(&addr, "", &zip_of(&[("main.tex", b"FAIL")])).await;
        assert_eq!(st, 200);
        let e = entries(&body);
        assert_eq!(e.len(), 1);
        assert!(String::from_utf8_lossy(&e[0].1).contains("Undefined control sequence"));
        drop(d);
    }

    #[tokio::test]
    async fn refuses_bad_requests() {
        let (_d, mut cfg) = setup();
        cfg.max_files = 3;
        cfg.max_unpacked = 100;
        let addr = serve(cfg).await;
        let ok = zip_of(&[("main.tex", b"x")]);
        assert_eq!(post(&addr, "?main=../main.tex", &ok).await.0, 400);
        assert_eq!(post(&addr, "?engine=evil", &ok).await.0, 400);
        assert_eq!(post(&addr, "?main=main.sh", &ok).await.0, 400);
        assert_eq!(post(&addr, "?timeout=abc", &ok).await.0, 400);
        assert_eq!(post(&addr, "", b"not a zip").await.0, 400);
        assert_eq!(post(&addr, "?main=other.tex", &ok).await.0, 422);
        assert_eq!(
            post(&addr, "?engine=xelatex", &ok).await.0,
            501,
            "engine not installed"
        );
        // Zip slip, too many files, too much data.
        assert_eq!(
            post(
                &addr,
                "",
                &zip_of(&[("../evil.tex", b"x"), ("main.tex", b"x")])
            )
            .await
            .0,
            400
        );
        assert_eq!(
            post(
                &addr,
                "",
                &zip_of(&[("main.tex", b"x"), ("a", b"1"), ("b", b"1"), ("c", b"1")])
            )
            .await
            .0,
            413
        );
        assert_eq!(
            post(&addr, "", &zip_of(&[("main.tex", &[b'a'; 200])]))
                .await
                .0,
            413
        );
    }

    #[tokio::test]
    async fn times_out_and_kills() {
        let (_d, mut cfg) = setup();
        cfg.default_timeout = Duration::from_secs(1);
        let work = cfg.work_dir.clone();
        let addr = serve(cfg).await;
        let t = Instant::now();
        let (st, body) = post(&addr, "?timeout=1", &zip_of(&[("main.tex", b"SLEEP")])).await;
        assert_eq!(st, 408);
        assert!(String::from_utf8_lossy(&body).contains("latex_timeout"));
        assert!(t.elapsed() < Duration::from_secs(10));
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(std::fs::read_dir(&work).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn health_lists_engines() {
        let (_d, cfg) = setup();
        let addr = serve(cfg).await;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(&addr).await.unwrap();
        s.write_all(b"GET /health HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut r = String::new();
        s.read_to_string(&mut r).await.unwrap();
        assert!(
            r.contains("\"engines\":[\"pdflatex\"]") && r.contains("\"latexmk\":true"),
            "{r}"
        );
    }

    #[tokio::test]
    async fn async_jobs_poll_and_cancel() {
        let (_d, cfg) = setup();
        let work = cfg.work_dir.clone();
        let addr = serve(cfg).await;
        // Finished job: poll returns the zip.
        let (st, b) = post(&addr, "?async=1", &zip_of(&[("main.tex", b"hello")])).await;
        assert_eq!(st, 202);
        let id = String::from_utf8_lossy(&b)
            .split('"')
            .nth(3)
            .unwrap()
            .to_string();
        let (st, body) = call(&addr, "GET", &format!("/jobs/{id}?wait=10"), b"").await;
        assert_eq!(st, 200);
        assert_eq!(entries(&body)[0].0, "output.pdf");
        assert_eq!(
            call(&addr, "DELETE", &format!("/jobs/{id}"), b"").await.0,
            204
        );
        assert_eq!(call(&addr, "GET", &format!("/jobs/{id}"), b"").await.0, 404);
        // Running job: 202 then cancelled, nothing left on disk.
        let (_, b) = post(&addr, "?async=1", &zip_of(&[("main.tex", b"SLEEP")])).await;
        let id = String::from_utf8_lossy(&b)
            .split('"')
            .nth(3)
            .unwrap()
            .to_string();
        assert_eq!(
            call(&addr, "GET", &format!("/jobs/{id}?wait=1"), b"")
                .await
                .0,
            202
        );
        assert_eq!(
            call(&addr, "DELETE", &format!("/jobs/{id}"), b"").await.0,
            204
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(std::fs::read_dir(&work).unwrap().count(), 0);
        // A failing job reports its error through the poll.
        let (_, b) = post(
            &addr,
            "?async=1&main=nope.tex",
            &zip_of(&[("main.tex", b"x")]),
        )
        .await;
        let id = String::from_utf8_lossy(&b)
            .split('"')
            .nth(3)
            .unwrap()
            .to_string();
        assert_eq!(
            call(&addr, "GET", &format!("/jobs/{id}?wait=5"), b"")
                .await
                .0,
            422
        );
    }
}
