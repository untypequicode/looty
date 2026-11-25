//! End to end against a fake relay (this test is the instance) and fake local services.
#![allow(
    clippy::result_large_err,
    clippy::single_match,
    clippy::field_reassign_with_default
)]

use futures_util::{SinkExt, StreamExt};
use looty_bridge::config::Services;
use looty_bridge::proto::*;
use looty_bridge::session::{run_forever, Params};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, accept_hdr_async};

const T: Duration = Duration::from_secs(10);

/// Local HTTP service: answers every request with 200 and a body repeating the request line.
async fn fake_http() -> String {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = s.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = req.lines().next().unwrap_or("").to_string();
                let body = format!("{line}|{}", req.to_lowercase().contains("x-custom: yes"));
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                s.write_all(resp.as_bytes()).await.unwrap();
            });
        }
    });
    format!("http://{addr}")
}

/// Local WebSocket service: echoes messages.
async fn fake_ws_echo() -> String {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut ws = accept_async(s).await.unwrap();
                while let Some(Ok(m)) = ws.next().await {
                    if m.is_text() || m.is_binary() {
                        ws.send(m).await.unwrap();
                    }
                }
            });
        }
    });
    format!("http://{addr}")
}

fn params(relay: &str, specs: &[String]) -> Params {
    Params {
        url: format!("ws://{relay}/api/tunnels/agent"),
        token: "lty_tun_secret".into(),
        services: Services::parse(specs).unwrap(),
    }
}

async fn ctl<S>(ws: &mut S) -> Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match timeout(T, ws.next()).await.unwrap().unwrap().unwrap() {
            Message::Text(t) => return serde_json::from_str(&t).unwrap(),
            _ => continue,
        }
    }
}

#[tokio::test]
async fn serves_http_and_ws_streams_through_the_whitelist() {
    let http = fake_http().await;
    let wsl = fake_ws_echo().await;
    let relay = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let raddr = relay.local_addr().unwrap().to_string();
    let p = params(
        &raddr,
        &[format!("jupyter={http}"), format!("kern:jupyter={wsl}")],
    );
    let (stx, srx) = watch::channel(false);
    let agent = tokio::spawn(async move { run_forever(&p, srx).await });

    let (sock, _) = relay.accept().await.unwrap();
    let mut seen_auth = String::new();
    let mut ws = accept_hdr_async(sock, |req: &Request, mut resp: Response| {
        seen_auth = req.headers()["authorization"].to_str().unwrap().to_string();
        assert_eq!(req.headers()["sec-websocket-protocol"], "looty-tunnel.v1");
        resp.headers_mut()
            .insert("sec-websocket-protocol", "looty-tunnel.v1".parse().unwrap());
        Ok(resp)
    })
    .await
    .unwrap();
    assert_eq!(seen_auth, "Bearer lty_tun_secret");

    let hello = ctl(&mut ws).await;
    assert_eq!(hello["t"], "hello");
    assert_eq!(hello["v"], 1);
    assert_eq!(hello["services"].as_array().unwrap().len(), 2);
    ws.send(Message::text(
        json!({"t":"welcome","v":1,"ping_interval":15,"limits":{}}).to_string(),
    ))
    .await
    .unwrap();

    // Whitelist: unknown service and bad path are refused, nothing is contacted.
    ws.send(Message::text(json!({"t":"open","id":1,"service":"127.0.0.1:1","proto":"http","method":"GET","path":"/x","headers":[]}).to_string())).await.unwrap();
    let e = ctl(&mut ws).await;
    assert_eq!(
        (e["t"].as_str(), e["id"].as_u64(), e["code"].as_str()),
        (Some("error"), Some(1), Some("unknown_service"))
    );
    ws.send(Message::text(json!({"t":"open","id":2,"service":"jupyter","proto":"http","method":"GET","path":"//evil","headers":[]}).to_string())).await.unwrap();
    assert_eq!(ctl(&mut ws).await["t"], "error");

    // Ping.
    ws.send(Message::text(json!({"t":"ping","n":42}).to_string()))
        .await
        .unwrap();
    let pong = ctl(&mut ws).await;
    assert_eq!(
        (pong["t"].as_str(), pong["n"].as_u64()),
        (Some("pong"), Some(42))
    );

    // HTTP stream.
    ws.send(Message::text(json!({"t":"open","id":3,"service":"jupyter","proto":"http","method":"GET","path":"/api/kernels?x=1","headers":[["x-custom","yes"],["connection","close"]]}).to_string())).await.unwrap();
    ws.send(Message::text(json!({"t":"end","id":3}).to_string()))
        .await
        .unwrap();
    let head = ctl(&mut ws).await;
    assert_eq!(
        (head["t"].as_str(), head["status"].as_u64()),
        (Some("head"), Some(200))
    );
    let mut body = Vec::new();
    loop {
        match timeout(T, ws.next()).await.unwrap().unwrap().unwrap() {
            Message::Binary(b) => {
                let (id, fl, p) = decode_frame(&b, 65536).unwrap();
                assert_eq!((id, fl), (3, 0));
                body.extend_from_slice(p);
            }
            Message::Text(t) => {
                let v: Value = serde_json::from_str(&t).unwrap();
                if v["t"] == "end" {
                    break;
                }
                assert_ne!(v["t"], "error", "{v}");
            }
            _ => {}
        }
    }
    assert_eq!(
        String::from_utf8(body).unwrap(),
        "GET /api/kernels?x=1 HTTP/1.1|true"
    );

    // WebSocket stream: text message and a binary message larger than a frame.
    ws.send(Message::text(json!({"t":"open","id":5,"service":"kern","proto":"ws","method":"GET","path":"/ws","headers":[]}).to_string())).await.unwrap();
    let head = ctl(&mut ws).await;
    assert_eq!(
        (head["t"].as_str(), head["status"].as_u64()),
        (Some("head"), Some(101))
    );
    ws.send(Message::binary(encode_frame(5, FLAG_TEXT, b"hello")))
        .await
        .unwrap();
    let mut got = Vec::new();
    while got.len() < 5 {
        match timeout(T, ws.next()).await.unwrap().unwrap().unwrap() {
            Message::Binary(b) => {
                let (id, fl, p) = decode_frame(&b, 65536).unwrap();
                assert_eq!((id, fl), (5, FLAG_TEXT));
                got.extend_from_slice(p);
            }
            _ => {}
        }
    }
    assert_eq!(got, b"hello");
    // 100 000 bytes in two frames (first with MORE): echoed back split by the agent.
    let big: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    ws.send(Message::binary(encode_frame(5, FLAG_MORE, &big[..60_000])))
        .await
        .unwrap();
    ws.send(Message::binary(encode_frame(5, 0, &big[60_000..])))
        .await
        .unwrap();
    let mut back = Vec::new();
    let mut last_flags = FLAG_MORE;
    let mut consumed = 0usize;
    while last_flags & FLAG_MORE != 0 {
        match timeout(T, ws.next()).await.unwrap().unwrap().unwrap() {
            Message::Binary(b) => {
                let (_, fl, p) = decode_frame(&b, 65536).unwrap();
                last_flags = fl;
                back.extend_from_slice(p);
                consumed += p.len();
                // Give credit back like the instance does.
                ws.send(Message::text(
                    json!({"t":"window","id":5,"add":p.len().max(1)}).to_string(),
                ))
                .await
                .unwrap();
            }
            Message::Text(_) => {}
            _ => {}
        }
    }
    assert_eq!(consumed, 100_000);
    assert_eq!(back, big);
    ws.send(Message::text(json!({"t":"end","id":5}).to_string()))
        .await
        .unwrap();
    loop {
        let v = ctl(&mut ws).await;
        if v["t"] == "end" {
            break;
        }
    }

    // Ctrl+C: clean close, exit code 0.
    stx.send(true).unwrap();
    assert_eq!(timeout(T, agent).await.unwrap().unwrap(), 0);
}

#[tokio::test]
async fn reconnects_after_a_drop() {
    let relay = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let raddr = relay.local_addr().unwrap().to_string();
    let p = params(&raddr, &["jupyter=http://127.0.0.1:9".to_string()]);
    let (stx, srx) = watch::channel(false);
    let agent = tokio::spawn(async move { run_forever(&p, srx).await });

    // First connection: handshake, then the relay drops the socket.
    let (sock, _) = relay.accept().await.unwrap();
    let mut ws = accept_agent(sock).await;
    assert_eq!(ctl(&mut ws).await["t"], "hello");
    ws.send(Message::text(json!({"t":"welcome","v":1}).to_string()))
        .await
        .unwrap();
    drop(ws);

    // The agent comes back (backoff of about one second) and says hello again.
    let (sock, _) = timeout(Duration::from_secs(8), relay.accept())
        .await
        .unwrap()
        .unwrap();
    let mut ws = accept_agent(sock).await;
    assert_eq!(ctl(&mut ws).await["t"], "hello");
    ws.send(Message::text(json!({"t":"welcome","v":1}).to_string()))
        .await
        .unwrap();
    stx.send(true).unwrap();
    assert_eq!(timeout(T, agent).await.unwrap().unwrap(), 0);
}

#[tokio::test]
async fn revoked_token_is_not_retried() {
    let relay = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let raddr = relay.local_addr().unwrap().to_string();
    let p = params(&raddr, &[]);
    let (_stx, srx) = watch::channel(false);
    let agent = tokio::spawn(async move { run_forever(&p, srx).await });
    let (sock, _) = relay.accept().await.unwrap();
    let _ = accept_hdr_async(sock, |_: &Request, _: Response| {
        let mut r = ErrorResponse::new(Some(r#"{"error":"tunnel_token_revoked"}"#.to_string()));
        *r.status_mut() = 401.try_into().unwrap();
        Err(r)
    })
    .await;
    assert_eq!(timeout(T, agent).await.unwrap().unwrap(), 1);
}

async fn accept_agent(
    sock: tokio::net::TcpStream,
) -> tokio_tungstenite::WebSocketStream<tokio::net::TcpStream> {
    accept_hdr_async(sock, |_: &Request, mut resp: Response| {
        resp.headers_mut()
            .insert("sec-websocket-protocol", "looty-tunnel.v1".parse().unwrap());
        Ok(resp)
    })
    .await
    .unwrap()
}

/// A `latex` service reached through the bridge: the instance posts a project zip to /compile and gets the PDF back,
/// while a service that was not declared stays unreachable.
#[cfg(unix)]
#[tokio::test]
async fn latex_compile_through_the_bridge() {
    use looty_bridge::latex_service::{router, Config};
    use std::io::{Cursor, Read, Write};
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("looty-bridge-latex-it-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("work")).unwrap();
    let engine = dir.join("pdflatex");
    std::fs::write(&engine, "#!/bin/sh\nfor last; do :; done\nstem=${last%.tex}\necho '%PDF-fake' > \"$stem.pdf\"\necho \"compiled $last\" > \"$stem.log\"\n").unwrap();
    std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut cfg = Config::default();
    cfg.bin_dir = Some(dir.clone());
    cfg.use_path = false;
    cfg.work_dir = dir.join("work");
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let laddr = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, router(cfg)).await.unwrap() });

    let relay = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let raddr = relay.local_addr().unwrap().to_string();
    let p = params(&raddr, &[format!("tex:latex=http://{laddr}")]);
    let (stx, srx) = watch::channel(false);
    let agent = tokio::spawn(async move { run_forever(&p, srx).await });
    let (sock, _) = relay.accept().await.unwrap();
    let mut ws = accept_agent(sock).await;
    let hello = ctl(&mut ws).await;
    assert_eq!(hello["services"][0], json!({"name":"tex","kind":"latex"}));
    ws.send(Message::text(
        json!({"t":"welcome","v":1,"ping_interval":15,"limits":{}}).to_string(),
    ))
    .await
    .unwrap();

    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    w.start_file("main.tex", zip::write::SimpleFileOptions::default())
        .unwrap();
    w.write_all(b"\\documentclass{article}").unwrap();
    let body = w.finish().unwrap().into_inner();

    ws.send(Message::text(json!({"t":"open","id":1,"service":"tex","proto":"http","method":"POST","path":"/compile?main=main.tex&engine=pdflatex","headers":[["content-type","application/zip"]]}).to_string())).await.unwrap();
    ws.send(Message::binary(encode_frame(1, 0, &body)))
        .await
        .unwrap();
    ws.send(Message::text(json!({"t":"end","id":1}).to_string()))
        .await
        .unwrap();
    let head = loop {
        let v = ctl(&mut ws).await;
        if v["t"] != "window" {
            break v;
        }
    };
    assert_eq!(
        (head["t"].as_str(), head["status"].as_u64()),
        (Some("head"), Some(200)),
        "{head}"
    );
    let mut resp = Vec::new();
    loop {
        match timeout(T, ws.next()).await.unwrap().unwrap().unwrap() {
            Message::Binary(b) => {
                let (_, _, p) = decode_frame(&b, 65536).unwrap();
                resp.extend_from_slice(p);
                ws.send(Message::text(
                    json!({"t":"window","id":1,"add":p.len().max(1)}).to_string(),
                ))
                .await
                .unwrap();
            }
            Message::Text(t) => {
                let v: Value = serde_json::from_str(&t).unwrap();
                if v["t"] == "end" {
                    break;
                }
                assert_ne!(v["t"], "error", "{v}");
            }
            _ => {}
        }
    }
    let mut z = zip::ZipArchive::new(Cursor::new(resp)).unwrap();
    let mut pdf = Vec::new();
    z.by_name("output.pdf")
        .unwrap()
        .read_to_end(&mut pdf)
        .unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let mut log = String::new();
    z.by_name("compile.log")
        .unwrap()
        .read_to_string(&mut log)
        .unwrap();
    assert!(log.contains("compiled main.tex"));

    // Not declared: refused by the agent.
    ws.send(Message::text(json!({"t":"open","id":3,"service":"other","proto":"http","method":"POST","path":"/compile","headers":[]}).to_string())).await.unwrap();
    assert_eq!(ctl(&mut ws).await["code"], "unknown_service");

    stx.send(true).unwrap();
    assert_eq!(timeout(T, agent).await.unwrap().unwrap(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}
