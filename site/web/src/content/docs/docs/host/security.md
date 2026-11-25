---
title: "Security"
description: "Secrets, LOOTY_DEV_LOGIN and what to expose."
---

- **Secrets**: set a strong `DB_PASSWORD` and a random `LOOTY_SECRET_KEY` (`openssl rand -base64 32`). Never commit `.env`. Keep `LOOTY_SECRET_KEY` with your backups.
- **`LOOTY_DEV_LOGIN` must never be enabled** on a real instance: it lets anyone sign in as any user. It is not set by the compose files.
- **`LOOTY_DEMO`** is for a public demo only, in its own stack (`docker-compose.demo.yml`).
- **Exposure**: only the proxy should listen on ports 80 and 443. PostgreSQL and the three services stay on the internal Docker network: do not publish their ports.
- **Private networks**: leave `CALDAV_ALLOW_PRIVATE`, `JUPYTER_ALLOW_PRIVATE` and `AI_ALLOW_PRIVATE` at `false` unless you need them; they let users make your server contact machines on your network.
- **Sign-in**: restrict who can create an account at your identity provider. With Google, restrict the consent screen.
- **Updates**: apply them regularly, after a backup.

## PC tunnels

A PC tunnel lets a person connect their own computer to a project (Jupyter, LaTeX) with an outgoing connection. **Members of the project can then run code on that computer**, so the feature is off until you turn it on and every limit is yours to set.

- **Turn on / off**: Administration > Instance > Tunnels. `LOOTY_TUNNELS=on` only gives the initial value, until the first save. Turning it off closes the tunnels in progress right away.
- **Limits**: tunnels per person, maximum token lifetime, simultaneous streams and bandwidth per tunnel, and the allowed kinds of service (`jupyter`, `latex`, `http`). A change applies to open tunnels within 15 seconds; a token that lives longer than the new maximum stops connecting.
- **Confirmation**: creating a tunnel requires the person to confirm (`ack_remote_code`) that members can run code on their PC. A tunnel can be limited to its owner (`owner_only`): other members of the project cannot use it.
- **Audit**: Administration > Instance shows who created, attached, connected, disconnected and revoked which tunnel, and when. Events older than the retention (90 days by default) are deleted. Tokens never appear in it.
- **Tokens**: stored hashed, shown once, revocable (the owner or an administrator), checked again every 15 seconds: a revoked, expired or suspended-account token is cut. The instance never reaches the PC's network, only the services the agent declares.
