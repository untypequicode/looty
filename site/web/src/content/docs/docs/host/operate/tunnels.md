---
title: "PC tunnels"
description: "Let people use their own computer as a Jupyter server with an outgoing connection: LOOTY_TUNNELS, administration settings, reverse proxy and WebSocket timeouts, audit and security."
---

A **PC tunnel** lets a person connect their own computer to a project, so that the instance uses it as a Jupyter server (Python, R...) for as long as their agent (`looty-bridge`) is running. The agent opens one outgoing WebSocket to your instance: **no inbound port** is needed on the PC, and nothing changes on your server's network. For the people's side, see [Use my PC as a Jupyter or R server](/docs/use/features/pc-tunnel/).

:::caution
**Members of a project can run code on the PC connected to it**, with the rights of the user running Jupyter there. The feature is **off by default**. Turn it on only if your people understand this.
:::

## Turn it on

- Set `LOOTY_TUNNELS=on` in the backend environment (see the [environment reference](/docs/reference/environment/)). It only gives the **initial value** of the switch.
- After the first save, **Administration > Instance > Tunnels** is the source of truth: the switch, then the limits below. Turning it off, and saving, closes every tunnel in progress immediately; people's projects keep their default engine (Pyodide, WebR).
- Not available in the public demo.

## What the instance can reach

Only the **services the agent declares** (`jupyter`, `latex`, `http`), at an address configured **on the PC**, never chosen by the instance. The instance cannot reach any other port of the PC or of its network, and cannot do anything when the agent is not connected. Your server does not contact private networks for tunnels, so `JUPYTER_ALLOW_PRIVATE` is irrelevant to them. The browser never talks to the PC: only the Looty server does, as with any project Jupyter server.

## Administration settings

Administration > Instance > Tunnels (defaults in brackets):

- **Enabled** (off).
- **Active tokens per person** (20).
- **Maximum token lifetime** (365 days; a token is created with 30 days by default).
- **Simultaneous streams per tunnel** (16): requests and kernel connections in parallel.
- **Bandwidth per tunnel** (unlimited), both directions.
- **Allowed kinds of service**: `jupyter`, `latex`, `http`.
- **Audit retention** (90 days).

Changes are applied to tunnels already open within **15 seconds** (the instance checks every connection against the database and these settings at each ping): a revoked, expired, deleted or suspended-account token, a kind that is no longer allowed or a lifetime above the new maximum closes the connection. Refusals use explicit codes the interface translates (disabled, too many tokens, kind not allowed, lifetime too long, owner only).

## Security model

- **Tokens**: `lty_tun_...`, bound to one person and one space, stored hashed, shown once at creation, revocable by their owner or by an administrator. They only authenticate the tunnel connection: they cannot call the rest of the API, and personal access tokens cannot manage tunnels.
- **Confirmation**: creating a tunnel requires the person to confirm that members can run code on their PC.
- **Owner only**: a tunnel can be limited to its owner, changeable at any time.
- **Bounded relay**: 16 streams, 64 KiB frames, 16 KiB headers, 8 MiB messages, 16 MiB request and 64 MiB response bodies, timeouts, one connection per token.
- **Audit**: Administration > Instance lists created, attached, connected, disconnected (with duration and bytes), revoked tunnels, refused kinds, owner-only changes and settings changes, kept for the retention period. Tokens never appear in it; refusals of unknown tokens at connection time are in the server logs, not in the audit.
- Single backend process: the tunnel state is in memory. A restart disconnects the agents, which reconnect on their own with a growing delay.

## Reverse proxy

Agents connect to `wss://<your instance>/api/tunnels/agent` (a WebSocket upgrade, authenticated by the tunnel token in the `Authorization` header). The default Caddy and Traefik files of this guide already forward WebSockets. As for live editing, a proxy that cuts idle connections must be raised: the instance pings every **15 seconds** and closes a silent tunnel after 45 seconds, so a proxy read timeout under about a minute is normally fine, but a shorter one produces periodic disconnects.

- **Traefik**: entry point timeouts as in [Timeouts and long-lived connections](/docs/host/install/#timeouts-and-long-lived-connections) (`readTimeout=0`, `idleTimeout=300s`).
- **nginx**: `proxy_http_version 1.1;`, the `Upgrade` and `Connection` headers, and `proxy_read_timeout 3600s; proxy_send_timeout 3600s;` on `/api/tunnels/agent`. No request body size limit applies to the tunnel itself.
- **Cloudflare**: a proxied WebSocket idle for 100 seconds is closed; the 15 second ping keeps it alive. Do not buffer or rewrite the path.

The agent also needs to reach your instance over HTTPS from the person's network; nothing else on your side.

## Not related to calls

The TURN relay of [voice calls](/docs/use/features/voice-calls/) is a separate thing: tunnels use no UDP, no TURN and no extra port, only the existing HTTPS endpoint.

## Diagnose

- An agent that gets `404` on connection: tunnels are off (switch or `LOOTY_TUNNELS`).
- `401`: unknown, revoked, expired token or suspended account; the agent stops and the person creates a new tunnel.
- Repeated disconnections every minute or two: proxy timeouts (above).
- Status (connected, latency, agent version, declared services, bytes) is in the project dialog; the audit and the server logs give the history.

Design and protocol: `docs/adr/0020-pc-tunnel.md` and `docs/tunnel-protocol.md` in the repository.
