# To do

Short list, most important first. **(you)** = only you can do it (GitHub settings, a device, an account).

## Now

1. **(you)** Clean the old files on GitHub. Open the `desktop-v0.7.1` and `web-v0.16.8` releases and delete every old
   `.deb` / `.exe`; keep only the 0.7.1 files, `looty-0.12.1.apk` and `docker-compose.yml`. The next releases won't add
   old files any more (fixed).
2. **(you)** Merge `redesign` into `main`. The Release workflow then publishes web 0.16.9, desktop 0.7.2 and
   Android 0.18.0.
3. **(you)** In GitHub → Settings → Branches, make the **CI** check required for `main`, so a broken build cannot be
   merged.

## Android: native app

Done (0.13.0 to 0.18.0, phases N1 to N6): every screen is native. Still opened in a web view, on purpose:
- editing a document or a project's files (the live collaborative editor);
- roles and permissions, and server administration (rarely used, complex forms).

## Testing

- The automatic browser tests live in `/tmp/pw` on this machine and disappear when `/tmp` is cleaned. Move them into the
  repository (`e2e/`) and run a few in CI.
- **(you)** Windows: the **Windows installer test** job (desktop.yml) now installs and runs the app on a clean
  Windows runner. What a runner cannot see (notification clicks, tray menu, shortcuts) is in
  [desktop/tests/windows/CHECKLIST.md](desktop/tests/windows/CHECKLIST.md): go through it once on a real PC.
- **(you)** iPhone: check that notifications keep coming after using the installed web app.

## Later

- Windows automatic updates (Tauri updater: needs a free signing key) and a code-signing certificate **(you)** so
  Windows stops warning about an unknown publisher.
- PostgreSQL 15 → 18 on your server: needs a dump and restore, plan a maintenance window **(you)**.
- Node 26 once it becomes the LTS version (end of October 2026).
- Collaborative editing without Traefik (`docker compose up` alone): the `/collab` WebSocket needs a proxy.
- The local Docker Compose stack (`looty-2-backend-1`, built on 2026-10-08) restarts in a loop with
  `Migrate(VersionMismatch(20261020120000))`: its database holds a migration whose file changed since the image was
  built. Rebuild the images (`docker compose build`) or reset that database **(you)**, once the Moodle dashboard work is done.
- Moodle: the course personalization (short name, colour, icon) is not applied to the Looty calendar events imported
  from Moodle (the ICS feed does not reliably link an event to its course).
