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

## NixOS

There is no NixOS module for the server. Run the compose files with Docker enabled on NixOS (see [requirements](/docs/host/requirements/)). The *desktop app* is packaged as a flake.
