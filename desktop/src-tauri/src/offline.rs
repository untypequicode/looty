//! Mode hors ligne.
//!
//! - **Lecture** : chaque réponse GET réussie (interface, API, fichiers) est conservée
//!   dans `~/.cache/looty/<serveur>/` (dossier 0700) ; serveur injoignable → copie locale.
//! - **Écriture** : les modifications prises en charge (tâches, commentaires, pages,
//!   notifications lues) sont mises en file d'attente (`outbox.json`, 0600), appliquées
//!   à la copie locale (elles survivent à un redémarrage) et la réponse attendue par
//!   l'interface est reconstituée. Les créations portent un identifiant choisi par
//!   l'application : le serveur les rejoue sans doublon.
//! - **Synchronisation** : dès que le serveur répond, la file est rejouée dans l'ordre ;
//!   une modification refusée (4xx) est signalée puis abandonnée.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use url::Url;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Queued {
    pub method: String,
    pub path: String,
    pub body: Option<Value>,
    pub label: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Failed {
    pub label: String,
    pub status: u16,
}

pub struct Offline {
    dir: Mutex<Option<PathBuf>>,
    online: AtomicBool,
    syncing: AtomicBool,
    outbox: Mutex<Vec<Queued>>,
    failed: Mutex<Vec<Failed>>,
}

/// Réponse reconstituée pour une modification mise en file d'attente.
pub struct Synth {
    pub status: u16,
    pub body: Option<Value>,
}

fn hash(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

#[cfg(unix)]
fn private_dir(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::create_dir_all(p);
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
}
#[cfg(not(unix))]
fn private_dir(p: &Path) {
    let _ = std::fs::create_dir_all(p);
}

fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    std::io::Write::write_all(&mut opts.open(&tmp)?, data)?;
    std::fs::rename(tmp, path)
}

/// Requêtes GET conservées pour la lecture hors ligne.
pub fn cacheable(path: &str) -> bool {
    if path.starts_with("/__looty/") || path.starts_with("/collab") || path == "/api/ws" {
        return false;
    }
    if let Some(api) = path.strip_prefix("/api/") {
        return !api.starts_with("auth/login") && !api.starts_with("auth/desktop") && !api.starts_with("auth/logout") && !api.starts_with("auth/callback") && !api.starts_with("auth/dev-login");
    }
    true
}

/// Création dont l'identifiant peut être choisi par l'application.
pub fn takes_client_id(method: &str, path: &str) -> bool {
    let s: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    method == "POST"
        && matches!(
            s.as_slice(),
            ["api", "boards", "columns", _, "tasks"] | ["api", "boards", "tasks", _, "comments"] | ["api", "workspaces", _, "pages"]
        )
}

impl Offline {
    pub fn new() -> Offline {
        Offline { dir: Mutex::new(None), online: AtomicBool::new(true), syncing: AtomicBool::new(false), outbox: Mutex::new(Vec::new()), failed: Mutex::new(Vec::new()) }
    }

    /// Sélectionne le cache du serveur courant (et recharge sa file d'attente).
    pub fn use_server(&self, server: Option<&Url>) {
        let dir = server.map(|s| {
            let d = dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".")).join("looty").join(&hash(&s.origin().ascii_serialization())[..16]);
            private_dir(&d);
            private_dir(&d.join("http"));
            d
        });
        let outbox: Vec<Queued> = dir
            .as_ref()
            .and_then(|d| std::fs::read(d.join("outbox.json")).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        *self.outbox.lock().unwrap() = outbox;
        *self.dir.lock().unwrap() = dir;
    }

    /// Efface tout (déconnexion, changement de serveur).
    pub fn clear(&self) {
        if let Some(d) = self.dir.lock().unwrap().clone() {
            let _ = std::fs::remove_dir_all(&d);
            private_dir(&d);
            private_dir(&d.join("http"));
        }
        self.outbox.lock().unwrap().clear();
        self.failed.lock().unwrap().clear();
    }

    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::SeqCst)
    }
    pub fn set_online(&self, on: bool) {
        if self.online.swap(on, Ordering::SeqCst) != on {
            tracing::info!("server {}", if on { "reachable again" } else { "unreachable: offline mode" });
        }
    }
    pub fn pending(&self) -> usize {
        self.outbox.lock().unwrap().len()
    }
    pub fn status(&self) -> Value {
        json!({
            "online": self.is_online(),
            "syncing": self.syncing.load(Ordering::SeqCst),
            "pending": self.pending(),
            "failed": *self.failed.lock().unwrap(),
        })
    }
    pub fn dismiss_failed(&self) {
        self.failed.lock().unwrap().clear();
    }

    // -----------------------------------------------------------------------
    // Cache
    // -----------------------------------------------------------------------

    fn paths(&self, key: &str) -> Option<(PathBuf, PathBuf)> {
        let d = self.dir.lock().unwrap().clone()?.join("http");
        let h = hash(key);
        Some((d.join(format!("{h}.meta")), d.join(format!("{h}.body"))))
    }

    pub fn store(&self, key: &str, content_type: &str, body: &[u8]) {
        let Some((meta, data)) = self.paths(key) else { return };
        let m = json!({ "key": key, "content_type": content_type, "stored_at": now() });
        if write_private(&data, body).is_ok() {
            let _ = write_private(&meta, m.to_string().as_bytes());
        }
    }

    pub fn load(&self, key: &str) -> Option<(String, Vec<u8>)> {
        let (meta, data) = self.paths(key)?;
        let m: Value = serde_json::from_slice(&std::fs::read(meta).ok()?).ok()?;
        Some((m["content_type"].as_str().unwrap_or("application/octet-stream").to_string(), std::fs::read(data).ok()?))
    }

    fn load_json(&self, key: &str) -> Option<Value> {
        let (ct, body) = self.load(key)?;
        if !ct.contains("json") {
            return None;
        }
        serde_json::from_slice(&body).ok()
    }

    fn save_json(&self, key: &str, v: &Value) {
        self.store(key, "application/json", v.to_string().as_bytes());
    }

    /// Clés en cache commençant par `prefix`.
    fn keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let Some(d) = self.dir.lock().unwrap().clone() else { return vec![] };
        let Ok(rd) = std::fs::read_dir(d.join("http")) else { return vec![] };
        rd.filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "meta"))
            .filter_map(|e| serde_json::from_slice::<Value>(&std::fs::read(e.path()).ok()?).ok())
            .filter_map(|m| m["key"].as_str().map(str::to_string))
            .filter(|k| k.starts_with(prefix))
            .collect()
    }

    fn me(&self) -> Option<String> {
        self.load_json("/api/auth/me").and_then(|v| v["user"]["id"].as_str().map(str::to_string))
    }

    // -----------------------------------------------------------------------
    // File d'attente
    // -----------------------------------------------------------------------

    fn persist(&self) {
        if let Some(d) = self.dir.lock().unwrap().clone() {
            let data = serde_json::to_vec(&*self.outbox.lock().unwrap()).unwrap_or_default();
            let _ = write_private(&d.join("outbox.json"), &data);
        }
    }

    fn enqueue(&self, q: Queued) {
        {
            let mut ob = self.outbox.lock().unwrap();
            // Mises à jour successives d'une même ressource : fusionnées (le contenu d'une
            // page est enregistré toutes les quelques secondes pendant l'édition).
            if q.method == "PUT" && !q.path.ends_with("/toggle_validation") {
                if let Some(last) = ob.last_mut() {
                    if last.method == "PUT" && last.path == q.path {
                        if let (Some(Value::Object(a)), Some(Value::Object(b))) = (&mut last.body, &q.body) {
                            for (k, v) in b {
                                a.insert(k.clone(), v.clone());
                            }
                            drop(ob);
                            self.persist();
                            return;
                        }
                    }
                }
            }
            ob.push(q);
        }
        self.persist();
    }

    /// Met une modification en file d'attente et reconstitue la réponse attendue.
    /// `None` : action non disponible hors ligne.
    pub fn queue(&self, method: &str, path: &str, body: Option<Value>) -> Option<Synth> {
        let route = path.split('?').next().unwrap_or(path);
        let s: Vec<&str> = route.trim_start_matches('/').split('/').collect();
        let b = body.clone().unwrap_or(Value::Null);
        let (synth, label) = match (method, s.as_slice()) {
            ("POST", ["api", "boards", "columns", col, "tasks"]) => (self.create_task(col, &b)?, format!("+ {}", b["title"].as_str().unwrap_or(""))),
            ("PUT", ["api", "boards", "tasks", id]) => (self.update_task(id, &b)?, self.task_label(id)),
            ("DELETE", ["api", "boards", "tasks", id]) => {
                let label = self.task_label(id);
                self.delete_task(id);
                (Synth { status: 204, body: None }, format!("− {label}"))
            }
            ("PUT", ["api", "boards", "tasks", id, "move"]) => (self.move_task(id, &b)?, self.task_label(id)),
            ("PUT", ["api", "boards", "tasks", id, "toggle_validation"]) => (self.toggle_task(id)?, self.task_label(id)),
            (m @ ("POST" | "DELETE"), ["api", "boards", "tasks", id, kind @ ("labels" | "assignees"), other]) => {
                self.set_task_relation(id, kind, other, m == "POST");
                (Synth { status: 204, body: None }, self.task_label(id))
            }
            ("POST", ["api", "boards", "tasks", id, "comments"]) => (self.add_comment(id, &b)?, format!("💬 {}", self.task_label(id))),
            ("POST", ["api", "workspaces", ws, "pages"]) => (self.create_page(ws, &b)?, format!("+ {}", b["title"].as_str().unwrap_or(""))),
            ("PUT", ["api", "workspaces", ws, "pages", id]) => (self.update_page(ws, id, &b)?, self.page_label(ws, id)),
            ("PUT", ["api", "notifications", _, "read"]) | ("POST", ["api", "notifications", "read-all"]) => (Synth { status: 204, body: None }, String::new()),
            _ => return None,
        };
        self.enqueue(Queued { method: method.to_string(), path: path.to_string(), body, label });
        Some(synth)
    }

    // --- Tâches ----------------------------------------------------------------

    fn board_keys(&self) -> Vec<String> {
        self.keys_with_prefix("/api/boards/").into_iter().filter(|k| k.matches('/').count() == 3 && !k.contains('?') && !k.contains("/tasks") && !k.contains("/columns")).collect()
    }

    /// Applique `f` à la tâche dans tous les tableaux en cache ; renvoie la tâche modifiée.
    fn with_task(&self, id: &str, f: &mut dyn FnMut(&mut Value, &mut Value)) -> Option<Value> {
        let mut result = None;
        for key in self.board_keys() {
            let Some(mut board) = self.load_json(&key) else { continue };
            let snapshot = board.clone();
            let found = board["columns"].as_array().into_iter().flatten().enumerate().find_map(|(ci, c)| {
                c["tasks"].as_array().and_then(|ts| ts.iter().position(|t| t["id"] == id)).map(|ti| (ci, ti))
            });
            if let Some((ci, ti)) = found {
                let mut task = board["columns"][ci]["tasks"][ti].take();
                let mut b2 = snapshot.clone();
                f(&mut task, &mut b2);
                board["columns"][ci]["tasks"][ti] = task.clone();
                // `f` peut aussi réorganiser le tableau (déplacement) : il travaille sur b2.
                if b2 != snapshot {
                    board = b2;
                    if let Some(t) = board["columns"].as_array_mut().into_iter().flatten().flat_map(|c| c["tasks"].as_array_mut().into_iter().flatten()).find(|t| t["id"] == id) {
                        *t = task.clone();
                    }
                }
                self.save_json(&key, &board);
                result = Some(task);
            }
        }
        if let Some(task) = &result {
            let dk = format!("/api/boards/tasks/{id}");
            if let Some(mut detail) = self.load_json(&dk) {
                detail["task"] = task.clone();
                self.save_json(&dk, &detail);
            }
        }
        result
    }

    fn task_label(&self, id: &str) -> String {
        for key in self.board_keys() {
            if let Some(board) = self.load_json(&key) {
                if let Some(t) = board["columns"].as_array().into_iter().flatten().flat_map(|c| c["tasks"].as_array().into_iter().flatten()).find(|t| t["id"] == id) {
                    return t["title"].as_str().unwrap_or("").to_string();
                }
            }
        }
        String::new()
    }

    fn create_task(&self, column: &str, b: &Value) -> Option<Synth> {
        let id = b["id"].as_str()?.to_string();
        for key in self.board_keys() {
            let Some(mut board) = self.load_json(&key) else { continue };
            let labels = board["labels"].clone();
            let Some(col) = board["columns"].as_array_mut().and_then(|cs| cs.iter_mut().find(|c| c["id"] == column)) else { continue };
            let positions: Vec<f64> = col["tasks"].as_array().into_iter().flatten().filter_map(|t| t["position"].as_f64()).collect();
            let position = if b["at_top"] == true {
                positions.iter().cloned().fold(131072.0, f64::min) / 2.0
            } else {
                positions.iter().cloned().fold(0.0, f64::max) + 65536.0
            };
            let wanted: Vec<&str> = b["labels"].as_array().into_iter().flatten().filter_map(|l| l.as_str()).collect();
            let task_labels: Vec<Value> = labels.as_array().into_iter().flatten().filter(|l| l["id"].as_str().is_some_and(|i| wanted.contains(&i))).cloned().collect();
            let t = now();
            let task = json!({
                "id": id, "column_id": column, "parent_id": null, "title": b["title"], "description": b["description"],
                "position": position, "previous_column_id": null, "is_per_person": b["is_per_person"].as_bool().unwrap_or(false),
                "is_completed": false, "require_all_validations": b["require_all_validations"].as_bool().unwrap_or(true),
                "start_date": b["start_date"], "due_date": b["due_date"], "priority": b["priority"].as_i64().unwrap_or(0),
                "created_at": t, "updated_at": t, "labels": task_labels, "assignees": b["assignees"].as_array().cloned().unwrap_or_default(),
                "validations": [],
            });
            col["tasks"].as_array_mut()?.push(task.clone());
            self.save_json(&key, &board);
            return Some(Synth { status: 200, body: Some(task) });
        }
        None
    }

    fn update_task(&self, id: &str, b: &Value) -> Option<Synth> {
        let t = now();
        let task = self.with_task(id, &mut |task, _| {
            for k in ["title", "description", "priority", "due_date", "start_date", "is_per_person", "require_all_validations", "is_completed"] {
                if let Some(v) = b.get(k) {
                    task[k] = v.clone();
                }
            }
            task["updated_at"] = json!(t);
        })?;
        Some(Synth { status: 200, body: Some(task) })
    }

    fn move_task(&self, id: &str, b: &Value) -> Option<Synth> {
        let column = b["column_id"].as_str()?.to_string();
        let position = b["position"].clone();
        let task = self.with_task(id, &mut |task, board| {
            task["column_id"] = json!(column);
            task["position"] = position.clone();
            // Retire la tâche de sa colonne et l'ajoute à la destination.
            let mut moved = None;
            for c in board["columns"].as_array_mut().into_iter().flatten() {
                if let Some(ts) = c["tasks"].as_array_mut() {
                    if let Some(i) = ts.iter().position(|t| t["id"] == id) {
                        moved = Some(ts.remove(i));
                    }
                }
            }
            if moved.is_some() {
                if let Some(dest) = board["columns"].as_array_mut().and_then(|cs| cs.iter_mut().find(|c| c["id"] == column)) {
                    if let Some(ts) = dest["tasks"].as_array_mut() {
                        ts.push(task.clone());
                        ts.sort_by(|a, b| a["position"].as_f64().partial_cmp(&b["position"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
                    }
                }
            }
        })?;
        Some(Synth { status: 200, body: Some(task) })
    }

    fn toggle_task(&self, id: &str) -> Option<Synth> {
        let me = self.me();
        let task = self.with_task(id, &mut |task, _| {
            if task["is_per_person"] == true {
                let Some(me) = &me else { return };
                let mut v: Vec<Value> = task["validations"].as_array().cloned().unwrap_or_default();
                if let Some(i) = v.iter().position(|x| x == me.as_str()) {
                    v.remove(i);
                } else {
                    v.push(json!(me));
                }
                let assignees: Vec<Value> = task["assignees"].as_array().cloned().unwrap_or_default();
                task["is_completed"] = json!(if task["require_all_validations"] != false && !assignees.is_empty() {
                    assignees.iter().all(|a| v.contains(a))
                } else {
                    !v.is_empty()
                });
                task["validations"] = json!(v);
            } else {
                task["is_completed"] = json!(task["is_completed"] != true);
            }
        })?;
        Some(Synth { status: 200, body: Some(task) })
    }

    fn delete_task(&self, id: &str) {
        for key in self.board_keys() {
            let Some(mut board) = self.load_json(&key) else { continue };
            let mut changed = false;
            for c in board["columns"].as_array_mut().into_iter().flatten() {
                if let Some(ts) = c["tasks"].as_array_mut() {
                    let before = ts.len();
                    ts.retain(|t| t["id"] != id && t["parent_id"] != id);
                    changed |= ts.len() != before;
                }
            }
            if changed {
                self.save_json(&key, &board);
            }
        }
    }

    fn set_task_relation(&self, id: &str, kind: &str, other: &str, on: bool) {
        self.with_task(id, &mut |task, board| {
            if kind == "assignees" {
                let mut v: Vec<Value> = task["assignees"].as_array().cloned().unwrap_or_default();
                v.retain(|x| x != other);
                if on {
                    v.push(json!(other));
                }
                task["assignees"] = json!(v);
            } else {
                let mut v: Vec<Value> = task["labels"].as_array().cloned().unwrap_or_default();
                v.retain(|l| l["id"] != other);
                if on {
                    if let Some(l) = board["labels"].as_array().and_then(|ls| ls.iter().find(|l| l["id"] == other)) {
                        v.push(l.clone());
                    }
                }
                task["labels"] = json!(v);
            }
        });
    }

    fn add_comment(&self, task: &str, b: &Value) -> Option<Synth> {
        let t = now();
        let comment = json!({ "id": b["id"].as_str()?, "task_id": task, "user_id": self.me()?, "content": b["content"], "created_at": t, "updated_at": t });
        let key = format!("/api/boards/tasks/{task}/comments");
        if let Some(Value::Array(mut list)) = self.load_json(&key) {
            list.push(comment.clone());
            self.save_json(&key, &Value::Array(list));
        }
        Some(Synth { status: 200, body: Some(comment) })
    }

    // --- Pages -------------------------------------------------------------------

    fn page_label(&self, ws: &str, id: &str) -> String {
        self.load_json(&format!("/api/workspaces/{ws}/pages"))
            .and_then(|l| l.as_array().and_then(|a| a.iter().find(|p| p["id"] == id).and_then(|p| p["title"].as_str().map(str::to_string))))
            .unwrap_or_default()
    }

    fn create_page(&self, ws: &str, b: &Value) -> Option<Synth> {
        let id = b["id"].as_str()?.to_string();
        let me = self.me()?;
        let list_key = format!("/api/workspaces/{ws}/pages");
        let mut list = self.load_json(&list_key).unwrap_or(json!([]));
        let parent = b["parent_id"].clone();
        let position = list.as_array().into_iter().flatten().filter(|p| p["parent_id"] == parent).filter_map(|p| p["position"].as_f64()).fold(0.0, f64::max) + 65536.0;
        let t = now();
        let page = json!({
            "id": id, "workspace_id": ws, "parent_id": parent, "icon": b["icon"], "title": b["title"].as_str().unwrap_or(""),
            "content": b["content"], "created_by": me, "updated_by": me, "position": position, "created_at": t, "updated_at": t,
            "share_token": null, "is_public": false, "is_favorite": false, "total_view_count": 0, "can_edit": true,
        });
        let mut lite = page.clone();
        lite["content"] = Value::Null;
        if let Some(a) = list.as_array_mut() {
            a.push(lite);
        }
        self.save_json(&list_key, &list);
        self.save_json(&format!("{list_key}/{id}"), &page);
        Some(Synth { status: 201, body: Some(page) })
    }

    fn update_page(&self, ws: &str, id: &str, b: &Value) -> Option<Synth> {
        let t = now();
        let fields = ["title", "icon", "parent_id", "content", "position"];
        let list_key = format!("/api/workspaces/{ws}/pages");
        if let Some(mut list) = self.load_json(&list_key) {
            if let Some(p) = list.as_array_mut().and_then(|a| a.iter_mut().find(|p| p["id"] == id)) {
                for k in fields.iter().filter(|k| **k != "content") {
                    if let Some(v) = b.get(*k) {
                        p[*k] = v.clone();
                    }
                }
                p["updated_at"] = json!(t);
            }
            self.save_json(&list_key, &list);
        }
        let dk = format!("{list_key}/{id}");
        let mut page = self.load_json(&dk).or_else(|| self.load_json(&list_key).and_then(|l| l.as_array().and_then(|a| a.iter().find(|p| p["id"] == id).cloned())))?;
        for k in fields {
            if let Some(v) = b.get(k) {
                page[k] = v.clone();
            }
        }
        page["updated_at"] = json!(t);
        self.save_json(&dk, &page);
        Some(Synth { status: 200, body: Some(page) })
    }

    // -----------------------------------------------------------------------
    // Synchronisation
    // -----------------------------------------------------------------------

    /// Rejoue la file d'attente ; renvoie le nombre de modifications refusées.
    pub async fn replay(&self, client: &reqwest::Client, server: &Url) -> Result<usize, ()> {
        if self.syncing.swap(true, Ordering::SeqCst) {
            return Err(());
        }
        let origin = server.origin().ascii_serialization();
        let mut refused = 0;
        let result = loop {
            let Some(q) = self.outbox.lock().unwrap().first().cloned() else { break Ok(refused) };
            let method = reqwest::Method::from_bytes(q.method.as_bytes()).unwrap_or(reqwest::Method::POST);
            let mut req = client.request(method, format!("{origin}{}", q.path)).timeout(std::time::Duration::from_secs(30));
            if let Some(b) = &q.body {
                req = req.json(b);
            }
            match req.send().await {
                Ok(r) if r.status().is_success() => {}
                // Session expirée : on garde la file pour après la reconnexion.
                Ok(r) if r.status() == reqwest::StatusCode::UNAUTHORIZED => break Err(()),
                Ok(r) if r.status().is_client_error() && r.status() != reqwest::StatusCode::TOO_MANY_REQUESTS => {
                    tracing::warn!("offline change refused ({}): {} {}", r.status(), q.method, q.path);
                    if !q.label.is_empty() {
                        self.failed.lock().unwrap().push(Failed { label: q.label.clone(), status: r.status().as_u16() });
                    }
                    refused += 1;
                }
                // Erreur serveur ou réseau : nouvel essai plus tard.
                _ => break Err(()),
            }
            self.outbox.lock().unwrap().remove(0);
            self.persist();
        };
        self.syncing.store(false, Ordering::SeqCst);
        result
    }
}
