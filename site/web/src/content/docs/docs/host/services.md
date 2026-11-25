---
title: "Email, push, Moodle, Jupyter and storage"
description: "Optional services: SMTP email, push notifications, Moodle, Jupyter and file storage."
---

## Email (SMTP)

Set it up in *Administration > Email* (recommended), or with variables as a fallback: `SMTP_HOST`, `SMTP_FROM` (both required to enable it), `SMTP_PORT`, `SMTP_SECURITY` (`starttls`, `tls`, `none`), `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM_NAME`. The compose files pass these from `.env`.

## Push notifications

- **Web Push** (browsers and iPhone web app) works out of the box: Looty creates its own VAPID key on first use and stores it encrypted. Your instance must be served over HTTPS and be able to reach the push services of the browsers.
- **UnifiedPush** (Android): people install a UnifiedPush distributor (for example ntfy) on their phone and choose it in the app. You can run your own distributor server; nothing to set in Looty.

## Moodle

People link their own Moodle account from their settings; nothing is required on the instance. Optional tuning: `MOODLE_POLL_SECONDS`, `MOODLE_CACHE_MB`, `MOODLE_CACHE_FILE_MB` (not passed by the compose files: add them in a `docker-compose.override.yml`).

## Jupyter

Project Jupyter servers are contacted by the Looty server only. By default servers on a private network or over `http` are refused: set `JUPYTER_ALLOW_PRIVATE=true` if your Jupyter server runs on your LAN or the same host. To let people use their own computer as a Jupyter server without opening any port, see [PC tunnels](/docs/host/operate/tunnels/).

## CalDAV and AI providers

External CalDAV servers on a private network need `CALDAV_ALLOW_PRIVATE=true`; AI providers on a private network (for example Ollama) need `AI_ALLOW_PRIVATE=true`.

## File storage

Uploads are kept in `./backend/uploads` and backups in `./backend/backups` next to the compose file (mounted into the backend). PostgreSQL data is in `./data/postgres` (`LOOTY_DB_DATA` changes it). Include all of them in your backups. LaTeX needs a compiler in the backend image or in `LATEX_BIN_DIR`.
