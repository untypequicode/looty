# PC tunnel: an outgoing agent lets the instance use your own computer

Issues #271 (this ADR), #272 (relay), under #278. Goal: a person uses **their own PC** as the Jupyter, R or LaTeX server of a project, **for as long as the tunnel is up**, without opening a port or touching the router. Remote Jupyter already works through the backend (ADR 0003), but only when the Looty server can reach the Jupyter address, which a home or office PC behind NAT never allows.

## Decision

- A small agent, **`looty-bridge`**, runs on the PC and opens **one outgoing WebSocket over HTTPS** to the instance (`wss://<instance>/api/tunnels/agent`). Nothing is opened on the box or the firewall; it works behind NAT and through HTTP proxies that allow WebSocket.
- The instance **multiplexes** over this connection the requests it would otherwise send to a server: HTTP and the kernel WebSocket of Jupyter, a LaTeX compile service, later R or any HTTP service. Only the Looty server talks to the tunnel; the browser never sees the PC, exactly like Jupyter today (ADR 0003).
- The agent **declares services** (`name`, `kind`: `jupyter`, `latex`, `http`). The instance can only open streams to a declared name of the right kind (**whitelist**); the local address of a service is configured on the PC and never chosen by the instance, so there is no way to reach an arbitrary port of the PC or of its network.
- The tunnel lives **as long as the agent is connected**. On disconnection every stream fails with a clear code (`jupyter_tunnel_offline`) and projects keep their default engine (Pyodide, WebR): nothing is silently rerouted and nothing is lost.
- Cost: one WebSocket per tunnel and a byte relay with no parsing of the payload. Idle tunnels cost a ping every 15 s.
- **Agent language: Rust**, one static binary (`tokio`, `tokio-tungstenite`, `reqwest`), also shipped as a Nix package and a container image. Same toolchain as the backend and the desktop app, no runtime to install on the PC. The protocol is specified in `docs/tunnel-protocol.md` so the agent can be written independently of the relay.
- **Multiplexing protocol**: our own, small, over WebSocket frames (JSON control messages in text frames, data in binary frames with a 5-byte header, per-stream credit windows, ping/pong). See "Why not something else" and `docs/tunnel-protocol.md`.
- **Off by default**: `LOOTY_TUNNELS=on` turns the feature on (an admin setting and the security review come with #276).

## Tokens

A **tunnel token** (`lty_tun_` + 32 random bytes, base64url) is bound to **one person** and **one space** (optionally one project), expires (30 days by default, 365 at most), can be revoked, is stored **hashed** (SHA-256, the token is high-entropy) and is **shown once**, at creation. It authenticates the WebSocket upgrade (`Authorization: Bearer`), never appears in a URL, and only works on `/api/tunnels/agent`: it cannot call the rest of the API, and personal access tokens (`looty_`) cannot manage tunnels. Revoking closes the live connection immediately; expiry closes it at the next ping; a suspended account is refused. A new connection with the same token replaces the previous one (close code 4009).

## Threat model

| Who | Can | Cannot |
| --- | --- | --- |
| **Instance (or its admin)** | Send requests to the services the agent declared, as long as it is connected; run code in the kernels of a declared Jupyter, with that Jupyter's rights on the PC. | Reach anything not declared; connect to the PC when the agent is off; see the Jupyter token if the person did not give it (the token for the PC's Jupyter, when used, is stored encrypted like ADR 0003). |
| **Project member** | Use the tunnel attached to the project according to their project rights (editors run code through it, readers see its status); each person gets their own kernel. | Attach someone else's tunnel, see the tunnel token, reach services other than the one chosen for the project, read the PC's files except through the notebook kernel they are allowed to run. |
| **Tunnel owner** | Create, attach (to projects they can edit), detach and revoke; choose which services exist and where they point. | Give the tunnel to a space they are not in. |
| **Attacker with a stolen token** | Impersonate the agent: answer the instance's requests with forged data (for example fake notebook outputs) until it is revoked or expires. | Read anything from the instance, call its API, or open streams: the instance is the only side that opens streams. |
| **Attacker on the PC side** | Whatever the local Jupyter allows: a Jupyter server **is** remote code execution. The agent should bind services to loopback and keep Jupyter's own token on. | Pivot through the agent: it only connects to the addresses in its own configuration. |

Important consequence: **connecting a Jupyter to a project gives the project's editors code execution on that PC**, with the PC user's rights. The interface must say so, and the agent runs unprivileged (ideally in a container or a dedicated user). Mitigations in the relay: whitelist by declared service and kind, path validation, bounded streams (16), frames (64 KiB), headers (16 KiB), messages (8 MiB), bodies (16 MiB up, 64 MiB down), per-stream credit windows (256 KiB), timeouts (30 s to first byte, 60 s idle on HTTP, 30 min idle on WebSocket), ping every 15 s and closing after 45 s of silence, one connection per token, logging of connect, disconnect, refusals and protocol violations (never the token), and the byte counters in the status.

## Why not something else

- **Reverse SSH** (`ssh -R`): needs an SSH server on the instance side, a system account or a restricted shell, key management outside Looty, and gives a raw TCP port the instance would have to trust; rights, whitelist, per-person tokens and live status would all be rebuilt around it. Not available on Windows without extra installs.
- **WireGuard / Tailscale-style VPN**: puts the PC on a network with the server (or its host), which is far more access than needed; needs root/kernel support on both sides, UDP, and an admin for every user. Wrong blast radius.
- **Third-party tunnels (Cloudflare Tunnel, ngrok, bore)**: exposes the Jupyter to the **public internet** (or to a third-party account and its terms) and sends the code and data through a company the instance owner did not choose. It breaks the "self-hosted, nothing leaves" promise and still needs the PC's Jupyter to defend itself.
- **Existing multiplexers (yamux, HTTP/2 over the WebSocket)**: fine technically, but they carry arbitrary streams without the notion of service, whitelist and per-stream limits we need, and an independent agent would have to bring the same library. Our protocol is small enough (about ten message types) to specify and to test with a fake agent; credit-based flow control is borrowed from HTTP/2.

## Consequences

- The relay lives in the backend (`tunnel.rs`: tokens, endpoint, status; `tunnel_relay.rs`: sessions and streams; `jupyter.rs`: a project can target a tunnel). It is in-memory: a backend restart disconnects agents, which reconnect (the agent must retry with backoff).
- A project's Jupyter configuration stores `tunnel_id` and `tunnel_service` instead of a URL. `JUPYTER_ALLOW_PRIVATE` is irrelevant for tunnels: no private network is contacted by the instance.
- A single backend process is assumed (as for kernels in ADR 0003); several replicas would need sticky routing of the agent connection.
- Follow-ups under #278: the agent (#273), the project interface (#274), the LaTeX service (#275), security review and admin switch (#276), docs (#277).

## Security and administration (#276, web 0.58.0)

What was done to bound the risks above:

- **Instance settings** (Administration > Instance > Tunnels, in `global_settings`; `LOOTY_TUNNELS` is only the initial value of the switch): on/off, active tokens per person (20), maximum token lifetime (365 days), simultaneous streams (16) and bandwidth (unlimited) per tunnel, allowed service kinds, audit retention (90 days). Create, handshake and relay refuse with explicit codes (`tunnels_disabled`, `tunnel_lifetime_too_long`, `tunnel_too_many_tokens`, `tunnel_kind_not_allowed`, `tunnel_token_lifetime`, `tunnel_owner_only`).
- **Live changes**: each open connection is revalidated against the database and the settings every 15 s (ping tick): revoked, expired, deleted, suspended-account tokens, a disabled feature, a kind no longer allowed or a lifetime over the new maximum close the connection; new limits (streams, bandwidth, kinds, owner only) are applied in place. Saving settings with the feature off closes every tunnel immediately.
- **Explicit confirmation**: `POST /api/tunnels` requires `ack_remote_code: true` (members of the project can run code on the PC). `owner_only` restricts a tunnel to its owner: enforced by the relay (`connected_for`), by the attach and server lookups, changeable live with `PATCH /api/tunnels/{id}`, exposed as `owner_only` and `can_use`.
- **Audit**: table `tunnel_audit` (created, attached, connected, disconnected with duration, bytes and reason, revoked, refused kind, owner-only changes, settings changes), read by administrators at `GET /api/tunnels/audit`, purged beyond the retention, never holding a token. Administrators can also revoke any token.
- **Bandwidth**: a per-tunnel pacing (both directions) delays frames; reading from the agent pauses so TCP back-pressure does the rest (minimum cap 16 KiB/s so a 64 KiB frame never starves the ping).
- **Tests**: stolen/unknown, revoked, expired, deleted and suspended-account tokens; undeclared service or kind; oversized frames; stream and kind limits at handshake and mid-session; admin disabling mid-session; owner-only in relay and API; the token absent from list, status and audit.

Residual risks: refusals of unknown or revoked tokens at the upgrade are logged but not in the audit table (to avoid unauthenticated spam), the bandwidth cap is per tunnel and not per instance, and the PC side remains trusted code execution by design.

## Documentation (#277)

User guide: `site/web/src/content/docs/docs/use/features/pc-tunnel.md` (site page `/docs/use/features/pc-tunnel/`, FR under `/fr/docs/...`). Hosting guide: `.../docs/host/operate/tunnels.md` (`LOOTY_TUNNELS`, administration settings, reverse proxy and WebSocket timeouts for `/api/tunnels/agent`, audit). Glossary entries: *Tunnel*, *Agent*, *Service tunnelé*, *Jeton de tunnel*. Related: ADR 0003 (Jupyter through the backend), ADR 0018 (R kernel and WebR).
