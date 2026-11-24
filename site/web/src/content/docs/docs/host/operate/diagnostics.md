---
title: "Diagnosing your instance"
description: "The Diagnostic page and the doctor command, plus the logs."
---

## The Diagnostic page and `looty doctor`

*Administration > Diagnostic* checks that the instance is installed properly, and shows OK, a warning or an error for each point with a link to the [troubleshooting guide](/docs/host/operate/troubleshooting/):

- `BASE_URL` matches the address you use, HTTPS and the certificate are valid, the proxy forwards `Host`, `X-Forwarded-Proto` and `X-Forwarded-For`;
- the WebSockets `/api/ws` (real time) and `/collab` (live editing) open from your browser;
- CalDAV discovery (`/.well-known/caldav`), each OpenID Connect provider (discovery, return address to register), SMTP (connection, and a test email on request);
- the database (version, size), the free disk space, the uploads folder, the encryption key and the latest published version (if `LOOTY_UPDATE_CHECK` is on).

The same checks, without the ones that need a browser, run from a terminal and exit with code 1 when something is an error:

```sh
docker compose exec backend looty doctor          # add --json for scripts
```

The first-launch guide at `/setup` ends with a link to this page.

## Logs

```sh
docker compose ps                         # all services should be running (postgres healthy)
docker compose logs --tail 100 backend    # API, sign-in, mail, migrations
docker compose logs --tail 100 collaboration frontend
curl -s https://<domain>/api/health       # {"version": "..."} when the backend answers
```

The sign-in page tells you when the OpenID Connect provider cannot be reached or rejected Looty; the backend log then names the cause (wrong issuer, wrong secret, unknown redirect URL). Set `RUST_LOG=debug` on the backend for more detail while you investigate.
