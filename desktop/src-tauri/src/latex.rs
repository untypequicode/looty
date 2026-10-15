//! Compilation LaTeX sur l'ordinateur (projets LaTeX de Looty).
//!
//! La page envoie les fichiers du projet ; ils sont écrits dans un dossier temporaire et
//! compilés avec la distribution TeX installée (latexmk, sinon pdflatex / xelatex /
//! lualatex avec bibtex ou biber). Mêmes garde-fous que sur le serveur : pas de
//! shell-escape, lecture et écriture limitées au dossier du projet, une minute par passe.

use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Deserialize)]
pub struct Bundle {
    main: String,
    engine: String,
    files: Vec<BundleFile>,
}

#[derive(Deserialize)]
struct BundleFile {
    path: String,
    #[serde(default)]
    is_dir: bool,
    #[serde(default)]
    data: String,
}

/// Exécutable cherché dans le PATH et les emplacements habituels de TeX Live.
fn find(bin: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.push("/usr/bin".into());
    dirs.push("/usr/local/bin".into());
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".nix-profile/bin"));
    }
    dirs.push("/run/current-system/sw/bin".into());
    if let Ok(entries) = std::fs::read_dir("/usr/local/texlive") {
        for e in entries.flatten() {
            dirs.push(e.path().join("bin/x86_64-linux"));
            dirs.push(e.path().join("bin/aarch64-linux"));
        }
    }
    // Windows : TeX Live (C:\texlive\<année>) et MiKTeX (pour l'utilisateur ou pour tous).
    #[cfg(windows)]
    {
        if let Ok(entries) = std::fs::read_dir("C:\\texlive") {
            for e in entries.flatten() {
                dirs.push(e.path().join("bin\\windows"));
                dirs.push(e.path().join("bin\\win64"));
            }
        }
        if let Some(local) = dirs::data_local_dir() {
            dirs.push(local.join("Programs\\MiKTeX\\miktex\\bin\\x64"));
        }
        dirs.push("C:\\Program Files\\MiKTeX\\miktex\\bin\\x64".into());
    }
    let name = if cfg!(windows) { format!("{bin}.exe") } else { bin.to_string() };
    dirs.into_iter().map(|d| d.join(&name)).find(|p| p.is_file())
}

/// Chemin relatif sans `..` ni chemin absolu.
fn safe_rel(p: &str) -> Option<PathBuf> {
    let path = Path::new(p);
    if p.is_empty() || path.is_absolute() || path.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return None;
    }
    Some(path.to_path_buf())
}

async fn run(dir: &Path, program: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .current_dir(dir)
        .env("openin_any", "p")
        .env("openout_any", "p")
        .env("shell_escape", "f")
        .env("max_print_line", "1000")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    match tokio::time::timeout(Duration::from_secs(60), child.wait_with_output()).await {
        Ok(Ok(o)) => Ok(format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("latex_timeout".into()),
    }
}

async fn needs_rerun(dir: &Path, base: &str) -> bool {
    let log = tokio::fs::read_to_string(dir.join(format!("{base}.log"))).await.unwrap_or_default();
    log.contains("Rerun to get") || log.contains("Label(s) may have changed") || log.contains("Please rerun") || log.contains("Rerun LaTeX")
}

pub async fn compile(bundle: Bundle) -> Value {
    let engine = match bundle.engine.as_str() {
        e @ ("xelatex" | "lualatex") => e,
        _ => "pdflatex",
    };
    let Some(engine_bin) = find(engine) else { return json!({ "error": "latex_local_missing" }) };
    let Some(main) = safe_rel(&bundle.main) else { return json!({ "error": "latex_no_main" }) };
    let dir = std::env::temp_dir().join(format!("looty-tex-{}", uuid::Uuid::new_v4()));
    let result = async {
        tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
        for f in &bundle.files {
            let Some(rel) = safe_rel(&f.path) else { continue };
            let target = dir.join(rel);
            if f.is_dir {
                tokio::fs::create_dir_all(&target).await.map_err(|e| e.to_string())?;
                continue;
            }
            if let Some(parent) = target.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
            }
            let bytes = base64::engine::general_purpose::STANDARD.decode(&f.data).unwrap_or_default();
            tokio::fs::write(&target, bytes).await.map_err(|e| e.to_string())?;
        }
        if !dir.join(&main).is_file() {
            return Err("latex_no_main".to_string());
        }
        let main_s = main.to_string_lossy().into_owned();
        let base = main.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "main".into());
        let flags = ["-interaction=nonstopmode", "-file-line-error", "-no-shell-escape", "-halt-on-error"];
        let mut log = String::new();
        if let Some(latexmk) = find("latexmk") {
            let mode = match engine {
                "xelatex" => "-xelatex",
                "lualatex" => "-lualatex",
                _ => "-pdf",
            };
            let mut args = vec![mode];
            args.extend_from_slice(&flags);
            args.push(&main_s);
            log = run(&dir, &latexmk, &args).await?;
        } else {
            let mut args: Vec<&str> = flags.to_vec();
            args.push(&main_s);
            log.push_str(&run(&dir, &engine_bin, &args).await?);
            let aux = tokio::fs::read_to_string(dir.join(format!("{base}.aux"))).await.unwrap_or_default();
            let mut extra = 0u32;
            if dir.join(format!("{base}.bcf")).exists() {
                if let Some(biber) = find("biber") {
                    log.push_str(&run(&dir, &biber, &[&base]).await?);
                    extra = 2;
                }
            } else if aux.contains("\\bibdata") {
                if let Some(bibtex) = find("bibtex") {
                    log.push_str(&run(&dir, &bibtex, &[&base]).await?);
                    extra = 2;
                }
            }
            let mut passes = 0;
            while passes < 3 && (extra > 0 || needs_rerun(&dir, &base).await) {
                log = run(&dir, &engine_bin, &args).await?;
                extra = extra.saturating_sub(1);
                passes += 1;
            }
        }
        let full = tokio::fs::read_to_string(dir.join(format!("{base}.log"))).await.unwrap_or(log);
        let pdf = tokio::fs::read(dir.join(format!("{base}.pdf"))).await.ok().filter(|b| !b.is_empty());
        Ok::<_, String>((pdf, full))
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    match result {
        Ok((pdf, log)) => {
            let log = if log.len() > 200_000 { log[log.len() - 200_000..].to_string() } else { log };
            json!({ "ok": pdf.is_some(), "pdf": pdf.map(|b| base64::engine::general_purpose::STANDARD.encode(b)), "log": log })
        }
        Err(e) if e.starts_with("latex_") => json!({ "error": e }),
        Err(e) => json!({ "ok": false, "pdf": null, "log": e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_paths() {
        assert!(safe_rel("../x.tex").is_none() && safe_rel("/etc/passwd").is_none() && safe_rel("").is_none());
        assert_eq!(safe_rel("chap/intro.tex"), Some(PathBuf::from("chap/intro.tex")));
    }

    /// Compilation réelle si un moteur TeX est disponible (sinon ignoré).
    #[tokio::test]
    async fn compiles_with_bibliography() {
        if find("pdflatex").is_none() {
            eprintln!("pdflatex absent : test ignoré");
            return;
        }
        let enc = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
        let bundle: Bundle = serde_json::from_value(json!({
            "main": "main.tex", "engine": "pdflatex",
            "files": [
                { "path": "main.tex", "data": enc("\\documentclass{article}\\begin{document}\\input{chap/a}\\cite{k}\\bibliographystyle{plain}\\bibliography{refs}\\end{document}") },
                { "path": "chap", "is_dir": true },
                { "path": "chap/a.tex", "data": enc("Bonjour.") },
                { "path": "refs.bib", "data": enc("@book{k, author={D. Knuth}, title={T}, publisher={P}, year={1984}}") },
                { "path": "../evil.tex", "data": enc("x") }
            ]
        }))
        .unwrap();
        let r = compile(bundle).await;
        assert_eq!(r["ok"], true, "{}", r["log"]);
        assert!(r["pdf"].as_str().unwrap().len() > 1000);
        assert!(!r["log"].as_str().unwrap().contains("Citation `k' on page 1 undefined"));
    }
}
