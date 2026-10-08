//! Compilation LaTeX sur le serveur.
//!
//! Les fichiers du projet sont écrits dans un dossier temporaire puis compilés :
//! `latexmk` s'il est installé, sinon le moteur choisi (pdflatex, xelatex, lualatex)
//! lancé autant de fois que nécessaire, avec bibtex/biber pour la bibliographie.
//!
//! Sécurité : jamais de shell-escape ; lecture et écriture limitées au dossier du projet
//! (`openin_any=p`, `openout_any=p`) ; une minute au plus par passe ; deux compilations
//! simultanées au plus. Moteurs cherchés dans `LATEX_BIN_DIR` puis dans le PATH.

use axum::{
    body::Body,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::access::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::projects::{all_files, file_bytes, project_for, Need};
use crate::state::AppState;

static SLOTS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));
const PASS_TIMEOUT: Duration = Duration::from_secs(60);

fn output_path(project: Uuid) -> PathBuf {
    PathBuf::from(crate::upload::UPLOAD_DIR).join("projects").join(project.to_string()).join("output.pdf")
}

/// Exécutable trouvé (LATEX_BIN_DIR, puis PATH).
fn find(bin: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var("LATEX_BIN_DIR").ok().map(|d| vec![PathBuf::from(d)]).unwrap_or_default();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter().map(|d| d.join(bin)).find(|p| p.is_file())
}

#[derive(Serialize, Clone)]
pub struct Issue {
    file: Option<String>,
    line: Option<u32>,
    message: String,
    level: &'static str,
}

/// Erreurs et avertissements du journal (`-file-line-error`).
fn parse_log(log: &str) -> Vec<Issue> {
    let mut out: Vec<Issue> = Vec::new();
    let lines: Vec<&str> = log.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        // ./chapitres/intro.tex:12: Undefined control sequence.
        if let Some((file, rest)) = l.split_once(".tex:").map(|(f, r)| (format!("{f}.tex"), r)) {
            if let Some((num, msg)) = rest.split_once(": ") {
                if let Ok(n) = num.parse::<u32>() {
                    let mut message = msg.trim().to_string();
                    // Contexte « l.12 \foo » sur les lignes suivantes.
                    if let Some(ctx) = lines.iter().skip(i + 1).take(4).find(|x| x.starts_with("l.")) {
                        message = format!("{message} — {}", ctx.trim());
                    }
                    out.push(Issue { file: Some(file.trim_start_matches("./").to_string()), line: Some(n), message, level: "error" });
                    continue;
                }
            }
        }
        if let Some(msg) = l.strip_prefix("! ") {
            if !out.iter().any(|e| e.message.starts_with(msg.trim())) {
                out.push(Issue { file: None, line: None, message: msg.trim().to_string(), level: "error" });
            }
            continue;
        }
        if l.contains("LaTeX Warning:") || l.contains("Package") && l.contains("Warning:") {
            let mut msg = l.trim().to_string();
            let mut j = i + 1;
            while j < lines.len() && j < i + 3 && !lines[j].trim().is_empty() && lines[j].starts_with(' ') {
                msg.push(' ');
                msg.push_str(lines[j].trim());
                j += 1;
            }
            let line = msg.rsplit_once("input line ").and_then(|(_, r)| r.trim_end_matches('.').split(|c: char| !c.is_ascii_digit()).next()?.parse().ok());
            out.push(Issue { file: None, line, message: msg, level: "warning" });
        }
    }
    out.truncate(200);
    out
}

async fn run(dir: &FsPath, program: &FsPath, args: &[&str]) -> ApiResult<(bool, String)> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .current_dir(dir)
        .env("openin_any", "p")
        .env("openout_any", "p")
        .env("shell_escape", "f")
        .env("HOME", dir)
        .env("TEXMFVAR", dir.join(".texmf-var"))
        .env("max_print_line", "1000")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| ApiError::Internal(format!("latex spawn: {e}")))?;
    match tokio::time::timeout(PASS_TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(o)) => Ok((o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))),
        Ok(Err(e)) => Err(ApiError::Internal(format!("latex: {e}"))),
        Err(_) => Err(ApiError::bad_request("latex_timeout")),
    }
}

#[derive(Deserialize, Default)]
pub struct CompileReq {
    /// Contenu actuel des fichiers ouverts (id → texte), plus récent que la base.
    #[serde(default)]
    overrides: HashMap<Uuid, String>,
}

pub async fn compile(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, body: Option<Json<CompileReq>>) -> ApiResult<Json<Value>> {
    let (p, _) = project_for(&state, &user, id, Need::Read).await?;
    if p.kind != "latex" {
        return Err(ApiError::bad_request("invalid_value"));
    }
    let overrides = body.map(|b| b.0.overrides).unwrap_or_default();
    let main = p.settings["main"].as_str().unwrap_or("main.tex").to_string();
    let engine = match p.settings["engine"].as_str() {
        Some(e @ ("xelatex" | "lualatex")) => e,
        _ => "pdflatex",
    };
    let engine_bin = find(engine).ok_or_else(|| ApiError::bad_request("latex_unavailable"))?;
    let _slot = tokio::time::timeout(Duration::from_secs(120), SLOTS.acquire()).await.map_err(|_| ApiError::bad_request("latex_busy"))?.map_err(|_| ApiError::bad_request("latex_busy"))?;
    let started = Instant::now();

    // Dossier de travail : copie du projet.
    let dir = std::env::temp_dir().join(format!("looty-tex-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir).await.map_err(|e| ApiError::Internal(e.to_string()))?;
    let result = async {
        let files = all_files(&state, id).await?;
        if !files.iter().any(|f| f.path == main && !f.is_dir) {
            return Err(ApiError::bad_request("latex_no_main"));
        }
        for f in &files {
            let target = dir.join(&f.path);
            if f.is_dir {
                tokio::fs::create_dir_all(&target).await.map_err(|e| ApiError::Internal(e.to_string()))?;
                continue;
            }
            if let Some(parent) = target.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            let bytes = match overrides.get(&f.id) {
                Some(text) if f.blob_path.is_none() => text.clone().into_bytes(),
                _ => file_bytes(f).await.unwrap_or_default(),
            };
            tokio::fs::write(&target, bytes).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        }
        let stem = main.trim_end_matches(".tex").to_string();
        // Compilation depuis la racine du projet (chemins relatifs à la racine, comme Overleaf).
        let main_dir = dir.clone();
        let main_file = main.clone();
        let base = FsPath::new(&stem).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(stem.clone());
        let mut log = String::new();
        let flags = ["-interaction=nonstopmode", "-file-line-error", "-no-shell-escape", "-halt-on-error"];
        if let Some(latexmk) = find("latexmk") {
            let mode = match engine {
                "xelatex" => "-xelatex",
                "lualatex" => "-lualatex",
                _ => "-pdf",
            };
            let mut args = vec![mode];
            args.extend_from_slice(&flags);
            args.push(&main_file);
            let (_, out) = run(&main_dir, &latexmk, &args).await?;
            log.push_str(&out);
        } else {
            let mut args: Vec<&str> = flags.to_vec();
            args.push(&main_file);
            let (mut ok, out) = run(&main_dir, &engine_bin, &args).await?;
            log.push_str(&out);
            // Bibliographie : biber (biblatex) ou bibtex, puis deux passes.
            let aux = tokio::fs::read_to_string(main_dir.join(format!("{base}.aux"))).await.unwrap_or_default();
            let bcf = main_dir.join(format!("{base}.bcf"));
            let mut extra: u32 = 0;
            if ok && bcf.exists() {
                if let Some(biber) = find("biber") {
                    let (_, out) = run(&main_dir, &biber, &[&base]).await?;
                    log.push_str(&out);
                    extra = 2;
                }
            } else if ok && aux.contains("\\bibdata") {
                if let Some(bibtex) = find("bibtex") {
                    let (_, out) = run(&main_dir, &bibtex, &[&base]).await?;
                    log.push_str(&out);
                    extra = 2;
                }
            }
            let mut passes = 0;
            while ok && passes < 3 && (extra > 0 || log_needs_rerun(&main_dir, &base).await) {
                let (o, out) = run(&main_dir, &engine_bin, &args).await?;
                ok = o;
                log = out;
                extra = extra.saturating_sub(1);
                passes += 1;
            }
        }
        // Le journal complet du dernier passage fait foi.
        let full_log = tokio::fs::read_to_string(main_dir.join(format!("{base}.log"))).await.unwrap_or(log);
        let pdf = main_dir.join(format!("{base}.pdf"));
        let produced = tokio::fs::metadata(&pdf).await.is_ok_and(|m| m.len() > 0);
        let issues = parse_log(&full_log);
        let ok = produced && !issues.iter().any(|i| i.level == "error");
        if produced {
            let out = output_path(id);
            if let Some(parent) = out.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            tokio::fs::copy(&pdf, &out).await.map_err(|e| ApiError::Internal(e.to_string()))?;
        }
        Ok::<_, ApiError>((ok, produced, full_log, issues))
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    let (ok, produced, log, issues) = result?;
    sqlx::query("UPDATE projects SET last_build_at = NOW(), last_build_ok = $2 WHERE id = $1").bind(id).bind(ok).execute(&state.pool).await?;
    // Journal limité aux 200 derniers Ko.
    let log = if log.len() > 200_000 { log[log.len() - 200_000..].to_string() } else { log };
    Ok(Json(json!({
        "ok": ok,
        "pdf": produced,
        "engine": engine,
        "duration_ms": started.elapsed().as_millis() as u64,
        "issues": issues,
        "log": log,
    })))
}

async fn log_needs_rerun(dir: &FsPath, base: &str) -> bool {
    let log = tokio::fs::read_to_string(dir.join(format!("{base}.log"))).await.unwrap_or_default();
    log.contains("Rerun to get") || log.contains("Label(s) may have changed") || log.contains("Please rerun") || log.contains("Rerun LaTeX")
}

pub async fn output_pdf(State(state): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> ApiResult<Response> {
    let (p, _) = project_for(&state, &user, id, Need::Read).await?;
    let bytes = tokio::fs::read(output_path(id)).await.map_err(|_| ApiError::NotFound)?;
    let name: String = p.name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (header::CONTENT_DISPOSITION, format!("inline; filename=\"{name}.pdf\"")),
            (header::CACHE_CONTROL, "no-store".into()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
            (header::X_FRAME_OPTIONS, "SAMEORIGIN".into()),
        ],
        Body::from(bytes),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_issues() {
        let log = "(./main.tex\n./main.tex:7: Undefined control sequence.\nl.7 \\foo\n         bar\n\nLaTeX Warning: Citation `x' on page 1 undefined on input line 9.\n\n! Emergency stop.\n";
        let i = parse_log(log);
        assert_eq!(i[0].file.as_deref(), Some("main.tex"));
        assert_eq!(i[0].line, Some(7));
        assert!(i[0].message.contains("l.7"));
        assert!(i.iter().any(|x| x.level == "warning" && x.line == Some(9)));
        assert!(i.iter().any(|x| x.message == "Emergency stop."));
    }
}
