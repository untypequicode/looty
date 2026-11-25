//! Relais du tunnel PC : multiplexage de flux sur la connexion WebSocket d'un agent `looty-bridge`.
//!
//! Le protocole (version 1) est décrit dans `docs/tunnel-protocol.md` ; ce module en est
//! l'implémentation côté instance, sans accès à la base ni à axum (voir `tunnel.rs` pour le point
//! d'entrée, les jetons et le routage). Il se teste donc avec un faux agent : un canal qui rejoue
//! ce que le relais écrit vers `Session::on_text` et `Session::on_binary`.
//!
//! - Seule l'instance ouvre des flux (`open`), vers un service que l'agent a déclaré et dont le
//!   type correspond (liste blanche) ; jamais vers une adresse choisie par l'appelant.
//! - Chaque sens d'un flux a une fenêtre de crédit en octets (contre-pression) : on n'envoie pas
//!   plus que le crédit reçu, on rend le crédit quand l'application a consommé les données.
//! - Tout est borné : flux simultanés, taille des trames, des en-têtes, des messages et des
//!   corps, délais d'attente. Une violation du protocole ferme le flux, ou la connexion.
//! - À la déconnexion, tous les flux ouverts échouent avec `tunnel_closed`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Notify, Semaphore};
use uuid::Uuid;

pub const PROTOCOL: u32 = 1;
pub const SUBPROTOCOL: &str = "looty-tunnel.v1";
/// Octets de données d'une trame (hors en-tête de 5 octets).
pub const MAX_FRAME: usize = 64 * 1024;
/// Crédit initial de chaque sens de chaque flux, en octets.
pub const INITIAL_WINDOW: usize = 256 * 1024;
pub const MAX_STREAMS: usize = 16;
pub const MAX_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_PATH: usize = 2048;
/// Message WebSocket relayé (réassemblé depuis plusieurs trames).
pub const MAX_MESSAGE: usize = 8 * 1024 * 1024;
pub const MAX_REQUEST_BODY: usize = 16 * 1024 * 1024;
pub const MAX_RESPONSE_BODY: usize = 64 * 1024 * 1024;
pub const MAX_SERVICES: usize = 16;
pub const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
pub const HEAD_TIMEOUT: Duration = Duration::from_secs(30);
pub const HTTP_IDLE: Duration = Duration::from_secs(60);
pub const WS_IDLE: Duration = Duration::from_secs(30 * 60);
pub const PING_EVERY: Duration = Duration::from_secs(15);
pub const DEAD_AFTER: Duration = Duration::from_secs(45);

pub const FLAG_TEXT: u8 = 0x01;
pub const FLAG_MORE: u8 = 0x02;

pub const KINDS: [&str; 3] = ["jupyter", "latex", "http"];

pub type Headers = Vec<(String, String)>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceDecl {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Proto {
    Http,
    Ws,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub max_streams: usize,
    pub max_frame: usize,
    pub initial_window: usize,
    pub max_header_bytes: usize,
    pub max_message: usize,
    pub max_services: usize,
}

pub fn limits() -> Limits {
    limits_for(MAX_STREAMS)
}

/// Limites annoncées à l'agent, avec le nombre de flux réglé par l'administrateur.
pub fn limits_for(max_streams: usize) -> Limits {
    Limits { max_streams, max_frame: MAX_FRAME, initial_window: INITIAL_WINDOW, max_header_bytes: MAX_HEADER_BYTES, max_message: MAX_MESSAGE, max_services: MAX_SERVICES }
}

/// Messages de contrôle (trames texte JSON, champ `t`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Ctl {
    Hello { v: u32, agent: String, services: Vec<ServiceDecl> },
    Welcome { v: u32, limits: Limits, ping_interval: u64 },
    Services { services: Vec<ServiceDecl> },
    Open { id: u32, service: String, proto: Proto, method: String, path: String, headers: Headers },
    Head { id: u32, status: u16, headers: Headers },
    End { id: u32 },
    Window { id: u32, add: u32 },
    Error { id: u32, code: String, #[serde(default)] message: Option<String> },
    Ping { n: u64 },
    Pong { n: u64 },
    Bye { code: String, #[serde(default)] message: Option<String> },
}

#[derive(Debug)]
pub enum Out {
    Text(String),
    Binary(Vec<u8>),
    Close(u16, String),
}

/// Ce qu'un flux reçoit de l'agent.
#[derive(Debug)]
pub enum Inbound {
    Head { status: u16, headers: Headers },
    Data { bytes: Vec<u8>, flags: u8 },
    End,
    Error { code: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelError {
    Offline,
    NoService,
    TooManyStreams,
    BadRequest,
    Timeout,
    Closed,
    TooLarge,
    /// Réponse HTTP d'un statut inattendu (ouverture WebSocket refusée par le service local).
    Status(u16),
    /// Le type de service est interdit par l'administrateur de l'instance.
    KindNotAllowed,
    /// Tunnel réservé à son propriétaire (`owner_only`).
    OwnerOnly,
    /// Erreur signalée par l'agent (`unreachable`, `refused`…).
    Agent(String),
}

impl TunnelError {
    pub fn code(&self) -> String {
        match self {
            TunnelError::Offline | TunnelError::Closed => "tunnel_offline".into(),
            TunnelError::NoService => "tunnel_no_service".into(),
            TunnelError::TooManyStreams => "tunnel_busy".into(),
            TunnelError::BadRequest => "tunnel_bad_request".into(),
            TunnelError::Timeout => "tunnel_timeout".into(),
            TunnelError::TooLarge => "tunnel_too_large".into(),
            TunnelError::Status(_) => "tunnel_status".into(),
            TunnelError::KindNotAllowed => "tunnel_kind_not_allowed".into(),
            TunnelError::OwnerOnly => "tunnel_owner_only".into(),
            TunnelError::Agent(c) => format!("tunnel_agent_{c}"),
        }
    }
}

/// Nom de service : minuscules, chiffres, `-` et `_`, 1 à 32 caractères.
pub fn valid_service_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
}

pub fn validate_services(list: &[ServiceDecl]) -> bool {
    list.len() <= MAX_SERVICES
        && list.iter().all(|s| valid_service_name(&s.name) && KINDS.contains(&s.kind.as_str()))
        && list.iter().enumerate().all(|(i, s)| list[..i].iter().all(|o| o.name != s.name))
}

fn valid_path(p: &str) -> bool {
    p.starts_with('/') && !p.starts_with("//") && p.len() <= MAX_PATH && !p.contains("..") && p.chars().all(|c| !c.is_control() && c != ' ' && c != '\\')
}

fn headers_size(h: &Headers) -> usize {
    h.iter().map(|(k, v)| k.len() + v.len() + 4).sum()
}

fn valid_headers(h: &Headers) -> bool {
    headers_size(h) <= MAX_HEADER_BYTES && h.len() <= 64 && h.iter().all(|(k, v)| !k.is_empty() && !k.chars().any(|c| c.is_control() || c == ':' || c == ' ') && !v.chars().any(|c| c == '\r' || c == '\n'))
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

struct Entry {
    tx: mpsc::UnboundedSender<Inbound>,
    /// Crédit d'envoi vers l'agent.
    credit: Arc<Semaphore>,
    /// Octets reçus de l'agent et pas encore consommés.
    pending: Arc<AtomicUsize>,
}

pub struct Session {
    pub id: Uuid,
    /// Propriétaire du jeton.
    pub user: Uuid,
    pub since: Instant,
    pub agent: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    out: mpsc::Sender<Out>,
    streams: Mutex<HashMap<u32, Entry>>,
    next: AtomicU32,
    services: RwLock<Vec<ServiceDecl>>,
    closed: AtomicBool,
    pub closed_notify: Notify,
    ping_sent: Mutex<Option<(u64, Instant)>>,
    ping_n: AtomicU64,
    rtt_ms: AtomicI64,
    last_rx: Mutex<Instant>,
    pub bytes_in: AtomicU64,
    pub bytes_out: AtomicU64,
    // Réglages d'instance appliqués en direct (`apply`) : flux simultanés, débit (octets/s, 0 = illimité),
    // types de service permis, tunnel réservé à son propriétaire.
    max_streams: AtomicUsize,
    rate: AtomicU64,
    kinds: RwLock<Vec<String>>,
    owner_only: AtomicBool,
    pace_at: Mutex<Instant>,
    reason: Mutex<Option<String>>,
}

/// Réglages d'instance qui s'appliquent à une connexion déjà ouverte.
#[derive(Debug, Clone, PartialEq)]
pub struct Live {
    pub max_streams: usize,
    pub bytes_per_sec: u64,
    pub kinds: Vec<String>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Session {
    pub fn new(id: Uuid, user: Uuid, agent: String, expires_at: chrono::DateTime<chrono::Utc>, services: Vec<ServiceDecl>, out: mpsc::Sender<Out>) -> Arc<Session> {
        Arc::new(Session {
            id,
            user,
            since: Instant::now(),
            agent,
            expires_at,
            out,
            streams: Mutex::new(HashMap::new()),
            next: AtomicU32::new(1),
            services: RwLock::new(services),
            closed: AtomicBool::new(false),
            closed_notify: Notify::new(),
            ping_sent: Mutex::new(None),
            ping_n: AtomicU64::new(0),
            rtt_ms: AtomicI64::new(-1),
            last_rx: Mutex::new(Instant::now()),
            bytes_in: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            max_streams: AtomicUsize::new(MAX_STREAMS),
            rate: AtomicU64::new(0),
            kinds: RwLock::new(KINDS.iter().map(|k| k.to_string()).collect()),
            owner_only: AtomicBool::new(false),
            pace_at: Mutex::new(Instant::now()),
            reason: Mutex::new(None),
        })
    }

    /// Applique les réglages d'instance (à la connexion, puis à chaque revalidation).
    pub fn apply(&self, live: &Live) {
        self.max_streams.store(live.max_streams.clamp(1, 64), Ordering::Relaxed);
        self.rate.store(live.bytes_per_sec, Ordering::Relaxed);
        *self.kinds.write().unwrap_or_else(|e| e.into_inner()) = live.kinds.clone();
    }

    pub fn max_streams(&self) -> usize {
        self.max_streams.load(Ordering::Relaxed)
    }

    pub fn kind_allowed(&self, kind: &str) -> bool {
        self.kinds.read().unwrap_or_else(|e| e.into_inner()).iter().any(|k| k == kind)
    }

    pub fn set_owner_only(&self, v: bool) {
        self.owner_only.store(v, Ordering::SeqCst);
    }

    pub fn owner_only(&self) -> bool {
        self.owner_only.load(Ordering::SeqCst)
    }

    /// Pourquoi la session s'est fermée (`revoked`, `expired`, `lost`…), pour l'audit.
    pub fn reason(&self) -> String {
        lock(&self.reason).clone().unwrap_or_else(|| "lost".into())
    }

    /// Plafond de débit du tunnel (tous flux et les deux sens confondus) : retarde l'appelant
    /// le temps qu'il faudrait pour rester sous le plafond. Une rafale initiale passe.
    pub async fn pace(&self, bytes: usize) {
        let rate = self.rate.load(Ordering::Relaxed);
        if rate == 0 {
            return;
        }
        let wait = {
            let mut at = lock(&self.pace_at);
            let now = Instant::now();
            let start = (*at).max(now);
            *at = start + Duration::from_secs_f64(bytes as f64 / rate as f64);
            start.saturating_duration_since(now).min(Duration::from_secs(10))
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub fn services(&self) -> Vec<ServiceDecl> {
        self.services.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Dernière latence mesurée par ping, en millisecondes.
    pub fn latency_ms(&self) -> Option<i64> {
        Some(self.rtt_ms.load(Ordering::Relaxed)).filter(|v| *v >= 0)
    }

    pub fn open_streams(&self) -> usize {
        lock(&self.streams).len()
    }

    pub fn idle_for(&self) -> Duration {
        lock(&self.last_rx).elapsed()
    }

    async fn send_ctl(&self, c: &Ctl) -> Result<(), TunnelError> {
        let s = serde_json::to_string(c).map_err(|_| TunnelError::BadRequest)?;
        self.out.send(Out::Text(s)).await.map_err(|_| TunnelError::Closed)
    }

    fn try_ctl(&self, c: &Ctl) {
        if let Ok(s) = serde_json::to_string(c) {
            let _ = self.out.try_send(Out::Text(s));
        }
    }

    pub async fn welcome(&self) -> Result<(), TunnelError> {
        self.send_ctl(&Ctl::Welcome { v: PROTOCOL, limits: limits_for(self.max_streams()), ping_interval: PING_EVERY.as_secs() }).await
    }

    /// Envoie un ping et note l'heure ; la réponse (`pong`) donne la latence.
    pub async fn ping(&self) -> Result<(), TunnelError> {
        let n = self.ping_n.fetch_add(1, Ordering::SeqCst) + 1;
        *lock(&self.ping_sent) = Some((n, Instant::now()));
        self.send_ctl(&Ctl::Ping { n }).await
    }

    /// Ferme proprement : `bye` puis fermeture WebSocket, et échec de tous les flux.
    pub fn close(&self, code: u16, bye: &str) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        *lock(&self.reason) = Some(bye.to_string());
        self.try_ctl(&Ctl::Bye { code: bye.to_string(), message: None });
        let _ = self.out.try_send(Out::Close(code, bye.to_string()));
        self.fail_all();
        self.closed_notify.notify_one();
    }

    /// Connexion perdue : même nettoyage, sans rien envoyer.
    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.fail_all();
        self.closed_notify.notify_one();
    }

    fn fail_all(&self) {
        let entries: Vec<Entry> = lock(&self.streams).drain().map(|(_, e)| e).collect();
        for e in entries {
            e.credit.close();
            let _ = e.tx.send(Inbound::Error { code: "tunnel_closed".into() });
        }
    }

    /// Ouvre un flux vers un service déclaré du type attendu.
    pub async fn open(self: &Arc<Self>, service: &str, kind: &str, proto: Proto, method: &str, path: &str, headers: Headers) -> Result<Stream, TunnelError> {
        if self.is_closed() {
            return Err(TunnelError::Offline);
        }
        if !self.kind_allowed(kind) {
            return Err(TunnelError::KindNotAllowed);
        }
        if !self.services().iter().any(|s| s.name == service && s.kind == kind) {
            return Err(TunnelError::NoService);
        }
        if !valid_path(path) || !valid_headers(&headers) || !method.chars().all(|c| c.is_ascii_uppercase()) || method.is_empty() || method.len() > 16 {
            return Err(TunnelError::BadRequest);
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let credit = Arc::new(Semaphore::new(INITIAL_WINDOW));
        let pending = Arc::new(AtomicUsize::new(0));
        let id = {
            let mut map = lock(&self.streams);
            if map.len() >= self.max_streams() {
                return Err(TunnelError::TooManyStreams);
            }
            let id = self.next.fetch_add(1, Ordering::SeqCst);
            map.insert(id, Entry { tx, credit: credit.clone(), pending: pending.clone() });
            id
        };
        let guard = Arc::new(Guard { id, session: self.clone(), remote_done: AtomicBool::new(false), local_done: AtomicBool::new(false) });
        self.send_ctl(&Ctl::Open { id, service: service.into(), proto, method: method.into(), path: path.into(), headers }).await?;
        Ok(Stream {
            tx: StreamTx { id, session: self.clone(), credit, guard: guard.clone() },
            rx: StreamRx { id, session: self.clone(), rx, pending, guard, partial: Vec::new(), partial_text: false },
        })
    }

    // -- Réception de l'agent ------------------------------------------------

    /// Trame texte reçue. `Err` : violation du protocole, la connexion doit être fermée.
    pub async fn on_text(&self, s: &str) -> Result<(), &'static str> {
        *lock(&self.last_rx) = Instant::now();
        let c: Ctl = serde_json::from_str(s).map_err(|_| "bad_message")?;
        match c {
            Ctl::Ping { n } => {
                let _ = self.send_ctl(&Ctl::Pong { n }).await;
            }
            Ctl::Pong { n } => {
                let mut sent = lock(&self.ping_sent);
                if let Some((m, at)) = *sent {
                    if m == n {
                        self.rtt_ms.store(at.elapsed().as_millis() as i64, Ordering::Relaxed);
                        *sent = None;
                    }
                }
            }
            Ctl::Services { services } => {
                if !validate_services(&services) {
                    return Err("bad_services");
                }
                if services.iter().any(|s| !self.kind_allowed(&s.kind)) {
                    return Err("tunnel_kind_not_allowed");
                }
                *self.services.write().unwrap_or_else(|e| e.into_inner()) = services;
            }
            Ctl::Head { id, status, headers } => {
                if !(100..=599).contains(&status) || !valid_headers(&headers) {
                    self.stream_error(id, "bad_head");
                } else {
                    self.deliver(id, Inbound::Head { status, headers });
                }
            }
            Ctl::End { id } => self.deliver(id, Inbound::End),
            Ctl::Error { id, code, .. } => {
                let code: String = code.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(48).collect();
                let entry = lock(&self.streams).remove(&id);
                if let Some(e) = entry {
                    e.credit.close();
                    let _ = e.tx.send(Inbound::Error { code });
                }
            }
            Ctl::Window { id, add } => {
                let credit = lock(&self.streams).get(&id).map(|e| e.credit.clone());
                if let Some(c) = credit {
                    if add == 0 || c.available_permits() + add as usize > INITIAL_WINDOW {
                        self.stream_error(id, "flow_control");
                    } else {
                        c.add_permits(add as usize);
                    }
                }
            }
            Ctl::Hello { .. } | Ctl::Welcome { .. } | Ctl::Open { .. } | Ctl::Bye { .. } => return Err("unexpected_message"),
        }
        Ok(())
    }

    /// Trame binaire (données d'un flux) : `id` (4 octets), `flags`, données.
    pub fn on_binary(&self, b: &[u8]) -> Result<(), &'static str> {
        *lock(&self.last_rx) = Instant::now();
        if b.len() < 5 || b.len() - 5 > MAX_FRAME {
            return Err("bad_frame");
        }
        let id = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        let flags = b[4];
        if flags & !(FLAG_TEXT | FLAG_MORE) != 0 {
            return Err("bad_frame");
        }
        let data = &b[5..];
        self.bytes_in.fetch_add(data.len() as u64, Ordering::Relaxed);
        let target = lock(&self.streams).get(&id).map(|e| (e.tx.clone(), e.pending.clone()));
        let Some((tx, pending)) = target else { return Ok(()) };
        let size = data.len().max(1);
        if pending.fetch_add(size, Ordering::SeqCst) + size > INITIAL_WINDOW {
            self.stream_error(id, "flow_control");
            return Ok(());
        }
        let _ = tx.send(Inbound::Data { bytes: data.to_vec(), flags });
        Ok(())
    }

    fn deliver(&self, id: u32, m: Inbound) {
        let tx = lock(&self.streams).get(&id).map(|e| e.tx.clone());
        if let Some(tx) = tx {
            let _ = tx.send(m);
        }
    }

    /// Termine un flux fautif : l'agent est prévenu, le consommateur reçoit l'erreur.
    fn stream_error(&self, id: u32, code: &str) {
        let entry = lock(&self.streams).remove(&id);
        if let Some(e) = entry {
            e.credit.close();
            let _ = e.tx.send(Inbound::Error { code: code.to_string() });
            self.try_ctl(&Ctl::Error { id, code: code.to_string(), message: None });
        }
    }
}

// ---------------------------------------------------------------------------
// Flux
// ---------------------------------------------------------------------------

/// Nettoyage commun aux deux moitiés d'un flux : libère la place, prévient l'agent.
struct Guard {
    id: u32,
    session: Arc<Session>,
    remote_done: AtomicBool,
    local_done: AtomicBool,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let had = lock(&self.session.streams).remove(&self.id).is_some();
        if !had {
            return;
        }
        if !self.remote_done.load(Ordering::SeqCst) {
            self.session.try_ctl(&Ctl::Error { id: self.id, code: "cancelled".into(), message: None });
        } else if !self.local_done.load(Ordering::SeqCst) {
            self.session.try_ctl(&Ctl::End { id: self.id });
        }
    }
}

pub struct Stream {
    pub tx: StreamTx,
    pub rx: StreamRx,
}

pub struct StreamTx {
    id: u32,
    session: Arc<Session>,
    credit: Arc<Semaphore>,
    guard: Arc<Guard>,
}

pub struct StreamRx {
    id: u32,
    session: Arc<Session>,
    rx: mpsc::UnboundedReceiver<Inbound>,
    pending: Arc<AtomicUsize>,
    guard: Arc<Guard>,
    partial: Vec<u8>,
    partial_text: bool,
}

impl StreamTx {
    async fn frame(&self, flags: u8, chunk: &[u8]) -> Result<(), TunnelError> {
        let n = chunk.len().max(1) as u32;
        let permit = tokio::time::timeout(HTTP_IDLE, self.credit.acquire_many(n)).await.map_err(|_| TunnelError::Timeout)?.map_err(|_| TunnelError::Closed)?;
        permit.forget();
        self.session.pace(chunk.len() + 5).await;
        let mut buf = Vec::with_capacity(5 + chunk.len());
        buf.extend_from_slice(&self.id.to_be_bytes());
        buf.push(flags);
        buf.extend_from_slice(chunk);
        self.session.bytes_out.fetch_add(chunk.len() as u64, Ordering::Relaxed);
        self.session.out.send(Out::Binary(buf)).await.map_err(|_| TunnelError::Closed)
    }

    /// Corps HTTP : découpé en trames sans drapeau.
    pub async fn send_body(&self, data: &[u8]) -> Result<(), TunnelError> {
        for c in data.chunks(MAX_FRAME) {
            self.frame(0, c).await?;
        }
        Ok(())
    }

    /// Message WebSocket : une ou plusieurs trames, `MORE` sauf la dernière.
    pub async fn send_message(&self, text: bool, data: &[u8]) -> Result<(), TunnelError> {
        if data.len() > MAX_MESSAGE {
            return Err(TunnelError::TooLarge);
        }
        let base = if text { FLAG_TEXT } else { 0 };
        if data.is_empty() {
            return self.frame(base, &[]).await;
        }
        let n = data.chunks(MAX_FRAME).count();
        for (i, c) in data.chunks(MAX_FRAME).enumerate() {
            self.frame(if i + 1 < n { base | FLAG_MORE } else { base }, c).await?;
        }
        Ok(())
    }

    /// Plus rien à envoyer sur ce flux (fin du corps, fermeture de la WebSocket).
    pub async fn end(&self) -> Result<(), TunnelError> {
        self.guard.local_done.store(true, Ordering::SeqCst);
        self.session.send_ctl(&Ctl::End { id: self.id }).await
    }
}

/// Message WebSocket réassemblé.
#[derive(Debug, PartialEq)]
pub struct WsMessage {
    pub text: bool,
    pub data: Vec<u8>,
}

impl StreamRx {
    /// Prochain événement ; rend le crédit à l'agent pour les données consommées.
    pub async fn recv(&mut self, idle: Duration) -> Result<Inbound, TunnelError> {
        let m = match tokio::time::timeout(idle, self.rx.recv()).await {
            Err(_) => return Err(TunnelError::Timeout),
            Ok(None) => Inbound::Error { code: "tunnel_closed".into() },
            Ok(Some(m)) => m,
        };
        match &m {
            Inbound::Data { bytes, .. } => {
                let n = bytes.len().max(1);
                self.pending.fetch_sub(n, Ordering::SeqCst);
                let _ = self.session.send_ctl(&Ctl::Window { id: self.id, add: n as u32 }).await;
            }
            Inbound::End | Inbound::Error { .. } => self.guard.remote_done.store(true, Ordering::SeqCst),
            Inbound::Head { .. } => {}
        }
        Ok(m)
    }

    /// Réponse d'ouverture : `head`, ou l'erreur du flux.
    pub async fn head(&mut self) -> Result<(u16, Headers), TunnelError> {
        match self.recv(HEAD_TIMEOUT).await? {
            Inbound::Head { status, headers } => Ok((status, headers)),
            Inbound::Error { code } if code == "tunnel_closed" => Err(TunnelError::Closed),
            Inbound::Error { code } => Err(TunnelError::Agent(code)),
            _ => Err(TunnelError::Agent("protocol".into())),
        }
    }

    /// Prochain message WebSocket entier ; `None` quand l'autre côté ferme.
    pub async fn recv_message(&mut self) -> Option<Result<WsMessage, TunnelError>> {
        loop {
            match self.recv(WS_IDLE).await {
                Err(e) => return Some(Err(e)),
                Ok(Inbound::End) => return None,
                Ok(Inbound::Error { code }) if code == "tunnel_closed" => return Some(Err(TunnelError::Closed)),
                Ok(Inbound::Error { code }) => return Some(Err(TunnelError::Agent(code))),
                Ok(Inbound::Head { .. }) => return Some(Err(TunnelError::Agent("protocol".into()))),
                Ok(Inbound::Data { bytes, flags }) => {
                    if self.partial.is_empty() {
                        self.partial_text = flags & FLAG_TEXT != 0;
                    }
                    if self.partial.len() + bytes.len() > MAX_MESSAGE {
                        return Some(Err(TunnelError::TooLarge));
                    }
                    self.partial.extend_from_slice(&bytes);
                    if flags & FLAG_MORE == 0 {
                        return Some(Ok(WsMessage { text: self.partial_text, data: std::mem::take(&mut self.partial) }));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Aides pour les appelants
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct HttpReply {
    pub status: u16,
    pub headers: Headers,
    pub body: Vec<u8>,
}

/// Requête HTTP complète à travers le tunnel (corps en mémoire, tailles bornées).
pub async fn http(session: &Arc<Session>, service: &str, kind: &str, method: &str, path: &str, headers: Headers, body: Option<Vec<u8>>) -> Result<HttpReply, TunnelError> {
    let body = body.unwrap_or_default();
    if body.len() > MAX_REQUEST_BODY {
        return Err(TunnelError::TooLarge);
    }
    let Stream { tx, mut rx } = session.open(service, kind, Proto::Http, method, path, headers).await?;
    tx.send_body(&body).await?;
    tx.end().await?;
    let (status, headers) = rx.head().await?;
    let mut out = Vec::new();
    loop {
        match rx.recv(HTTP_IDLE).await? {
            Inbound::Data { bytes, .. } => {
                if out.len() + bytes.len() > MAX_RESPONSE_BODY {
                    return Err(TunnelError::TooLarge);
                }
                out.extend_from_slice(&bytes);
            }
            Inbound::End => break,
            Inbound::Error { code } if code == "tunnel_closed" => return Err(TunnelError::Closed),
            Inbound::Error { code } => return Err(TunnelError::Agent(code)),
            Inbound::Head { .. } => return Err(TunnelError::Agent("protocol".into())),
        }
    }
    Ok(HttpReply { status, headers, body: out })
}

/// Ouvre une WebSocket vers un service local ; l'agent répond `head` 101 si elle est acceptée.
pub async fn ws_open(session: &Arc<Session>, service: &str, kind: &str, path: &str, headers: Headers) -> Result<Stream, TunnelError> {
    let mut s = session.open(service, kind, Proto::Ws, "GET", path, headers).await?;
    let (status, _) = s.rx.head().await?;
    if status != 101 {
        return Err(TunnelError::Status(status));
    }
    Ok(s)
}

// ---------------------------------------------------------------------------
// Registre des tunnels connectés
// ---------------------------------------------------------------------------

static REGISTRY: std::sync::LazyLock<dashmap::DashMap<Uuid, Arc<Session>>> = std::sync::LazyLock::new(dashmap::DashMap::new);

pub fn connected(id: Uuid) -> Option<Arc<Session>> {
    REGISTRY.get(&id).map(|s| s.clone()).filter(|s| !s.is_closed())
}

/// Enregistre la session ; une connexion précédente du même jeton est fermée (code 4009).
pub fn register(s: Arc<Session>) {
    if let Some(old) = REGISTRY.insert(s.id, s) {
        old.close(4009, "replaced");
    }
}

/// Retire la session si c'est toujours celle du registre.
pub fn unregister(s: &Arc<Session>) {
    REGISTRY.remove_if(&s.id, |_, cur| Arc::ptr_eq(cur, s));
}

/// Session connectée pour quelqu'un qui veut s'en servir : refusée si le tunnel est réservé à son
/// propriétaire et que ce n'est pas lui. Tout appelant qui agit pour une personne passe par là.
pub fn connected_for(id: Uuid, caller: Uuid) -> Result<Arc<Session>, TunnelError> {
    let s = connected(id).ok_or(TunnelError::Offline)?;
    if s.owner_only() && s.user != caller {
        return Err(TunnelError::OwnerOnly);
    }
    Ok(s)
}

/// Toutes les sessions connectées (réglages en direct, fermeture générale).
pub fn sessions() -> Vec<Arc<Session>> {
    REGISTRY.iter().map(|e| e.value().clone()).filter(|s| !s.is_closed()).collect()
}

/// Ferme la connexion d'un jeton (révocation, expiration).
pub fn disconnect(id: Uuid, code: u16, reason: &str) {
    if let Some((_, s)) = REGISTRY.remove(&id) {
        s.close(code, reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faux agent : rejoue ce que le relais écrit, selon un script par service.
    struct Fake {
        session: Arc<Session>,
        out: mpsc::Receiver<Out>,
    }

    fn fake(services: &[(&str, &str)]) -> Fake {
        let (tx, out) = mpsc::channel(256);
        let decl = services.iter().map(|(n, k)| ServiceDecl { name: n.to_string(), kind: k.to_string() }).collect();
        let session = Session::new(Uuid::new_v4(), Uuid::new_v4(), "test".into(), chrono::Utc::now() + chrono::Duration::days(1), decl, tx);
        Fake { session, out }
    }

    fn frame(id: u32, flags: u8, data: &[u8]) -> Vec<u8> {
        let mut b = id.to_be_bytes().to_vec();
        b.push(flags);
        b.extend_from_slice(data);
        b
    }

    async fn next_text(f: &mut Fake) -> Ctl {
        loop {
            match tokio::time::timeout(Duration::from_secs(2), f.out.recv()).await.unwrap().unwrap() {
                Out::Text(t) => return serde_json::from_str(&t).unwrap(),
                Out::Binary(_) | Out::Close(..) => {}
            }
        }
    }

    async fn next_binary(f: &mut Fake) -> Vec<u8> {
        loop {
            match tokio::time::timeout(Duration::from_secs(2), f.out.recv()).await.unwrap().unwrap() {
                Out::Binary(b) => return b,
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn http_goes_through_a_declared_service_only() {
        let mut f = fake(&[("pc", "jupyter")]);
        assert_eq!(http(&f.session, "other", "jupyter", "GET", "/api", vec![], None).await.err(), Some(TunnelError::NoService));
        assert_eq!(http(&f.session, "pc", "latex", "GET", "/api", vec![], None).await.err(), Some(TunnelError::NoService));
        assert_eq!(http(&f.session, "pc", "jupyter", "GET", "/../etc", vec![], None).await.err(), Some(TunnelError::BadRequest));
        assert_eq!(http(&f.session, "pc", "jupyter", "GET", "//evil.example/x", vec![], None).await.err(), Some(TunnelError::BadRequest));
        let s = f.session.clone();
        let call = tokio::spawn(async move { http(&s, "pc", "jupyter", "POST", "/api/kernels", vec![("content-type".into(), "application/json".into())], Some(b"{\"a\":1}".to_vec())).await });
        let Ctl::Open { id, service, proto, method, path, .. } = next_text(&mut f).await else { panic!("open expected") };
        assert_eq!((service.as_str(), proto, method.as_str(), path.as_str()), ("pc", Proto::Http, "POST", "/api/kernels"));
        assert_eq!(next_binary(&mut f).await[5..], *b"{\"a\":1}");
        assert!(matches!(next_text(&mut f).await, Ctl::End { .. }));
        f.session.on_text(&serde_json::to_string(&Ctl::Head { id, status: 201, headers: vec![] }).unwrap()).await.unwrap();
        f.session.on_binary(&frame(id, 0, b"{\"id\":\"k\"}")).unwrap();
        f.session.on_text(&serde_json::to_string(&Ctl::End { id }).unwrap()).await.unwrap();
        let r = call.await.unwrap().unwrap();
        assert_eq!((r.status, r.body.as_slice()), (201, b"{\"id\":\"k\"}".as_slice()));
        assert_eq!(f.session.open_streams(), 0);
    }

    #[tokio::test]
    async fn websocket_messages_are_split_and_reassembled() {
        let mut f = fake(&[("pc", "jupyter")]);
        let s = f.session.clone();
        let open = tokio::spawn(async move { ws_open(&s, "pc", "jupyter", "/api/kernels/k/channels", vec![]).await });
        let Ctl::Open { id, proto, .. } = next_text(&mut f).await else { panic!() };
        assert_eq!(proto, Proto::Ws);
        f.session.on_text(&serde_json::to_string(&Ctl::Head { id, status: 101, headers: vec![] }).unwrap()).await.unwrap();
        let Stream { tx, mut rx } = open.await.unwrap().unwrap();
        // Un message de 150 000 octets part en trois trames, les deux premières avec MORE.
        let big = vec![b'x'; 150_000];
        let send = tokio::spawn(async move {
            tx.send_message(true, &big).await.unwrap();
            tx
        });
        let mut got = Vec::new();
        for i in 0..3 {
            let b = next_binary(&mut f).await;
            assert_eq!(b[4], FLAG_TEXT | if i < 2 { FLAG_MORE } else { 0 });
            got.extend_from_slice(&b[5..]);
            // Rend le crédit comme le ferait l'agent.
            f.session.on_text(&serde_json::to_string(&Ctl::Window { id, add: (b.len() - 5) as u32 }).unwrap()).await.ok();
        }
        assert_eq!(got.len(), 150_000);
        let _tx = send.await.unwrap();
        // Réception : deux trames pour un message.
        f.session.on_binary(&frame(id, FLAG_TEXT | FLAG_MORE, b"hel")).unwrap();
        f.session.on_binary(&frame(id, FLAG_TEXT, b"lo")).unwrap();
        assert_eq!(rx.recv_message().await.unwrap().unwrap(), WsMessage { text: true, data: b"hello".to_vec() });
        f.session.on_text(&serde_json::to_string(&Ctl::End { id }).unwrap()).await.unwrap();
        assert!(rx.recv_message().await.is_none());
    }

    #[tokio::test]
    async fn the_window_limits_what_is_sent_until_credit_returns() {
        let mut f = fake(&[("pc", "http")]);
        let s = f.session.clone();
        let call = tokio::spawn(async move { http(&s, "pc", "http", "PUT", "/up", vec![], Some(vec![1u8; INITIAL_WINDOW + 1000])).await });
        let Ctl::Open { id, .. } = next_text(&mut f).await else { panic!() };
        let mut sent = 0;
        while sent < INITIAL_WINDOW {
            sent += next_binary(&mut f).await.len() - 5;
        }
        // Plus de crédit : rien ne part tant que l'agent n'en rend pas.
        assert!(tokio::time::timeout(Duration::from_millis(200), f.out.recv()).await.is_err());
        f.session.on_text(&serde_json::to_string(&Ctl::Window { id, add: 1000 }).unwrap()).await.unwrap();
        assert_eq!(next_binary(&mut f).await.len() - 5, 1000);
        call.abort();
    }

    #[tokio::test]
    async fn an_agent_that_overruns_the_window_or_credit_gets_the_stream_cut() {
        let mut f = fake(&[("pc", "http")]);
        let mut s = f.session.open("pc", "http", Proto::Http, "GET", "/big", vec![]).await.unwrap();
        let Ctl::Open { id, .. } = next_text(&mut f).await else { panic!() };
        // Rien n'est consommé : au-delà de la fenêtre, le flux est coupé et l'agent prévenu.
        for _ in 0..(INITIAL_WINDOW / MAX_FRAME + 1) {
            f.session.on_binary(&frame(id, 0, &vec![0u8; MAX_FRAME])).unwrap();
        }
        assert!(matches!(next_text(&mut f).await, Ctl::Error { code, .. } if code == "flow_control"));
        let mut last = None;
        while let Ok(m) = s.rx.recv(Duration::from_millis(100)).await {
            let done = matches!(m, Inbound::Error { .. });
            last = Some(m);
            if done {
                break;
            }
        }
        assert!(matches!(last, Some(Inbound::Error { ref code }) if code == "flow_control"));
        // Crédit rendu en trop : même sanction.
        let mut g = fake(&[("pc", "http")]);
        let _s = g.session.open("pc", "http", Proto::Http, "GET", "/x", vec![]).await.unwrap();
        let Ctl::Open { id, .. } = next_text(&mut g).await else { panic!() };
        g.session.on_text(&serde_json::to_string(&Ctl::Window { id, add: 10 }).unwrap()).await.unwrap();
        assert!(matches!(next_text(&mut g).await, Ctl::Error { code, .. } if code == "flow_control"));
    }

    #[tokio::test]
    async fn stream_count_and_protocol_violations_are_limited() {
        let mut f = fake(&[("pc", "jupyter")]);
        let mut streams = Vec::new();
        for _ in 0..MAX_STREAMS {
            streams.push(f.session.open("pc", "jupyter", Proto::Ws, "GET", "/a", vec![]).await.unwrap());
        }
        assert_eq!(f.session.open("pc", "jupyter", Proto::Ws, "GET", "/a", vec![]).await.err(), Some(TunnelError::TooManyStreams));
        drop(streams.pop());
        assert!(f.session.open("pc", "jupyter", Proto::Ws, "GET", "/a", vec![]).await.is_ok());
        // Trames invalides ou messages réservés à l'instance : fermeture de la connexion.
        assert!(f.session.on_binary(&[0, 0]).is_err());
        assert!(f.session.on_binary(&frame(1, 0x80, b"x")).is_err());
        assert!(f.session.on_binary(&vec![0u8; MAX_FRAME + 6]).is_err());
        assert!(f.session.on_text("not json").await.is_err());
        assert!(f.session.on_text(r#"{"t":"open","id":1,"service":"pc","proto":"http","method":"GET","path":"/","headers":[]}"#).await.is_err());
        assert!(f.session.on_text(r#"{"t":"services","services":[{"name":"Bad Name","kind":"jupyter"}]}"#).await.is_err());
        // Données pour un flux inconnu : ignorées.
        assert!(f.session.on_binary(&frame(999, 0, b"x")).is_ok());
        let _ = &mut f;
    }

    #[tokio::test]
    async fn closing_fails_every_open_stream_and_cleans_up() {
        let f = fake(&[("pc", "jupyter")]);
        let mut a = f.session.open("pc", "jupyter", Proto::Ws, "GET", "/a", vec![]).await.unwrap();
        let b = f.session.open("pc", "jupyter", Proto::Ws, "GET", "/b", vec![]).await.unwrap();
        assert_eq!(f.session.open_streams(), 2);
        f.session.shutdown();
        assert!(matches!(a.rx.head().await, Err(TunnelError::Closed)));
        assert_eq!(b.tx.send_body(b"x").await.err(), Some(TunnelError::Closed));
        assert_eq!(f.session.open("pc", "jupyter", Proto::Ws, "GET", "/c", vec![]).await.err(), Some(TunnelError::Offline));
        assert_eq!(f.session.open_streams(), 0);
    }

    #[tokio::test]
    async fn dropping_a_stream_cancels_it_on_the_agent() {
        let mut f = fake(&[("pc", "jupyter")]);
        let s = f.session.open("pc", "jupyter", Proto::Ws, "GET", "/a", vec![]).await.unwrap();
        assert!(matches!(next_text(&mut f).await, Ctl::Open { .. }));
        drop(s);
        assert!(matches!(next_text(&mut f).await, Ctl::Error { code, .. } if code == "cancelled"));
        assert_eq!(f.session.open_streams(), 0);
    }

    #[tokio::test]
    async fn ping_measures_latency_and_the_registry_replaces_old_connections() {
        let mut f = fake(&[]);
        f.session.ping().await.unwrap();
        let Ctl::Ping { n } = next_text(&mut f).await else { panic!() };
        tokio::time::sleep(Duration::from_millis(20)).await;
        f.session.on_text(&format!(r#"{{"t":"pong","n":{n}}}"#)).await.unwrap();
        assert!(f.session.latency_ms().unwrap() >= 20);
        // Un ping de l'agent reçoit un pong.
        f.session.on_text(r#"{"t":"ping","n":7}"#).await.unwrap();
        assert!(matches!(next_text(&mut f).await, Ctl::Pong { n: 7 }));
        let id = f.session.id;
        register(f.session.clone());
        assert!(connected(id).is_some());
        let (tx, _rx) = mpsc::channel(8);
        let newer = Session::new(id, f.session.user, "b".into(), f.session.expires_at, vec![], tx);
        register(newer.clone());
        assert!(f.session.is_closed());
        unregister(&f.session);
        assert!(connected(id).is_some());
        disconnect(id, 4001, "revoked");
        assert!(connected(id).is_none() && newer.is_closed());
    }

    #[test]
    fn service_declarations_are_validated() {
        let d = |n: &str, k: &str| ServiceDecl { name: n.into(), kind: k.into() };
        assert!(validate_services(&[d("jupyter", "jupyter"), d("tex-1", "latex")]));
        assert!(!validate_services(&[d("a", "jupyter"), d("a", "latex")]));
        assert!(!validate_services(&[d("A", "jupyter")]));
        assert!(!validate_services(&[d("a", "ssh")]));
        assert!(!validate_services(&(0..17).map(|i| d(&format!("s{i}"), "http")).collect::<Vec<_>>()));
    }
}
