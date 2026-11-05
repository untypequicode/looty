//! Jeu de données « Looty Démo » (GLOSSARY : *Démo*) : l'espace modèle copié pour chaque visiteur de la démo
//! et utilisé pour les captures du site.
//!
//! `backend seed-demo [fr] [en]` (les deux langues par défaut) crée les cinq personnages, puis tout le
//! contenu **par l'API** : chaque personnage reçoit un jeton d'API temporaire et les requêtes passent par le
//! routeur de Looty dans le processus (droits, groupes reliés, récurrences, éléments personnels compris), sans
//! ouvrir de port. Relancer remplace l'espace précédent et vide les espaces perso des personnages ; les dates
//! sont recalculées à partir d'aujourd'hui (Europe/Paris). La commande affiche les identifiants en JSON.
//!
//! Les comptes ont des adresses en `.invalid` (RFC 2606) : aucun e-mail ne peut leur parvenir.

use axum::{
    body::Body,
    http::{header, Method, Request},
    Router,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::{json, Value};
use std::collections::HashMap;
use tower::ServiceExt;
use uuid::Uuid;

use crate::state::AppState;

const TZ: Tz = chrono_tz::Europe::Paris;
const TOKEN_NAME: &str = "seed-demo";

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Fr,
    En,
}

impl Lang {
    fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }
    /// Texte selon la langue.
    fn t(self, fr: &'static str, en: &'static str) -> &'static str {
        if self == Lang::Fr { fr } else { en }
    }
}

/// Rôle d'un personnage dans l'espace modèle (rôle prédéfini de l'espace : propriétaire, membre, lecteur).
#[derive(Clone, Copy, PartialEq)]
enum Cast {
    Admin,
    Member,
    Viewer,
}

impl Cast {
    fn as_str(self) -> &'static str {
        match self {
            Cast::Admin => "admin",
            Cast::Member => "member",
            Cast::Viewer => "viewer",
        }
    }
}

struct Character {
    key: &'static str,
    name: &'static str,
    role: Cast,
    color: (&'static str, &'static str),
    /// Tâches de son espace perso : (fr, en, échéance en jours).
    personal: [(&'static str, &'static str, i64); 3],
}

const MAYA: usize = 0;
const LEO: usize = 1;
const SARA: usize = 2;
const INES: usize = 3;
const NOAH: usize = 4;

const CHARACTERS: [Character; 5] = [
    Character {
        key: "maya",
        name: "Maya Chen",
        role: Cast::Admin,
        color: ("#7170ff", "#c084fc"),
        personal: [
            ("Préparer l'entretien annuel de Léo", "Prepare Leo's yearly review", 3),
            ("Relire le budget du trimestre", "Review the quarterly budget", 5),
            ("Réserver le train pour Lyon", "Book the train to Lyon", 1),
        ],
    },
    Character {
        key: "leo",
        name: "Léo Martin",
        role: Cast::Member,
        color: ("#4ea7fc", "#2dd4bf"),
        personal: [
            ("Mettre à jour mon portable", "Update my laptop", 0),
            ("Lire l'article sur les CRDT", "Read the CRDT article", 4),
            ("Préparer la présentation du mode hors ligne", "Prepare the offline mode talk", 2),
        ],
    },
    Character {
        key: "sara",
        name: "Sara Okafor",
        role: Cast::Member,
        color: ("#f2994a", "#f43f5e"),
        personal: [
            ("Répondre aux journalistes", "Answer the journalists", 1),
            ("Idées de posts pour la semaine", "Post ideas for the week", 2),
            ("Faire le point avec l'agence", "Catch up with the agency", 6),
        ],
    },
    Character {
        key: "ines",
        name: "Inès Moreau",
        role: Cast::Member,
        color: ("#b18cff", "#ec4899"),
        personal: [
            ("Ranger la bibliothèque d'icônes", "Tidy up the icon library", 3),
            ("Exporter les maquettes pour Léo", "Export the mockups for Leo", 1),
            ("Inscription à la conférence design", "Sign up for the design conference", 8),
        ],
    },
    Character {
        key: "noah",
        name: "Noah Fischer",
        role: Cast::Viewer,
        color: ("#4cb782", "#0ea5e9"),
        personal: [
            ("Envoyer mes retours sur la bêta", "Send my beta feedback", 2),
            ("Préparer les questions pour la démo", "Prepare questions for the demo", 3),
            ("Partager la démo avec mon équipe", "Share the demo with my team", 7),
        ],
    },
];

/// Domaine des comptes de la démo : jamais de courrier (`mail::send` l'ignore), et ils ne comptent pas
/// comme premier compte de l'instance (`auth`).
const EMAIL_DOMAIN: &str = "demo.looty.invalid";
/// Motif `LIKE` des adresses des personnages.
pub const EMAIL_PATTERN: &str = "%@demo.looty.invalid";

fn email(c: &Character, lang: Lang) -> String {
    format!("{}.{}@{EMAIL_DOMAIN}", c.key, lang.code())
}

/// Point d'entrée de la commande ; renvoie le code de sortie.
pub async fn run(state: AppState, args: &[String]) -> i32 {
    let mut langs = Vec::new();
    for a in args {
        match a.as_str() {
            "fr" => langs.push(Lang::Fr),
            "en" => langs.push(Lang::En),
            _ => {
                eprintln!("usage: backend seed-demo [fr] [en]");
                return 2;
            }
        }
    }
    if langs.is_empty() {
        langs = vec![Lang::Fr, Lang::En];
    }
    if let Err(e) = tokio::fs::create_dir_all(crate::upload::UPLOAD_DIR).await {
        eprintln!("seed-demo: {e}");
        return 1;
    }
    let mut out = serde_json::Map::new();
    for lang in langs {
        match seed(&state, lang).await {
            Ok(v) => {
                out.insert(lang.code().into(), v);
            }
            Err(e) => {
                eprintln!("seed-demo ({}): {e}", lang.code());
                return 1;
            }
        }
    }
    println!("{}", Value::Object(out));
    0
}

/// Crée (ou recrée) l'espace « Looty Démo » d'une langue.
async fn seed(state: &AppState, lang: Lang) -> Result<Value, String> {
    let pool = &state.pool;
    let mut ids = Vec::new();
    // Un personnage suspendu par un administrateur est réactivé : le modèle est recréé tel quel.
    for c in &CHARACTERS {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (email, name) VALUES ($1, $2) ON CONFLICT (email) DO UPDATE SET is_suspended = FALSE RETURNING id",
        )
        .bind(email(c, lang))
        .bind(c.name)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
        ids.push(id);
    }
    let api = Api::new(state, &ids).await?;
    let result = build(&api, lang, &ids).await;
    api.revoke().await;
    result
}

/// Accès à l'API de Looty dans le processus, avec un jeton par personnage.
struct Api {
    app: Router,
    pool: sqlx::PgPool,
    tokens: HashMap<Uuid, String>,
}

impl Api {
    /// Jetons créés ensemble ou pas du tout ; ceux d'une génération interrompue sont d'abord retirés.
    async fn new(state: &AppState, users: &[Uuid]) -> Result<Self, String> {
        let db = |e: sqlx::Error| e.to_string();
        let mut tx = state.pool.begin().await.map_err(db)?;
        sqlx::query("DELETE FROM api_tokens WHERE name = $1 AND user_id = ANY($2)").bind(TOKEN_NAME).bind(users).execute(&mut *tx).await.map_err(db)?;
        let mut tokens = HashMap::new();
        for &u in users {
            let raw = format!("looty_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
            sqlx::query("INSERT INTO api_tokens (user_id, name, token_hash, scope) VALUES ($1, $2, $3, 'api')")
                .bind(u)
                .bind(TOKEN_NAME)
                .bind(crate::access::hash_token(&raw))
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            tokens.insert(u, raw);
        }
        tx.commit().await.map_err(db)?;
        Ok(Api { app: crate::app(state.clone()), pool: state.pool.clone(), tokens })
    }

    async fn revoke(&self) {
        let users: Vec<Uuid> = self.tokens.keys().copied().collect();
        let _ = sqlx::query("DELETE FROM api_tokens WHERE name = $1 AND user_id = ANY($2)").bind(TOKEN_NAME).bind(&users).execute(&self.pool).await;
    }

    async fn send(&self, who: Uuid, method: Method, path: &str, content_type: &str, body: Body) -> Result<Value, String> {
        let req = Request::builder()
            .method(method.clone())
            .uri(path)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.tokens[&who]))
            .header(header::CONTENT_TYPE, content_type)
            .body(body)
            .map_err(|e| e.to_string())?;
        let res = self.app.clone().oneshot(req).await.map_err(|e| e.to_string())?;
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("{method} {path} → {status} {}", String::from_utf8_lossy(&bytes)));
        }
        Ok(serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn call(&self, who: Uuid, method: Method, path: &str, body: Option<Value>) -> Result<Value, String> {
        let body = body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty);
        self.send(who, method, path, "application/json", body).await
    }
    async fn get(&self, who: Uuid, path: &str) -> Result<Value, String> {
        self.call(who, Method::GET, path, None).await
    }
    async fn post(&self, who: Uuid, path: &str, body: Value) -> Result<Value, String> {
        self.call(who, Method::POST, path, Some(body)).await
    }
    async fn put(&self, who: Uuid, path: &str, body: Value) -> Result<Value, String> {
        self.call(who, Method::PUT, path, Some(body)).await
    }
    async fn delete(&self, who: Uuid, path: &str) -> Result<Value, String> {
        self.call(who, Method::DELETE, path, None).await
    }

    async fn upload(&self, who: Uuid, file_name: &str, mime: &str, data: &str) -> Result<Value, String> {
        let boundary = format!("looty-demo-{}", Uuid::new_v4().simple());
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: {mime}\r\n\r\n{data}\r\n--{boundary}--\r\n"
        );
        self.send(who, Method::POST, "/api/upload", &format!("multipart/form-data; boundary={boundary}"), Body::from(body)).await
    }
}

fn id(v: &Value) -> Result<Uuid, String> {
    v["id"].as_str().and_then(|s| Uuid::parse_str(s).ok()).ok_or_else(|| format!("missing id in {v}"))
}

/// Dates relatives au jour de la génération, à l'heure de Paris, comptées en **jours ouvrés** : une équipe
/// crédible ne se réunit pas le week-end, quel que soit le jour où l'on génère.
struct Clock {
    today: NaiveDate,
}

fn workday(d: NaiveDate) -> bool {
    d.weekday().num_days_from_monday() < 5
}

impl Clock {
    fn now() -> Self {
        Clock { today: Utc::now().with_timezone(&TZ).date_naive() }
    }
    /// `n`-ième jour ouvré à partir d'aujourd'hui (0 : aujourd'hui, ou lundi prochain le week-end).
    fn day(&self, n: i64) -> NaiveDate {
        let mut d = self.today;
        while !workday(d) {
            d += Duration::days(1);
        }
        for _ in 0..n.abs() {
            d += Duration::days(n.signum());
            while !workday(d) {
                d += Duration::days(n.signum());
            }
        }
        d
    }
    /// Lundi de la semaine de `day(0)`.
    fn monday(&self) -> NaiveDate {
        let d = self.day(0);
        d - Duration::days(d.weekday().num_days_from_monday() as i64)
    }
    fn local(date: NaiveDate, h: u32, m: u32) -> DateTime<Utc> {
        let local = date.and_hms_opt(h, m, 0).expect("valid time");
        TZ.from_local_datetime(&local).earliest().expect("valid local time").with_timezone(&Utc)
    }
    fn at(&self, n: i64, h: u32, m: u32) -> DateTime<Utc> {
        Self::local(self.day(n), h, m)
    }
    fn rfc(&self, n: i64, h: u32, m: u32) -> String {
        self.at(n, h, m).to_rfc3339()
    }
    /// « mardi 17 novembre » / « Tuesday, November 17 ».
    fn label(&self, n: i64, lang: Lang) -> String {
        let date = self.day(n);
        let wd = date.weekday().num_days_from_monday() as usize;
        let mo = date.month0() as usize;
        match lang {
            Lang::Fr => format!(
                "{} {} {}",
                ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"][wd],
                date.day(),
                ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"][mo]
            ),
            Lang::En => format!(
                "{}, {} {}",
                ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"][wd],
                ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"][mo],
                date.day()
            ),
        }
    }
}

/// Avatar SVG : initiales sur un dégradé.
fn avatar_svg(c: &Character) -> String {
    let initials: String = c.name.split_whitespace().filter_map(|w| w.chars().next()).collect();
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient></defs><rect width="128" height="128" rx="64" fill="url(#g)"/><text x="64" y="66" text-anchor="middle" dominant-baseline="middle" font-family="Inter, system-ui, sans-serif" font-size="50" font-weight="600" fill="#fff">{initials}</text></svg>"##,
        c.color.0, c.color.1
    )
}

async fn build(api: &Api, lang: Lang, ids: &[Uuid]) -> Result<Value, String> {
    let t = |fr, en| lang.t(fr, en);
    let clock = Clock::now();
    let admin = ids[MAYA];

    // --- Profils : nom, avatar (gardé s'il existe encore), langue, nouveautés déjà vues.
    for (c, &u) in CHARACTERS.iter().zip(ids) {
        let current: Option<String> = sqlx::query_scalar("SELECT avatar_url FROM users WHERE id = $1").bind(u).fetch_one(&api.pool).await.map_err(|e| e.to_string())?;
        let kept = current.filter(|url| {
            url.strip_prefix("/uploads/").is_some_and(|f| std::path::Path::new(crate::upload::UPLOAD_DIR).join(f).is_file())
        });
        let avatar = match kept {
            Some(url) => url,
            None => {
                let up = api.upload(u, &format!("{}.svg", c.key), "image/svg+xml", &avatar_svg(c)).await?;
                up["url"].as_str().ok_or_else(|| format!("avatar upload: {up}"))?.to_string()
            }
        };
        api.put(u, "/api/user/profile", json!({ "name": c.name, "avatar_url": avatar })).await?;
        api.put(u, "/api/user/preferences", json!({ "locale": lang.code(), "timezone": TZ.name() })).await?;
        api.put(u, "/api/user/releases/seen", json!({ "version": env!("CARGO_PKG_VERSION") })).await?;
    }

    // --- Idempotence : l'ancien espace modèle (tout espace partagé dont l'admin de la démo est propriétaire :
    // ces comptes ne servent qu'à lui), puis le contenu des espaces perso.
    let old: Vec<Uuid> = sqlx::query_scalar(
        "SELECT w.id FROM workspaces w JOIN workspace_members m ON m.workspace_id = w.id
          WHERE m.user_id = $1 AND m.role = 'owner' AND w.personal_of IS NULL",
    )
    .bind(admin.to_string())
    .fetch_all(&api.pool)
    .await
    .map_err(|e| e.to_string())?;
    for w in old {
        api.delete(admin, &format!("/api/workspaces/{w}")).await?;
    }
    let mut personal = Vec::new();
    for &u in ids {
        let p = id(&api.get(u, "/api/workspaces/personal").await?)?;
        for b in api.get(u, &format!("/api/workspaces/{p}/boards")).await?.as_array().cloned().unwrap_or_default() {
            api.delete(u, &format!("/api/boards/{}", id(&b)?)).await?;
        }
        for pg in api.get(u, &format!("/api/workspaces/{p}/pages")).await?.as_array().cloned().unwrap_or_default() {
            if pg["parent_id"].is_null() {
                api.delete(u, &format!("/api/workspaces/{p}/pages/{}", id(&pg)?)).await?;
            }
        }
        api.delete(u, &format!("/api/workspaces/{p}/trash")).await?;
        personal.push(p);
    }

    // --- Espace et équipe.
    let ws = api.post(admin, "/api/workspaces", json!({ "name": t("Looty Démo", "Looty Demo") })).await?;
    let w = id(&ws["workspace"])?;
    let roles = api.get(admin, &format!("/api/workspaces/{w}/access/roles")).await?;
    for (c, &u) in CHARACTERS.iter().zip(ids) {
        // L'admin a créé l'espace (propriétaire) ; les autres reçoivent le rôle prédéfini du même nom.
        if c.role == Cast::Admin {
            continue;
        }
        let role_id = roles.as_array().and_then(|r| r.iter().find(|x| x["preset"] == c.role.as_str())).map(id).transpose()?;
        api.post(admin, &format!("/api/workspaces/{w}/access/members/add"), json!({ "user_id": u, "role_id": role_id })).await?;
    }
    let modes = json!({ "modes": { "docs": "write", "boards": "write", "calendar": "write" }, "modesChosen": true });
    for (&u, &p) in ids.iter().zip(&personal) {
        api.call(u, Method::PATCH, &format!("/api/workspaces/{w}/prefs"), Some(modes.clone())).await?;
        api.call(u, Method::PATCH, &format!("/api/workspaces/{p}/prefs"), Some(modes.clone())).await?;
    }
    let groups_path = format!("/api/workspaces/{w}/groups");
    let group = |name: &'static str, color: &'static str, members: Vec<Uuid>| {
        api.post(admin, &groups_path, json!({ "name": name, "color": color, "members": members }))
    };
    let design = id(&group("Design", "#b18cff", vec![ids[INES], ids[MAYA]]).await?)?;
    let tech = id(&group(t("Technique", "Engineering"), "#4ea7fc", vec![ids[LEO], ids[MAYA]]).await?)?;
    let team = id(&group(t("Équipe lancement", "Launch team"), "#f2994a", vec![ids[MAYA], ids[LEO], ids[SARA], ids[INES]]).await?)?;

    // --- Tableau de lancement.
    let board_name = t("Lancement de Looty 2.0", "Looty 2.0 launch");
    let board = id(&api.post(admin, &format!("/api/workspaces/{w}/boards"), json!({ "name": board_name, "with_default_columns": false })).await?)?;
    let mut cols = Vec::new();
    for name in [t("À faire", "Backlog"), t("En cours", "In progress"), t("En revue", "Review"), t("Terminé", "Done")] {
        cols.push(id(&api.post(admin, &format!("/api/boards/{board}/columns"), json!({ "name": name })).await?)?);
    }
    let mut labels = Vec::new();
    for (name, color) in [("Design", "#b18cff"), (t("Technique", "Engineering"), "#4ea7fc"), ("Marketing", "#f2994a"), ("Bug", "#eb5757")] {
        labels.push(id(&api.post(admin, &format!("/api/boards/{board}/labels"), json!({ "name": name, "color": color })).await?)?);
    }
    let (l_design, l_tech, l_marketing, l_bug) = (labels[0], labels[1], labels[2], labels[3]);
    /// Tâche du tableau : colonne, titre, étiquette, personne assignée, priorité, échéance (jours ouvrés).
    struct BoardTask {
        col: usize,
        title: (&'static str, &'static str),
        label: Uuid,
        who: usize,
        priority: i32,
        due: i64,
        keywords: &'static [&'static str],
    }
    const TODO: usize = 0;
    const DOING: usize = 1;
    const REVIEW: usize = 2;
    const DONE: usize = 3;
    let task = |col, title, label, who, priority, due, keywords| BoardTask { col, title, label, who, priority, due, keywords };
    let tasks = [
        task(TODO, ("Préparer la semaine de lancement", "Plan the launch week"), l_marketing, SARA, 2, 9, &["v2"]),
        task(TODO, ("Écrire les notes de version", "Write the release notes"), l_marketing, MAYA, 1, 7, &[]),
        task(TODO, ("Tourner la vidéo de démo", "Record the demo video"), l_design, INES, 2, 10, &[]),
        task(DOING, ("Mode hors ligne de l'app mobile", "Offline mode for the mobile app"), l_tech, LEO, 4, 5, &["mobile", "v2"]),
        task(DOING, ("Textes de la page tarifs", "Pricing page copy"), l_marketing, SARA, 2, 4, &[]),
        task(DOING, ("Plan de la conférence de presse", "Press conference outline"), l_marketing, MAYA, 0, 8, &[]),
        task(REVIEW, ("Corriger la synchro de l'agenda sur Android", "Fix calendar sync on Android"), l_bug, LEO, 4, 1, &["mobile"]),
        task(REVIEW, ("Audit d'accessibilité", "Accessibility audit"), l_design, INES, 3, 2, &[]),
        task(DONE, ("Mettre en place la CI", "Set up the CI pipeline"), l_tech, LEO, 2, -2, &[]),
        task(DONE, ("Choisir la nouvelle icône", "Choose the new icon"), l_design, INES, 1, -4, &[]),
        task(DONE, ("Synthèse des retours de la bêta", "Summarize the beta feedback"), l_marketing, SARA, 1, -3, &[]),
    ];
    let col_path = |c: usize| format!("/api/boards/columns/{}/tasks", cols[c]);
    for bt in tasks {
        let body = json!({ "title": t(bt.title.0, bt.title.1), "priority": bt.priority, "due_date": clock.at(bt.due, 18, 0), "labels": [bt.label], "assignees": [ids[bt.who].to_string()] });
        let task_id = id(&api.post(admin, &col_path(bt.col), body).await?)?;
        if bt.col == DONE {
            api.put(ids[bt.who], &format!("/api/boards/tasks/{task_id}/toggle_validation"), json!({})).await?;
        }
        if !bt.keywords.is_empty() {
            api.put(admin, &format!("/api/keywords/task/{task_id}"), json!({ "keywords": bt.keywords })).await?;
        }
    }
    // Groupe relié et sous-tâches.
    let onboarding = id(&api
        .post(admin, &col_path(DOING), json!({
            "title": t("Refondre l'accueil des nouveaux comptes", "Redesign the onboarding flow"),
            "description": t("<p>Moins d'écrans, un premier tableau prêt à l'emploi.</p>", "<p>Fewer screens and a ready-to-use first board.</p>"),
            "priority": 3, "start_date": clock.at(-3, 9, 0), "due_date": clock.at(3, 18, 0), "labels": [l_design], "groups": [design],
        }))
        .await?)?;
    for (fr, en) in [("Maquettes", "Wireframes"), ("Tests utilisateurs", "User testing"), ("Écrans finaux", "Final screens")] {
        api.post(ids[INES], &format!("/api/boards/tasks/{onboarding}/subtasks"), json!({ "title": t(fr, en) })).await?;
    }
    api.post(admin, &col_path(DOING), json!({
        "title": t("Revue de sécurité avant la sortie", "Security review before release"), "priority": 3,
        "due_date": clock.at(6, 18, 0), "labels": [l_tech], "groups": [tech],
    })).await?;
    // Tâche récurrente.
    api.post(admin, &col_path(TODO), json!({
        "title": t("Point hebdo avec les clients de la bêta", "Weekly check-in with beta customers"), "priority": 1,
        "due_date": clock.at(2, 17, 0), "labels": [l_marketing], "assignees": [ids[SARA].to_string()], "repeat": "weekly",
    })).await?;
    // Tâche personnelle sur le tableau partagé.
    api.post(ids[INES], &col_path(TODO), json!({ "title": t("Mes pistes pour le logo animé", "My ideas for the animated logo"), "personal": true, "due_date": clock.at(4, 18, 0) })).await?;

    // --- Documents.
    let pages_path = format!("/api/workspaces/{w}/pages");
    let page = |who: Uuid, title: &str, icon: &str, content: String, parent: Option<Uuid>, personal: bool| {
        api.post(who, &pages_path, json!({ "title": title, "icon": icon, "content": content, "parent_id": parent, "personal": personal }))
    };
    let launch = id(&page(admin, t("Plan de lancement", "Launch plan"), "🚀", launch_plan(lang, &clock), None, false).await?)?;
    api.put(admin, &format!("/api/keywords/page/{launch}"), json!({ "keywords": [t("lancement", "launch"), "v2"] })).await?;
    page(admin, t("Compte rendu · réunion de lundi", "Meeting notes · Monday"), "📝", meeting_notes(lang), Some(launch), false).await?;
    page(admin, t("Mes notes (personnel)", "My private notes"), "🔒", t(
        "<p>Visible de moi seule. À aborder avec Léo : la charge de travail avant le gel des fonctions.</p>",
        "<p>Only I can see this. To discuss with Leo: the workload before the feature freeze.</p>",
    ).into(), Some(launch), true).await?;
    page(ids[LEO], t("Guide technique", "Engineering handbook"), "📚", handbook(lang), None, false).await?;
    page(ids[INES], t("Charte graphique", "Brand guidelines"), "🎨", brand(lang), None, false).await?;

    // --- Projets : notebook (Pyodide) et petit site, chacun porté par une page.
    let project_page = |who: Uuid, title: &'static str, icon: &'static str, kind: &'static str, template: &'static str| {
        let path = &pages_path;
        async move {
            let p = api.post(who, path, json!({ "title": title, "icon": icon, "project": { "kind": kind, "template": template } })).await?;
            p["project_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()).ok_or_else(|| format!("missing project_id in {p}"))
        }
    };
    let notebook = project_page(ids[SARA], t("Analyse de la bêta", "Beta analysis"), "📊", "notebook", "blank").await?;
    set_file(api, ids[SARA], notebook, "notebook.ipynb", &notebook_content(lang)).await?;
    api.put(ids[SARA], &format!("/api/keywords/project/{notebook}"), json!({ "keywords": [t("bêta", "beta"), t("analyse", "analysis")] })).await?;
    let site = project_page(ids[INES], t("Site de lancement", "Launch site"), "🌐", "site", "starter").await?;
    for (path, content) in site_files(lang, &clock) {
        set_file(api, ids[INES], site, path, &content).await?;
    }

    // --- Agenda : une semaine glissante à partir d'aujourd'hui.
    let events_path = format!("/api/workspaces/{w}/events");
    let event = |body: Value| api.post(admin, &events_path, body);
    // Point quotidien du lundi au vendredi : une série hebdomadaire par jour, commencée la semaine dernière.
    for k in 0..5 {
        let date = clock.monday() - Duration::days(7 - k);
        let start = Clock::local(date, 9, 30);
        event(json!({ "title": t("Point quotidien", "Daily standup"), "all_day": false, "start": start.to_rfc3339(), "end": (start + Duration::minutes(15)).to_rfc3339(), "tz": TZ.name(), "repeat": { "freq": "weekly", "interval": 1 }, "groups": [team] })).await?;
    }
    for (d, h, m, minutes, fr, en, place, groups, members) in [
        (0, 10, 30, 60, "Planification du sprint", "Sprint planning", "", vec![team], vec![]),
        (1, 14, 0, 60, "Revue design", "Design review", "", vec![design], vec![]),
        (2, 11, 0, 45, "Point mode hors ligne", "Offline mode sync", "", vec![tech], vec![]),
        (3, 16, 0, 60, "Démo client", "Customer demo", "Visio", vec![], vec![ids[MAYA], ids[SARA], ids[NOAH]]),
        (4, 12, 30, 90, "Déjeuner d'équipe", "Team lunch", "Café Lumière", vec![team], vec![]),
        (6, 15, 0, 45, "Rétrospective", "Retrospective", "", vec![team], vec![]),
        (-1, 15, 0, 60, "Atelier tarifs", "Pricing workshop", "", vec![], vec![ids[MAYA], ids[SARA]]),
    ] {
        let end = clock.at(d, h, m) + Duration::minutes(minutes);
        event(json!({ "title": t(fr, en), "all_day": false, "start": clock.rfc(d, h, m), "end": end.to_rfc3339(), "tz": TZ.name(), "location": place, "groups": groups, "members": members })).await?;
    }
    let freeze = clock.day(5).format("%Y-%m-%d").to_string();
    event(json!({ "title": t("Gel des fonctions", "Feature freeze"), "all_day": true, "start": freeze, "end": freeze })).await?;

    // --- Annonces : une épinglée avec sondage, une pour un groupe.
    let announcement = api
        .post(admin, &format!("/api/workspaces/{w}/announcements"), json!({
            "title": format!("{} {}", t("Gel des fonctions le", "Feature freeze on"), clock.label(5, lang)),
            "body": t("Après le gel, seulement des corrections relues par deux personnes. Pensez à finir vos revues d'ici là.", "After the freeze, only fixes reviewed by two people. Please finish your reviews before then."),
            "pinned": true,
            "poll": {
                "question": t("Où fête-t-on le lancement ?", "Where should we celebrate the launch?"),
                "options": [t("Pique-nique au parc", "Picnic in the park"), t("Restaurant italien", "Italian restaurant"), t("Soirée jeux au bureau", "Board games at the office")],
            },
        }))
        .await?;
    let a = id(&announcement)?;
    let options: Vec<Uuid> = announcement["poll"]["options"].as_array().map(|o| o.iter().filter_map(|x| id(x).ok()).collect()).unwrap_or_default();
    for (who, choice) in [(LEO, 0), (SARA, 1), (INES, 0)] {
        if let Some(o) = options.get(choice) {
            api.post(ids[who], &format!("/api/workspaces/{w}/announcements/{a}/vote"), json!({ "options": [o] })).await?;
        }
    }
    api.post(admin, &format!("/api/workspaces/{w}/announcements"), json!({
        "title": t("Nouvelle CI : les tests tournent en 4 minutes", "New CI: tests now run in 4 minutes"),
        "body": t("Merci Léo ! Pensez à relancer vos branches.", "Thanks Leo! Remember to rebase your branches."),
        "audience": { "groups": [tech] },
    })).await?;

    // --- Espaces perso : quelques tâches par personnage.
    let mut characters = Vec::new();
    for ((c, &u), &p) in CHARACTERS.iter().zip(ids).zip(&personal) {
        let b = api.post(u, &format!("/api/workspaces/{p}/boards"), json!({ "name": t("Mes tâches", "My tasks"), "with_default_columns": true })).await?;
        let full = api.get(u, &format!("/api/boards/{}", id(&b)?)).await?;
        let first = id(&full["columns"][0])?;
        for (fr, en, due) in c.personal {
            api.post(u, &format!("/api/boards/columns/{first}/tasks"), json!({ "title": t(fr, en), "due_date": clock.at(due, 18, 0) })).await?;
        }
        characters.push(json!({
            "id": u, "email": email(c, lang), "name": c.name,
            "role": c.role.as_str(),
            "personal_tasks": c.personal.len(),
        }));
    }

    Ok(json!({ "workspace": w, "board": board, "board_name": board_name, "launch": launch, "characters": characters }))
}

async fn set_file(api: &Api, who: Uuid, project: Uuid, path: &str, content: &str) -> Result<(), String> {
    let p = api.get(who, &format!("/api/projects/{project}")).await?;
    let existing = p["files"].as_array().and_then(|f| f.iter().find(|x| x["path"] == path)).map(id).transpose()?;
    match existing {
        Some(f) => api.put(who, &format!("/api/projects/{project}/files/{f}"), json!({ "content": content })).await?,
        None => api.post(who, &format!("/api/projects/{project}/files"), json!({ "path": path, "content": content })).await?,
    };
    Ok(())
}

fn checklist(items: &[(bool, &str)]) -> String {
    let li: String = items
        .iter()
        .map(|(done, text)| {
            format!(
                r#"<li data-checked="{done}" data-type="taskItem"><label><input type="checkbox"{}><span></span></label><div><p>{text}</p></div></li>"#,
                if *done { r#" checked="checked""# } else { "" }
            )
        })
        .collect();
    format!(r#"<ul data-type="taskList">{li}</ul>"#)
}

fn launch_plan(lang: Lang, c: &Clock) -> String {
    let t = |fr, en| lang.t(fr, en);
    let diagram = t(
        "graph LR; A[Maquettes] --> B[Développement]; B --> C[Revue]; C --> D[Sortie]; C --> B",
        "graph LR; A[Design] --> B[Build]; B --> C[Review]; C --> D[Ship]; C --> B",
    );
    let row = |when: String, what: &str, who: &str| format!("<tr><td><p>{when}</p></td><td><p>{what}</p></td><td><p>{who}</p></td></tr>");
    format!(
        r#"<p>{intro} <strong>{launch}</strong>.</p>
<h2>{goals}</h2>
<ul><li><p>{g1}</p></li><li><p>{g2}</p></li><li><p>{g3}</p></li></ul>
<blockquote data-notice-type="info" data-type="notice" class="callout callout-info"><p><strong>{freeze} {freeze_date}.</strong> {freeze_more}</p></blockquote>
<h2>{flow}</h2>
<div data-type="mermaid"><pre>{diagram}</pre></div>
<h2>{timeline}</h2>
<table><tbody><tr><th><p>{date}</p></th><th><p>{focus}</p></th><th><p>{owner}</p></th></tr>{r1}{r2}{r3}{r4}</tbody></table>
<h2>{goal_h}</h2>
<p>{goal_p} <span data-type="math-inline" latex="r = \frac{{\text{{actifs}}_{{30}}}}{{\text{{inscrits}}}} \geq 40\,\%"></span></p>
<h2>Checklist</h2>
{list}"#,
        intro = t("Tout ce qu'il faut pour sortir Looty 2.0 le", "Everything we need to ship Looty 2.0 on"),
        launch = c.label(10, lang),
        goals = t("Objectifs", "Goals"),
        g1 = t("Sortir le nouvel accueil et le mode hors ligne sur toutes les plateformes", "Ship the new onboarding and offline mode on every platform"),
        g2 = t("Une semaine de lancement calme : aucun gros changement après le gel", "Keep the launch week calm: no big changes after the freeze"),
        g3 = t("Atteindre <strong>500 équipes</strong> le premier mois", "Reach <strong>500 teams</strong> in the first month"),
        freeze = t("Gel des fonctions le", "Feature freeze on"),
        freeze_date = c.label(5, lang),
        freeze_more = t("Ensuite, seulement des corrections relues par deux personnes.", "After that, only fixes reviewed by two people."),
        flow = t("Circuit de sortie", "Release flow"),
        timeline = t("Calendrier", "Timeline"),
        date = t("Date", "Date"),
        focus = t("Sujet", "Focus"),
        owner = t("Responsable", "Owner"),
        r1 = row(c.label(-5, lang), t("Accueil et mode hors ligne", "Onboarding and offline mode"), "Maya, Léo"),
        r2 = row(c.label(0, lang), t("Finitions mobiles et accessibilité", "Mobile polish and accessibility"), "Inès, Léo"),
        r3 = row(c.label(5, lang), t("Version candidate, gel des fonctions", "Release candidate, feature freeze"), "Léo"),
        r4 = row(c.label(10, lang), t("Semaine de lancement", "Launch week"), "Sara"),
        goal_h = t("Indicateur de réussite", "Success metric"),
        goal_p = t("Taux de rétention à 30 jours :", "30-day retention rate:"),
        list = checklist(&[
            (true, t("Fixer la date de lancement", "Pick the launch date")),
            (true, t("Choisir la nouvelle icône", "Choose the new icon")),
            (false, t("Écrire les notes de version", "Write the release notes")),
            (false, t("Caler la démo avec les premiers clients", "Book the demo with the first customers")),
        ]),
    )
}

fn meeting_notes(lang: Lang) -> String {
    let t = |fr, en| lang.t(fr, en);
    format!(
        "<p>{}</p><h3>{}</h3><ul><li><p>{}</p></li><li><p>{}</p></li></ul><h3>{}</h3>{}",
        t("Présents : Maya, Léo, Sara, Inès.", "Present: Maya, Leo, Sara, Ines."),
        t("Décisions", "Decisions"),
        t("Le mode hors ligne passe en priorité haute.", "Offline mode becomes high priority."),
        t("La vidéo de démo sera tournée après la revue design.", "The demo video will be recorded after the design review."),
        t("Actions", "Action items"),
        checklist(&[
            (true, t("Léo : corriger la synchro Android", "Leo: fix the Android sync")),
            (false, t("Sara : envoyer le brief à l'agence", "Sara: send the brief to the agency")),
        ]),
    )
}

fn handbook(lang: Lang) -> String {
    let t = |fr, en| lang.t(fr, en);
    let code = "async function sync(queue) {\n  for (const change of queue.pending()) {\n    await api.push(change)\n    queue.ack(change.id)\n  }\n}";
    format!(
        r#"<p>{intro}</p>
<h2>{branches}</h2>
<table><tbody><tr><th><p>{env}</p></th><th><p>{branch}</p></th><th><p>{deploy}</p></th></tr><tr><td><p>Production</p></td><td><p><code>main</code></p></td><td><p>{tag}</p></td></tr><tr><td><p>Staging</p></td><td><p><code>develop</code></p></td><td><p>{push}</p></td></tr></tbody></table>
<h2>{offline}</h2>
<p>{offline_p}</p>
<pre><code class="language-javascript">{code}</code></pre>
<h2>{conflicts}</h2>
<p>{conflicts_p}</p>
<div data-type="math-block" latex="t_{{\text{{final}}}} = \max_{{i \in \{{1,\dots,n\}}}} t_i"></div>"#,
        intro = t("Comment nous construisons, relisons et livrons.", "How we build, review and ship."),
        branches = t("Branches et environnements", "Branches and environments"),
        env = t("Environnement", "Environment"),
        branch = t("Branche", "Branch"),
        deploy = t("Déploiement", "Deploy"),
        tag = t("À chaque étiquette de version", "On every release tag"),
        push = t("À chaque push", "On every push"),
        offline = t("Mode hors ligne", "Offline mode"),
        offline_p = t("Les changements faits hors ligne sont mis en file puis envoyés dans l'ordre :", "Changes made offline are queued, then sent in order:"),
        code = code.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"),
        conflicts = t("Conflits", "Conflicts"),
        conflicts_p = t("Quand deux appareils modifient le même champ, la modification la plus récente gagne :", "When two devices change the same field, the latest change wins:"),
    )
}

fn brand(lang: Lang) -> String {
    let t = |fr, en| lang.t(fr, en);
    format!(
        "<p>{}</p><h2>{}</h2><table><tbody><tr><th><p>{}</p></th><th><p>{}</p></th></tr><tr><td><p>Indigo</p></td><td><p><code>#7170ff</code></p></td></tr><tr><td><p>{}</p></td><td><p><code>#ffb27a</code></p></td></tr><tr><td><p>{}</p></td><td><p><code>#0b0b0d</code></p></td></tr></tbody></table><h2>{}</h2><p>{}</p>",
        t("Couleurs, typographie et ton.", "Colors, typography and tone of voice."),
        t("Couleurs", "Colors"),
        t("Nom", "Name"),
        t("Valeur", "Value"),
        t("Pêche", "Peach"),
        t("Nuit", "Night"),
        t("Ton", "Tone"),
        t("Calme, précis, chaleureux. On tutoie les équipes, jamais de jargon.", "Calm, precise, warm. Plain words, never jargon."),
    )
}

fn notebook_content(lang: Lang) -> String {
    let t = |fr, en| lang.t(fr, en);
    let data = "jours = list(range(1, 15))\nactifs = [120, 132, 140, 151, 149, 160, 171, 180, 178, 190, 204, 211, 220, 236]";
    let data_en = "days = list(range(1, 15))\nactive = [120, 132, 140, 151, 149, 160, 171, 180, 178, 190, 204, 211, 220, 236]";
    let plot = t(
        "import matplotlib.pyplot as plt\n\nplt.plot(jours, actifs, marker='o')\nplt.title('Équipes actives pendant la bêta')\nplt.xlabel('Jour')\nplt.ylabel('Équipes')\nplt.show()",
        "import matplotlib.pyplot as plt\n\nplt.plot(days, active, marker='o')\nplt.title('Active teams during the beta')\nplt.xlabel('Day')\nplt.ylabel('Teams')\nplt.show()",
    );
    let growth = t(
        "croissance = (actifs[-1] - actifs[0]) / actifs[0]\nprint(f'Croissance sur deux semaines : {croissance:.0%}')",
        "growth = (active[-1] - active[0]) / active[0]\nprint(f'Growth over two weeks: {growth:.0%}')",
    );
    crate::projects::notebook_json(&[
        ("markdown", t("# Analyse de la bêta\n\nLe code s'exécute dans votre navigateur (Pyodide) : **Maj+Entrée** sur une cellule.", "# Beta analysis\n\nThe code runs in your browser (Pyodide): **Shift+Enter** on a cell.")),
        ("code", t(data, data_en)),
        ("code", growth),
        ("code", plot),
        ("markdown", t("Objectif : une croissance $g = \\frac{a_{14} - a_1}{a_1}$ d'au moins 50 %.", "Goal: a growth $g = \\frac{a_{14} - a_1}{a_1}$ of at least 50%.")),
    ])
}

fn site_files(lang: Lang, c: &Clock) -> Vec<(&'static str, String)> {
    let t = |fr, en| lang.t(fr, en);
    vec![
        (
            "index.html",
            format!(
                "<!doctype html>\n<html lang=\"{code}\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>Looty 2.0</title>\n  <link rel=\"stylesheet\" href=\"css/style.css\">\n</head>\n<body>\n  <main>\n    <p class=\"badge\">{date}</p>\n    <h1>Looty 2.0</h1>\n    <p>{tagline}</p>\n    <button id=\"notify\">{button}</button>\n  </main>\n  <script src=\"js/script.js\"></script>\n</body>\n</html>\n",
                code = lang.code(),
                date = c.label(10, lang),
                tagline = t("Le nouvel accueil, le mode hors ligne et une app mobile plus rapide.", "The new onboarding, offline mode and a faster mobile app."),
                button = t("Me prévenir", "Notify me"),
            ),
        ),
        ("css/style.css", "body {\n  margin: 0;\n  font-family: system-ui, sans-serif;\n  color: #ededef;\n  background: radial-gradient(60% 50% at 30% 20%, #2b2a6b, #0b0b0d);\n  min-height: 100vh;\n}\n\nmain {\n  max-width: 640px;\n  margin: 0 auto;\n  padding: 96px 24px;\n}\n\nh1 {\n  font-size: 56px;\n  margin: 8px 0;\n}\n\n.badge {\n  display: inline-block;\n  padding: 4px 12px;\n  border-radius: 999px;\n  background: #7170ff33;\n  color: #a5a4ff;\n}\n\nbutton {\n  padding: 10px 18px;\n  border: 0;\n  border-radius: 8px;\n  background: #7170ff;\n  color: white;\n  cursor: pointer;\n}\n".into()),
        (
            "js/script.js",
            format!(
                "document.getElementById('notify').addEventListener('click', (e) => {{\n  e.target.textContent = '{}'\n}})\n",
                t("C'est noté !", "You're on the list!")
            ),
        ),
    ]
}
