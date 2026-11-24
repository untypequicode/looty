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

By default the labels set no entry point or certificate: Traefik must reach the stack's network and serve HTTPS on its entry points, for example `--entrypoints.websecure.http.tls.certResolver=<resolver>`. `APP_NAME` and `APP_DOMAIN` in `.env` name the routers and the host rule.

To have the labels carry the names of your Traefik, fill in the **Your Traefik** section of the [configuration generator](/install/) (or pass `--traefik-network`, `--traefik-entrypoint` and `--traefik-resolver` to `install.sh`). The generated files then contain `TRAEFIK_ENTRYPOINT` (router entry point), `TRAEFIK_CERT_RESOLVER` (certificate resolver) and `TRAEFIK_NETWORK` (existing external Docker network, which the services join and the `traefik.docker.network` label names). See the [environment reference](/docs/reference/environment/).

### Several Looty stacks on one Traefik network

If Traefik reaches your stacks through a shared external network (for example `traefik-public`) and you attach the services of two Looty stacks to it, Docker answers the bare name `backend` with the addresses of every stack. A service of one stack can then reach the backend of the other and get 401 errors (the editor fails to connect, for instance). The compose files avoid this: each stack reaches its own services by names that include `APP_NAME`, `${APP_NAME}-backend` and `${APP_NAME}-collaboration`, which are set as network aliases and used by `BACKEND_URL` and `COLLAB_URL`.

- Give every stack its own `APP_NAME` in `.env` (for example `looty` and `looty-demo`); the demo compose file uses `looty-demo`.
- Attach to the shared network only the services Traefik must reach (usually all three), and never declare a generic alias such as `backend` on it.
- Keep `postgres` off the shared network.

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
