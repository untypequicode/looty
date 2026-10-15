# To do

Ordered by priority. Items marked **(you)** need access only the maintainer has: GitHub settings, devices, accounts.

## 1. Release pipeline: finish the setup and clean up

- [ ] **(you)** Delete the old files attached to the `desktop-v0.7.1` and `web-v0.16.8` releases. Keep only `Looty_0.7.1_amd64.deb`, `Looty_0.7.1_x64-setup.exe`, `looty-0.12.1.apk` and `docker-compose.yml`. A cached build folder had been publishing every previous `.deb`/`.exe` (fixed in CI).
- [ ] **(you)** Check the repository settings against [RELEASING.md](RELEASING.md):
  - Android secrets: `ANDROID_KEYSTORE_B64`, `…_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`;
  - apt signing: `APT_GPG_PRIVATE_KEY` (and `APT_GPG_PASSPHRASE` if the key has one);
  - download site: variable `SITE_URL` and secret `SITE_UPLOAD_TOKEN`;
  - Nix cache: `CACHIX_AUTH_TOKEN` (+ `CACHIX_CACHE`).
- [ ] **(you)** Turn on "Require status checks" for `main` with the new **CI** workflow, so a red build cannot be merged.
- [ ] Update the local `main` branch (82 commits behind `origin/main`), or delete it locally and work from `redesign` only.
- [ ] Merge this branch into `main` and check that the Release workflow publishes `web-v0.16.9` with only the expected files.

## 2. Automated tests in the repository

The end-to-end Playwright scripts used during development live in `/tmp/pw` and are lost on reboot.

- [ ] Move them into `e2e/`: sign-in via `/api/auth/dev-login`, boards and tasks, the document editor (math, Mermaid, undo), calendar, mail (mock IMAP/SMTP), Moodle (mock server), projects and notebooks.
- [ ] Add a docker-compose stack for tests: Postgres, backend with `LOOTY_DEV_LOGIN=true`, collab, frontend, Dovecot and an SMTP sink.
- [ ] Run a smoke subset of these tests in CI.
- [ ] Add `nuxi typecheck` (vue-tsc) to CI and fix the errors it reports. Right now only `nuxt build` runs, and it does not type-check.
- [ ] Add ESLint (`@nuxt/eslint`, configured in the Nuxt modules this time) and `cargo clippy -D warnings`. Fix the 34 style warnings in `backend/` first, mostly "very complex type" and `sort_by_key`.
- [ ] Add backend integration tests against a real Postgres for permissions (`access.rs`, `roles.rs`) and backups (`backup.rs`). These protect the most data.

## 3. Self-hosting without Traefik

- [ ] Real-time collaboration (`/collab` WebSocket) only works behind a reverse proxy. The frontend's fallback proxy handles `/api`, `/uploads`, `/pages` and `/dav`, but not WebSockets. Either proxy `/collab` (and `/api/ws`) from the Nitro server, or ship a ready-made Caddy/nginx example. Then document `docker compose up` as fully working.
- [ ] Add healthchecks for `backend` (`/api/health`), `collaboration` and `frontend` in both compose files, so `depends_on` waits for readiness.

## 4. Platforms not yet tested on real hardware

- [ ] **(you)** Windows: install the NSIS installer on Windows 10/11 and check toasts (click, "Mark as done"), tray, global shortcuts, autostart, `looty://` links, Save as and the taskbar badge. Nothing on Windows has ever been run.
- [ ] Windows updates: add the Tauri updater, or at least an "Update available → download" notification. Code signing needs a certificate **(you)**.
- [ ] **(you)** Linux global shortcuts: test on GNOME and KDE (XDG GlobalShortcuts portal); only Hyprland/headless was tested.
- [ ] **(you)** iPhone/iPad PWA: check that push notifications keep arriving after using the app in the foreground (fixed in `sw.js`, untested on a device).
- [ ] Android:
  - install a signed release over the previous one (in-app update path);
  - test Moodle offline with the real Moodle;
  - offline access to documents never opened, and to attachments.

## 5. Quality and performance

- [ ] Frontend bundle: Vite warns about large chunks. Make sure Mermaid, Excalidraw (and React), Pyodide, CodeMirror and pdf.js load only when used (`defineAsyncComponent` / dynamic `import()`).
- [ ] Accessibility pass (WCAG AA): contrast of the amber/orange status texts in light mode, keyboard access to the board (drag and drop has no keyboard alternative), focus trapping in dialogs and sheets.
- [ ] Move to Node 26 LTS once it is released (late October 2026). Update the Dockerfiles, `shell.nix`, `engines` and `node-version` in the workflows.
