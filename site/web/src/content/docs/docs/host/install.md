---
title: "Detailed install"
description: "Docker Compose with Caddy, with Traefik, behind another proxy, and on NixOS."
---

The compose files are rendered from `deploy/templates/` by `deploy/render.sh`; edit the templates, not the rendered files. Pin a version with `LOOTY_VERSION=<x.y.z>` in `.env`.

## With Caddy (no existing proxy)

Follow the [quick install](/docs/host/quick-install/). The `Caddyfile` routes everything and gets HTTPS certificates.

## With Traefik

Take only the compose file; routing is in its labels:

```sh
curl -LO https://github.com/untypequicode/looty/releases/latest/download/docker-compose.yml
curl -o .env https://raw.githubusercontent.com/untypequicode/looty/main/.env.example
```

The labels set no entry point or certificate: Traefik must reach the stack's network and serve HTTPS on its entry points, for example `--entrypoints.websecure.http.tls.certResolver=<resolver>`. `APP_NAME` and `APP_DOMAIN` in `.env` name the routers and the host rule.

## Behind another reverse proxy

Use the Traefik compose file and route, with WebSockets enabled:

| Path | Target |
|---|---|
| `/api`, `/uploads`, `/pages`, `/dav`, `/.well-known/caldav` | backend, port 8080 |
| `/collab` | collaboration, port 8082, **prefix stripped** |
| everything else | frontend, port 3000 |

The `Caddyfile` is a working example. Forward the `Host` and `X-Forwarded-*` headers.

### Timeouts and long-lived connections

Live editing (`/collab`) and notifications (`/api/ws`) are WebSockets that stay open for hours. The collaboration server sends a ping every 15 seconds, but a proxy that cuts connections on its own timer still produces periodic disconnects (typically every minute or two) that leave pages read-only until they reconnect. Raise these limits:

- **Traefik**: set the entry point timeouts, for example `--entrypoints.websecure.transport.respondingTimeouts.readTimeout=0` and `--entrypoints.websecure.transport.respondingTimeouts.idleTimeout=300s` (a 60 s read timeout and a 120 s idle timeout cut idle WebSockets).
- **nginx**: `proxy_read_timeout 3600s;` and `proxy_send_timeout 3600s;` on the `/collab` and `/api/ws` locations, with `proxy_http_version 1.1` and the `Upgrade` / `Connection` headers.
- **Cloudflare**: a proxied connection with no traffic for 100 seconds is closed. The 15-second ping keeps it alive, but do not put the WebSocket paths behind a rule that buffers or rewrites them.

## NixOS

The flake provides a NixOS module, `services.looty`: see [NixOS](/docs/host/nixos/).
