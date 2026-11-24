---
title: "Host Looty"
description: "Self-host Looty: requirements, installation, sign-in, operations and security."
---

Looty does not offer hosting: **this documentation is the way to get your own instance**. It is three small services plus PostgreSQL, shipped as Docker images for amd64 and arm64.

| Service | Role |
|---|---|
| `backend` | API, files, CalDAV, mail (Rust) |
| `frontend` | Web interface (Nuxt) |
| `collaboration` | Real-time document editing (Hocuspocus, Yjs) |

## Follow the guide

1. [Check the requirements](/docs/host/requirements/).
2. Generate your files with the [configuration generator](/install/), or follow the [quick install](/docs/host/quick-install/) or the [detailed install](/docs/host/install/) with Caddy, Traefik or another proxy.
3. [Choose how people sign in](/docs/host/sign-in/oidc/) (OpenID Connect recommended).
4. Configure [email, push, Moodle, Jupyter and storage](/docs/host/services/).
5. Plan [backups](/docs/host/operate/backup/), [updates](/docs/host/operate/updates/) and read the [security checklist](/docs/host/security/).

Every setting is listed in the [environment variables reference](/docs/reference/environment/).
