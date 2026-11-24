---
title: "Requirements"
description: "Machine, Docker or NixOS, domain name and ports needed to host Looty."
---

- **A machine** you control (a small VPS is enough for a team): Linux on amd64 or arm64. The compose files cap memory at about 1 GB for the backend, 768 MB for PostgreSQL and a few hundred MB for the other services; plan **2 GB of RAM** and a few GB of disk plus room for uploads.
- **Docker** with the Compose plugin. On NixOS, enable Docker (`virtualisation.docker.enable = true;`) and use the same compose files: there is no dedicated NixOS module.
- **A domain name** whose DNS points to the machine (and a second one, `auth.<domain>`, if you use the bundled Pocket ID).
- **Ports 80 and 443** free, for HTTPS. With the bundled Caddy, certificates come from Let's Encrypt automatically. Behind an existing proxy, only that proxy needs them.
- **An outgoing mail server** (optional) for email notifications.
- **A sign-in method**: an OpenID Connect provider, or the bundled Pocket ID (see [sign-in](/docs/host/sign-in/oidc/)).
