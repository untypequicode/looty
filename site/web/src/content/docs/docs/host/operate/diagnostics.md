---
title: "Diagnosing your instance"
description: "Check that every service is up and read the logs."
---

```sh
docker compose ps                         # all services should be running (postgres healthy)
docker compose logs --tail 100 backend    # API, sign-in, mail, migrations
docker compose logs --tail 100 collaboration frontend
curl -s https://<domain>/api/health       # {"version": "..."} when the backend answers
```

The sign-in page tells you when the OpenID Connect provider cannot be reached or rejected Looty; the backend log then names the cause (wrong issuer, wrong secret, unknown redirect URL). Set `RUST_LOG=debug` on the backend for more detail while you investigate.
