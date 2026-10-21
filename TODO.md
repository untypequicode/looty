# To do

Short list, most important first. **(you)** = only you can do it (GitHub settings, a device, an account).

## Now

1. **(you)** Windows updates: once desktop 0.8.0 is published, check that `desktop-v0.8.0` has a `*-setup.exe.sig` and that the `updater` release has `latest.json`. The update itself can only be tested with a later version (install 0.8.0, publish 0.8.1, wait up to 6 hours). Keep a backup of the updater key (`TAURI_SIGNING_PRIVATE_KEY`): if it is lost, installed apps can no longer be updated.

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

- Node 26 once it becomes the LTS version (end of October 2026).
- Moodle: the course personalization (short name, colour, icon) is deliberately not applied to the Looty calendar events imported from Moodle, see [docs/adr/0001-moodle-personalization-not-in-looty-calendar.md](docs/adr/0001-moodle-personalization-not-in-looty-calendar.md).
