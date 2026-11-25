//! One connection to the instance: handshake, streams, flow control.

use crate::config::Services;
use crate::log;
use crate::proto::*;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch, Semaphore};
use tokio::task::AbortHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::{
    frame::coding::CloseCode, CloseFrame, WebSocketConfig,
};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{connect_async_tls_with_config, Connector};

pub const AGENT: &str = concat!("looty-bridge/", env!("CARGO_PKG_VERSION"));
const LOCAL_CONNECT: Duration = Duration::from_secs(5);
const CREDIT_WAIT: Duration = Duration::from_secs(60);
const FIRST_BYTE: Duration = Duration::from_secs(30);
const HTTP_IDLE: Duration = Duration::from_secs(60);
const SILENCE: Duration = Duration::from_secs(45);
const MAX_REQUEST_BODY: usize = 16 * 1024 * 1024;

pub struct Params {
    pub url: String,
    /// Never logged, never formatted.
    pub token: String,
    pub services: Services,
}

pub enum Outcome {
    /// Network or server trouble: reconnect with backoff.
    Retry(String),
    /// Reconnecting cannot help without operator action.
    Fatal(String),
    Shutdown,
}

fn tls() -> Connector {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("tls versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    Connector::Rustls(Arc::new(cfg))
}

#[derive(Clone)]
struct Out(mpsc::Sender<Message>);

impl Out {
    async fn ctl(&self, m: Outgoing<'_>) -> bool {
        self.0.send(Message::text(m.to_text())).await.is_ok()
    }
    async fn data(&self, id: u32, flags: u8, payload: &[u8]) -> bool {
        self.0
            .send(Message::binary(encode_frame(id, flags, payload)))
            .await
            .is_ok()
    }
}

enum In {
    Data { flags: u8, payload: Vec<u8> },
    End,
}

struct Entry {
    tx: mpsc::UnboundedSender<In>,
    credit: Arc<Semaphore>,
    task: AbortHandle,
}

type Streams = Arc<Mutex<HashMap<u32, Entry>>>;

struct SErr {
    code: &'static str,
    msg: String,
}

fn serr(code: &'static str, msg: impl Into<String>) -> SErr {
    SErr {
        code,
        msg: msg.into(),
    }
}

struct Ctx {
    id: u32,
    out: Out,
    limits: Arc<Limits>,
    credit: Arc<Semaphore>,
    rx: mpsc::UnboundedReceiver<In>,
}

impl Ctx {
    /// Gives credit back after `n` payload bytes were consumed locally.
    async fn consumed(&self, n: usize) {
        self.out
            .ctl(Outgoing::Window {
                id: self.id,
                add: n.max(1) as u64,
            })
            .await;
    }

    async fn send_data(&self, flags: u8, payload: &[u8]) -> Result<(), SErr> {
        let cost = payload.len().max(1) as u32;
        match timeout(CREDIT_WAIT, self.credit.acquire_many(cost)).await {
            Ok(Ok(p)) => p.forget(),
            Ok(Err(_)) => return Err(serr("internal", "closed")),
            Err(_) => return Err(serr("timeout", "no credit")),
        }
        if self.out.data(self.id, flags, payload).await {
            Ok(())
        } else {
            Err(serr("internal", "tunnel closed"))
        }
    }

    /// Sends one WebSocket message, split in frames of at most `max_frame`.
    async fn send_message(&self, text: bool, bytes: &[u8]) -> Result<(), SErr> {
        let base = if text { FLAG_TEXT } else { 0 };
        let max = self.limits.max_frame.max(1);
        if bytes.is_empty() {
            return self.send_data(base, &[]).await;
        }
        let mut chunks = bytes.chunks(max).peekable();
        while let Some(c) = chunks.next() {
            let more = if chunks.peek().is_some() {
                FLAG_MORE
            } else {
                0
            };
            self.send_data(base | more, c).await?;
        }
        Ok(())
    }
}

fn http_err(e: &reqwest::Error) -> SErr {
    if e.is_connect() {
        serr("unreachable", e.to_string())
    } else if e.is_timeout() {
        serr("timeout", e.to_string())
    } else {
        serr("protocol", e.to_string())
    }
}

async fn http_stream(
    mut ctx: Ctx,
    client: reqwest::Client,
    base: String,
    method: String,
    path: String,
    headers: Headers,
) -> Result<(), SErr> {
    let mut body = Vec::new();
    loop {
        match ctx.rx.recv().await {
            Some(In::Data { payload, .. }) => {
                if body.len() + payload.len() > MAX_REQUEST_BODY {
                    return Err(serr("too_large", "request body"));
                }
                body.extend_from_slice(&payload);
                ctx.consumed(payload.len()).await;
            }
            Some(In::End) => break,
            None => return Ok(()),
        }
    }
    let method = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|_| serr("protocol", "bad method"))?;
    let mut req = client.request(method.clone(), format!("{base}{path}"));
    for (k, v) in forward_headers(&headers) {
        if let (Ok(k), Ok(v)) = (
            reqwest::header::HeaderName::from_bytes(k.as_bytes()),
            reqwest::header::HeaderValue::from_str(&v),
        ) {
            req = req.header(k, v);
        }
    }
    if !body.is_empty() || !matches!(method, reqwest::Method::GET | reqwest::Method::HEAD) {
        req = req.body(body);
    }
    let mut resp = match timeout(FIRST_BYTE, req.send()).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => return Err(http_err(&e)),
        Err(_) => return Err(serr("timeout", "no response")),
    };
    let rh: Headers = resp
        .headers()
        .iter()
        .filter(|(k, _)| !is_hop_by_hop(k.as_str()))
        .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
        .collect();
    let status = resp.status().as_u16().clamp(100, 599);
    ctx.out
        .ctl(Outgoing::Head {
            id: ctx.id,
            status,
            headers: &rh,
        })
        .await;
    loop {
        match timeout(HTTP_IDLE, resp.chunk()).await {
            Ok(Ok(Some(chunk))) => {
                for part in chunk.chunks(ctx.limits.max_frame.max(1)) {
                    ctx.send_data(0, part).await?;
                }
            }
            Ok(Ok(None)) => break,
            Ok(Err(e)) => return Err(http_err(&e)),
            Err(_) => return Err(serr("timeout", "idle")),
        }
    }
    ctx.out.ctl(Outgoing::End { id: ctx.id }).await;
    Ok(())
}

async fn ws_stream(mut ctx: Ctx, base: String, path: String, headers: Headers) -> Result<(), SErr> {
    let url = format!("ws{}{}", &base["http".len()..], path);
    let mut req = url
        .into_client_request()
        .map_err(|e| serr("protocol", e.to_string()))?;
    for (k, v) in forward_ws_headers(&headers) {
        if let (Ok(k), Ok(v)) = (
            HeaderName::from_bytes(k.as_bytes()),
            HeaderValue::from_str(&v),
        ) {
            req.headers_mut().insert(k, v);
        }
    }
    if let Ok(o) = HeaderValue::from_str(&base) {
        req.headers_mut().insert("origin", o);
    }
    let cfg = WebSocketConfig::default()
        .max_message_size(Some(ctx.limits.max_message))
        .max_frame_size(Some(ctx.limits.max_message));
    let ws = match timeout(
        LOCAL_CONNECT,
        connect_async_tls_with_config(req, Some(cfg), false, None),
    )
    .await
    {
        Ok(Ok((ws, _))) => ws,
        Ok(Err(WsError::Http(resp))) => {
            let none = Headers::new();
            ctx.out
                .ctl(Outgoing::Head {
                    id: ctx.id,
                    status: resp.status().as_u16().clamp(100, 599),
                    headers: &none,
                })
                .await;
            ctx.out.ctl(Outgoing::End { id: ctx.id }).await;
            return Ok(());
        }
        Ok(Err(WsError::Io(e))) => return Err(serr("unreachable", e.to_string())),
        Ok(Err(e)) => return Err(serr("refused", e.to_string())),
        Err(_) => return Err(serr("timeout", "connect")),
    };
    let none = Headers::new();
    ctx.out
        .ctl(Outgoing::Head {
            id: ctx.id,
            status: 101,
            headers: &none,
        })
        .await;
    let (mut lw, mut lr) = ws.split();
    let mut asm: Vec<u8> = Vec::new();
    let mut asm_text: Option<bool> = None;
    loop {
        tokio::select! {
            m = ctx.rx.recv() => match m {
                Some(In::Data { flags, payload }) => {
                    let n = payload.len();
                    let text = *asm_text.get_or_insert(flags & FLAG_TEXT != 0);
                    if asm.len() + n > ctx.limits.max_message {
                        return Err(serr("too_large", "message"));
                    }
                    asm.extend_from_slice(&payload);
                    if flags & FLAG_MORE == 0 {
                        let data = std::mem::take(&mut asm);
                        asm_text = None;
                        let msg = if text {
                            Message::text(String::from_utf8(data).map_err(|_| serr("protocol", "bad utf-8"))?)
                        } else {
                            Message::binary(data)
                        };
                        lw.send(msg).await.map_err(|e| serr("protocol", e.to_string()))?;
                    }
                    ctx.consumed(n).await;
                }
                Some(In::End) => {
                    let _ = lw.send(Message::Close(Some(CloseFrame { code: CloseCode::Normal, reason: "".into() }))).await;
                    break;
                }
                None => return Ok(()),
            },
            m = lr.next() => match m {
                Some(Ok(Message::Text(t))) => ctx.send_message(true, t.as_bytes()).await?,
                Some(Ok(Message::Binary(b))) => ctx.send_message(false, &b).await?,
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(serr("protocol", e.to_string())),
            },
        }
    }
    ctx.out.ctl(Outgoing::End { id: ctx.id }).await;
    Ok(())
}

fn fatal_bye(code: &str) -> bool {
    matches!(
        code,
        "revoked" | "expired" | "replaced" | "unsupported_version"
    )
}

/// Runs one connection until it ends. `up` is set once the handshake succeeded.
pub async fn run_once(p: &Params, shutdown: &mut watch::Receiver<bool>, up: &mut bool) -> Outcome {
    let mut req = match p.url.as_str().into_client_request() {
        Ok(r) => r,
        Err(e) => return Outcome::Fatal(format!("bad URL: {e}")),
    };
    let mut auth = match HeaderValue::from_str(&format!("Bearer {}", p.token)) {
        Ok(a) => a,
        Err(_) => return Outcome::Fatal("the token contains invalid characters".into()),
    };
    auth.set_sensitive(true);
    req.headers_mut().insert("authorization", auth);
    req.headers_mut().insert(
        "sec-websocket-protocol",
        HeaderValue::from_static("looty-tunnel.v1"),
    );
    let cfg = WebSocketConfig::default()
        .max_message_size(Some(HEADER_LEN + 65536 + 64))
        .max_frame_size(Some(HEADER_LEN + 65536 + 64));
    let connecting = connect_async_tls_with_config(req, Some(cfg), false, Some(tls()));
    let ws = tokio::select! {
        r = timeout(Duration::from_secs(20), connecting) => match r {
            Ok(Ok((ws, _))) => ws,
            Ok(Err(WsError::Http(resp))) => {
                let code = resp
                    .body()
                    .as_ref()
                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(b).ok())
                    .and_then(|v| v["error"].as_str().map(str::to_string))
                    .unwrap_or_default();
                let st = resp.status().as_u16();
                return if st == 401 {
                    Outcome::Fatal(format!("token refused ({code}); create a new tunnel token in Looty"))
                } else if st == 404 {
                    Outcome::Retry("HTTP 404: tunnels are disabled on this instance (or wrong --instance)".into())
                } else {
                    Outcome::Retry(format!("HTTP {st} {code}"))
                };
            }
            Ok(Err(e)) => return Outcome::Retry(format!("connection failed: {e}")),
            Err(_) => return Outcome::Retry("connection timed out".into()),
        },
        _ = shutdown.changed() => return Outcome::Shutdown,
    };
    let (mut sink, mut stream) = ws.split();
    let (wtx, mut wrx) = mpsc::channel::<Message>(64);
    let writer = tokio::spawn(async move {
        while let Some(m) = wrx.recv().await {
            let close = matches!(m, Message::Close(_));
            if sink.send(m).await.is_err() || close {
                break;
            }
        }
        let _ = sink.close().await;
    });
    let out = Out(wtx);
    let decls = p.services.decls();
    let agent = format!(
        "{AGENT} {}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    out.ctl(Outgoing::Hello {
        v: 1,
        agent: &agent,
        services: &decls,
    })
    .await;

    // Wait for welcome.
    let limits = loop {
        let m = tokio::select! {
            m = timeout(Duration::from_secs(10), stream.next()) => m,
            _ = shutdown.changed() => { writer.abort(); return Outcome::Shutdown; }
        };
        match m {
            Err(_) => {
                writer.abort();
                return Outcome::Retry("no welcome from the instance".into());
            }
            Ok(None) | Ok(Some(Err(_))) => {
                writer.abort();
                return Outcome::Retry("connection closed during handshake".into());
            }
            Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<Incoming>(&t) {
                Ok(Incoming::Welcome { v, limits, .. }) => {
                    if v != 1 {
                        writer.abort();
                        return Outcome::Fatal(format!("instance speaks protocol v{v}, this agent speaks v1: update looty-bridge"));
                    }
                    break limits;
                }
                Ok(Incoming::Bye { code, message }) => {
                    writer.abort();
                    let m = format!("refused by the instance: {code} {message}");
                    return if fatal_bye(&code) {
                        Outcome::Fatal(m)
                    } else {
                        Outcome::Retry(m)
                    };
                }
                _ => {
                    writer.abort();
                    return Outcome::Retry("unexpected message during handshake".into());
                }
            },
            Ok(Some(Ok(Message::Close(_)))) => {
                writer.abort();
                return Outcome::Retry("closed during handshake".into());
            }
            Ok(Some(Ok(_))) => {}
        }
    };
    *up = true;
    let limits = Arc::new(limits);
    log(format!(
        "connected; services: {}",
        if decls.is_empty() {
            "none".to_string()
        } else {
            decls
                .iter()
                .map(|d| format!("{} ({})", d.name, d.kind))
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));

    let client = reqwest::Client::builder()
        .connect_timeout(LOCAL_CONNECT)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .expect("http client");
    let streams: Streams = Arc::new(Mutex::new(HashMap::new()));
    let mut bye: Option<String> = None;

    let outcome = loop {
        let msg = tokio::select! {
            m = timeout(SILENCE, stream.next()) => m,
            _ = shutdown.changed() => {
                let _ = out.0.send(Message::Close(Some(CloseFrame { code: CloseCode::Normal, reason: "bye".into() }))).await;
                break Outcome::Shutdown;
            }
        };
        let msg = match msg {
            Err(_) => break Outcome::Retry("no news from the instance for 45 s".into()),
            Ok(None) => break Outcome::Retry("connection closed".into()),
            Ok(Some(Err(e))) => break Outcome::Retry(format!("connection error: {e}")),
            Ok(Some(Ok(m))) => m,
        };
        match msg {
            Message::Text(t) => {
                let inc = match serde_json::from_str::<Incoming>(&t) {
                    Ok(i) => i,
                    Err(_) => continue, // unknown control message: ignored
                };
                match inc {
                    Incoming::Ping { n } => {
                        out.ctl(Outgoing::Pong { n }).await;
                    }
                    Incoming::Pong { .. } | Incoming::Welcome { .. } => {}
                    Incoming::Bye { code, message } => {
                        log(format!("instance says bye: {code} {message}"));
                        if fatal_bye(&code) {
                            break Outcome::Fatal(format!("tunnel ended by the instance: {code}"));
                        }
                        bye = Some(code);
                    }
                    Incoming::Open {
                        id,
                        service,
                        proto,
                        method,
                        path,
                        headers,
                    } => {
                        open_stream(
                            &streams, &out, &limits, &client, p, id, service, proto, method, path,
                            headers,
                        )
                        .await;
                    }
                    Incoming::End { id } => {
                        if let Some(e) = streams.lock().unwrap().get(&id) {
                            let _ = e.tx.send(In::End);
                        }
                    }
                    Incoming::Error { id, code, message } => {
                        if let Some(e) = streams.lock().unwrap().remove(&id) {
                            e.task.abort();
                            log(format!(
                                "stream {id} cancelled by the instance: {code} {message}"
                            ));
                        }
                    }
                    Incoming::Window { id, add } => {
                        let mut violation = false;
                        if let Some(e) = streams.lock().unwrap().get(&id) {
                            if add == 0
                                || e.credit.available_permits() as u64 + add > limits.initial_window
                            {
                                violation = true;
                            } else {
                                e.credit.add_permits(add as usize);
                            }
                        }
                        if violation {
                            if let Some(e) = streams.lock().unwrap().remove(&id) {
                                e.task.abort();
                            }
                            out.ctl(Outgoing::Error {
                                id,
                                code: "flow_control",
                                message: "bad window",
                            })
                            .await;
                        }
                    }
                }
            }
            Message::Binary(b) => match decode_frame(&b, limits.max_frame) {
                Ok((id, flags, payload)) => {
                    if let Some(e) = streams.lock().unwrap().get(&id) {
                        let _ = e.tx.send(In::Data {
                            flags,
                            payload: payload.to_vec(),
                        });
                    }
                }
                Err(_) => break Outcome::Retry("invalid data frame from the instance".into()),
            },
            Message::Close(f) => {
                let code = f.as_ref().map(|f| u16::from(f.code)).unwrap_or(1005);
                break match code {
                    4001 | 4002 | 4009 | 4426 => Outcome::Fatal(format!(
                        "tunnel closed by the instance ({})",
                        bye.as_deref().unwrap_or(match code {
                            4001 => "revoked",
                            4002 => "expired",
                            4009 => "replaced: another agent uses this token",
                            _ => "unsupported_version",
                        })
                    )),
                    _ => Outcome::Retry(format!("closed by the instance (code {code})")),
                };
            }
            _ => {}
        }
    };
    // Everything local dies with the connection.
    for (_, e) in streams.lock().unwrap().drain() {
        e.task.abort();
    }
    drop(out);
    if matches!(outcome, Outcome::Shutdown) {
        let _ = timeout(Duration::from_secs(2), writer).await;
    } else {
        writer.abort();
    }
    outcome
}

#[allow(clippy::too_many_arguments)]
async fn open_stream(
    streams: &Streams,
    out: &Out,
    limits: &Arc<Limits>,
    client: &reqwest::Client,
    p: &Params,
    id: u32,
    service: String,
    proto: String,
    method: String,
    path: String,
    headers: Headers,
) {
    // Whitelist: the local address comes only from the operator's configuration.
    let Some(base) = p.services.base(&service).map(str::to_string) else {
        out.ctl(Outgoing::Error {
            id,
            code: "unknown_service",
            message: "service not served",
        })
        .await;
        return;
    };
    if !valid_path(&path) || (proto != "http" && proto != "ws") {
        out.ctl(Outgoing::Error {
            id,
            code: "protocol",
            message: "bad open",
        })
        .await;
        return;
    }
    let (tx, rx) = mpsc::unbounded_channel();
    let credit = Arc::new(Semaphore::new(limits.initial_window as usize));
    let ctx = Ctx {
        id,
        out: out.clone(),
        limits: limits.clone(),
        credit: credit.clone(),
        rx,
    };
    let full = {
        let map = streams.lock().unwrap();
        map.len() >= limits.max_streams || map.contains_key(&id)
    };
    if full {
        out.ctl(Outgoing::Error {
            id,
            code: "refused",
            message: "too many streams",
        })
        .await;
        return;
    }
    let is_http = proto == "http";
    let client = client.clone();
    let (out2, streams2) = (out.clone(), streams.clone());
    let shown = format!("{service} {method} {path}");
    let task = tokio::spawn(async move {
        let res = if is_http {
            http_stream(ctx, client, base, method, path, headers).await
        } else {
            ws_stream(ctx, base, path, headers).await
        };
        if let Err(e) = res {
            log(format!(
                "stream {id} ({}): {} {}",
                shown.split(' ').next().unwrap_or(""),
                e.code,
                e.msg
            ));
            out2.ctl(Outgoing::Error {
                id,
                code: e.code,
                message: &e.msg.chars().take(200).collect::<String>(),
            })
            .await;
        }
        streams2.lock().unwrap().remove(&id);
    });
    streams.lock().unwrap().insert(
        id,
        Entry {
            tx,
            credit,
            task: task.abort_handle(),
        },
    );
}

/// Connect, serve, reconnect with backoff until Ctrl+C (`srx`) or a fatal refusal.
/// Returns the process exit code.
pub async fn run_forever(params: &Params, mut srx: watch::Receiver<bool>) -> i32 {
    let mut attempt = 0u32;
    loop {
        let started = Instant::now();
        let mut up = false;
        let outcome = run_once(params, &mut srx, &mut up).await;
        if up && started.elapsed() > Duration::from_secs(60) {
            attempt = 0;
        }
        match outcome {
            Outcome::Shutdown => {
                log("disconnected, bye");
                return 0;
            }
            Outcome::Fatal(why) => {
                log(format!("stopped: {why}"));
                return 1;
            }
            Outcome::Retry(why) => {
                let d = backoff(attempt, jitter());
                attempt += 1;
                log(format!(
                    "disconnected: {why}; retrying in {:.0} s",
                    d.as_secs_f64()
                ));
                tokio::select! {
                    _ = tokio::time::sleep(d) => {}
                    _ = srx.changed() => { log("bye"); return 0; }
                }
            }
        }
    }
}
