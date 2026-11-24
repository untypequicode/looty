---
title: "Updates and version pinning"
description: "Update an instance and stay on a given version."
---

By default the compose files use the `latest` images:

```sh
docker compose pull && docker compose up -d
```

To stay on a version, set `LOOTY_VERSION=<x.y.z>` in `.env` (the version of the web release) and run the same commands when you choose to move. Read the release notes (in the app, *What's new*, or on GitHub) before updating, and **back up first**. Database migrations run automatically at start.

When a new release changes the compose files (for example the PostgreSQL storage change in 0.30), the release notes say so: download the new compose file from the release and compare with yours.
