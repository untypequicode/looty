# Browser tests (e2e)

Playwright tests that drive a real Chromium against a running Looty. No Moodle or timetable server is needed: `lib/mock-moodle.mjs` simulates Moodle (web services, calendar export, mobile login) on `127.0.0.1:18300`, and `run.mjs` starts it.

What you need running first:

- the backend with `LOOTY_DEV_LOGIN=true` (passwordless `/api/auth/dev-login`) and `CALDAV_ALLOW_PRIVATE=true` (so the workspace can point to the local mock Moodle);
- the collaboration server (only for `ws-proxy.mjs`; set `E2E_COLLAB=0` to skip that check);
- the frontend (`node frontend/.output/server/index.mjs`), built with `BACKEND_URL` and `COLLAB_URL` set, because the `/api` relay is fixed at build time.

```sh
cd e2e
npm ci
npx playwright-core install chromium      # or CHROMIUM=/path/to/chromium
E2E_BASE=http://localhost:3000 node run.mjs          # all tests
E2E_BASE=http://localhost:3000 node run.mjs courses  # tests whose file name contains "courses"
```

`run.mjs` signs in as `alice@x.io`, creates a fresh workspace linked to the mock Moodle and passes its id to each test in `tests/` as `WS`. Screenshots go to `out/` (ignored by git). CI runs all of it in the **Browser tests** job of `ci.yml`.

Adding a test: create `tests/<name>.mjs`, import `ok`, `launch`, `login`, `api` from `../lib/index.mjs` and print `PASS …`/`FAIL …` lines through `ok()`.
