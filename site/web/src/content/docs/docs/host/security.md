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

## Uploaded files

Since 0.66.0 the files under `/uploads/` (images and attachments of pages, tasks, avatars) are **not public any more**.

- Each file is tied to the page, task or space it was uploaded for, and is served only to a signed-in person (session cookie, API token, the Android and desktop apps) who can read that page, task or space. Take the access away and the file stops loading at once, even for someone who still has its address. An answer is `404` for a signed-in person without access and `401` without sign-in.
- Public by design: avatars and the instance icon. A page shared with a **public link** serves its own images to anyone who has the link, and only those images, as long as the link exists.
- Files are never kept by a shared cache (`Cache-Control: private, no-cache`, `Vary: Cookie`); media support range requests.
- Files that existed before the update are tied to the first page, task or avatar that cites them during the migration. Files cited nowhere and with no known uploader are listed as **unattributed** (log line at startup and the `unattributed_files` count of the storage overview in the admin API) and only instance administrators can open them.
- A client that cannot send credentials can ask `GET /api/uploads/{name}/link` for a signed link valid 10 minutes. As a last resort, `LOOTY_UPLOADS_PUBLIC=true` brings back the old open behavior.
- Exports, backups and e-mails sent from Looty read the files from the server, not from this address: they are unchanged, except that an e-mail cannot attach a file its sender cannot open.

## PC tunnels

Details: [PC tunnels](/docs/host/operate/tunnels/).

A PC tunnel lets a person connect their own computer to a project (Jupyter, LaTeX) with an outgoing connection. **Members of the project can then run code on that computer**, so the feature is off until you turn it on and every limit is yours to set.

- **Turn on / off**: Administration > Instance > Tunnels. `LOOTY_TUNNELS=on` only gives the initial value, until the first save. Turning it off closes the tunnels in progress right away.
- **Limits**: tunnels per person, maximum token lifetime, simultaneous streams and bandwidth per tunnel, and the allowed kinds of service (`jupyter`, `latex`, `http`). A change applies to open tunnels within 15 seconds; a token that lives longer than the new maximum stops connecting.
- **Confirmation**: creating a tunnel requires the person to confirm (`ack_remote_code`) that members can run code on their PC. A tunnel can be limited to its owner (`owner_only`): other members of the project cannot use it.
- **Audit**: Administration > Instance shows who created, attached, connected, disconnected and revoked which tunnel, and when. Events older than the retention (90 days by default) are deleted. Tokens never appear in it.
- **Tokens**: stored hashed, shown once, revocable (the owner or an administrator), checked again every 15 seconds: a revoked, expired or suspended-account token is cut. The instance never reaches the PC's network, only the services the agent declares.
