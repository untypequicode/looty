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
