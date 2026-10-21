# To do

Short list, most important first. **(you)** = only you can do it (GitHub settings, a device, an account).

## Now

1. **(you)** Windows updates: add the repository secret `TAURI_SIGNING_PRIVATE_KEY` (the content of the updater private
   key; the public key is already in `desktop/src-tauri/tauri.conf.json`). Without it the Windows installer is built
   unsigned and Windows does not update itself. Keep a backup of the key: if it is lost, installed apps can no longer
   be updated.

## Android: native app

Done (0.13.0 to 0.18.0, phases N1 to N6): every screen is native. Still opened in a web view, on purpose:
- editing a document or a project's files (the live collaborative editor);
- roles and permissions, and server administration (rarely used, complex forms).

## Testing

- Browser tests: `e2e/` (see [e2e/README.md](e2e/README.md)), run by the **Browser tests** job of CI. More of the old `/tmp/pw` scripts can be ported there when needed.
- **(you)** Windows: the **Windows installer test** job (desktop.yml) now installs and runs the app on a clean
  Windows runner. What a runner cannot see (notification clicks, tray menu, shortcuts) is in
  [desktop/tests/windows/CHECKLIST.md](desktop/tests/windows/CHECKLIST.md): go through it once on a real PC.
- **(you)** iPhone: check that notifications keep coming after using the installed web app.

## Later

- PostgreSQL 15 → 18 on your server: needs a dump and restore, plan a maintenance window **(you)**.
- Node 26 once it becomes the LTS version (end of October 2026).
- Moodle: the course personalization (short name, colour, icon) is deliberately not applied to the Looty calendar events imported from Moodle, see [docs/adr/0001-moodle-personalization-not-in-looty-calendar.md](docs/adr/0001-moodle-personalization-not-in-looty-calendar.md).
