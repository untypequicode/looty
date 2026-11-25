# Runbook: Flathub and winget (issues #169, #170)

The repository holds everything that can be automated. The steps marked **owner** need the maintainer's GitHub account and cannot be done by CI or an agent. Nothing here is submitted automatically except the winget update PR, and only once you enable it.

## What is in the repository

| Path | Role |
| --- | --- |
| `packaging/flatpak/io.github.untypequicode.looty.yml` | Flatpak manifest (GNOME 50 runtime, Rust from the SDK extension, offline Cargo build) |
| `packaging/flatpak/io.github.untypequicode.looty.desktop` | Desktop entry, icon name = app id |
| `packaging/flatpak/io.github.untypequicode.looty.metainfo.xml.in` | AppStream template (EN + FR, screenshots from the site) |
| `scripts/flatpak-render.mjs` | Writes the final `metainfo.xml`, release notes (EN/FR) from `backend/releases.json`; `--flathub <dir>` also writes the Flathub manifest with a Git source at the latest `desktop-vX.Y.Z` tag and its commit |
| `scripts/flatpak-prepare.sh` | Generates `cargo-sources.json` (flatpak-cargo-generator, pinned), fetches `flathub/shared-modules`, renders the metainfo |
| `scripts/winget-manifest.mjs` | Generates the 3 winget YAML files from `releases.json` and the released installer (`--download`, `--exe file` or `--sha256`) |
| `.github/workflows/packaging.yml` | PR checks: Flatpak build + launch + `flatpak-builder-lint` + `appstreamcli validate`; winget manifest against the Microsoft JSON schemas |
| `.github/workflows/winget.yml` | Opens the update PR on `microsoft/winget-pkgs` with `wingetcreate` (called by `release.yml` after a desktop release) |

Generated files (`cargo-sources.json`, `shared-modules/`, `metainfo.xml`) are not committed; the scripts recreate them.

Design choices: the app id on Flathub is `io.github.untypequicode.looty` (the Tauri identifier `app.looty.desktop` is only used for Windows toasts). The built-in updater is compiled for Windows only (`#[cfg(windows)]`), so the Flatpak never updates itself: Flathub does. Permissions: network, Wayland/X11, DRI, and D-Bus names for notifications, the tray icon (StatusNotifier) and the secret service (session encryption key). No filesystem access. The metainfo licence field is `GPL-3.0-only`.

## Flathub (#169)

Check locally (needs `flatpak`, `flatpak-builder`, Python with `aiohttp tomlkit PyYAML`):

```sh
./scripts/flatpak-prepare.sh
cd packaging/flatpak
flatpak-builder --user --install --install-deps-from=flathub --force-clean build io.github.untypequicode.looty.yml
flatpak run io.github.untypequicode.looty
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest io.github.untypequicode.looty.yml
```

Submission (**owner**), for a release that already has its `desktop-vX.Y.Z` tag (latest: `desktop-v0.8.4`):

1. `./scripts/flathub-submission.sh [tag] [dir]` (needs node and python3 with aiohttp, tomlkit, PyYAML; with Nix: `nix-shell -p nodejs_24 git 'python3.withPackages (p: [p.aiohttp p.tomlkit p.pyyaml])' --run ./scripts/flathub-submission.sh`). It builds everything from the tag itself and writes, by default in `/tmp/looty-flathub-submission`: the Flathub manifest (Git source at the tag and its commit), `cargo-sources.json`, `metainfo.xml` and `flathub.json` (x86_64 only).
2. Fork `flathub/flathub` (**owner**, GitHub account), then:
   ```sh
   gh repo fork flathub/flathub --clone && cd flathub
   git checkout -b new-pr origin/new-pr    # the PR must target the new-pr branch
   git checkout -b add-looty
   cp /tmp/looty-flathub-submission/* .
   git submodule add https://github.com/flathub/shared-modules.git shared-modules
   git -C shared-modules checkout 34add58f536223b780a3c9f077980cdaa51fc81f   # revision pinned in flatpak-prepare.sh
   git add -A && git commit -m "Add io.github.untypequicode.looty" && git push -u origin add-looty
   gh pr create --repo flathub/flathub --base new-pr --title "Add io.github.untypequicode.looty" --body-file pr-body.md
   ```
   Check the current instructions first: https://docs.flathub.org/docs/for-app-authors/submission.
3. After acceptance a `flathub/io.github.untypequicode.looty` repository is created. For each later desktop release, rerun step 1 and open a PR there (or enable the Flathub external-data checker/bot).
4. Screenshots: the metainfo uses `https://looty.untypequicode.fr/shots/*-light-en.webp`; Flathub requires them to be reachable and a stable size, so replace them with desktop-window screenshots when you have them.

Licence: `GPL-3.0-only` everywhere (README, Cargo/package manifests, metainfo, F-Droid, winget `GPL-3.0`), confirmed by the maintainer.

Ready-to-paste PR body (title: `Add io.github.untypequicode.looty`):

```md
### App
Looty is the desktop client of Looty, a self-hosted team workspace (tasks, wiki pages, calendar, files). It is a Tauri (Rust + WebView) app that shows the web interface of a Looty server the user chooses. https://looty.untypequicode.fr , source: https://github.com/untypequicode/looty

### Checklist answers
- Original upstream project: yes, I am the maintainer and author (github.com/untypequicode/looty). The app id `io.github.untypequicode.looty` matches the GitHub account `untypequicode`.
- Licence: GPL-3.0-only. Source is public; the manifest builds from the `desktop-v0.8.4` tag with the Cargo dependencies vendored offline through flatpak-cargo-generator (`cargo-sources.json`).
- No bundled binaries; Rust comes from `org.freedesktop.Sdk.Extension.rust-stable`, the tray library from `flathub/shared-modules`.
- Built-in updater: compiled for Windows only, so the Flatpak never updates itself.
- Permissions, each justified: `--share=network` (the app talks to the user's Looty server); `--socket=wayland`, `--socket=fallback-x11`, `--share=ipc`, `--device=dri` (window and GPU rendering of the WebView); `--talk-name=org.freedesktop.Notifications` (task and message notifications); `--talk-name=org.kde.StatusNotifierWatcher` (tray icon); `--talk-name=org.freedesktop.secrets` (the session encryption key is stored in the keyring). No filesystem access: downloads and "Save as" go through portals.
- Without a server the window is mostly empty (it asks for the server address): this is expected, the client needs a Looty instance (documentation: https://looty.untypequicode.fr/docs/).
- Validation: `appstreamcli validate` and `flatpak-builder-lint` pass in the project's CI, which also builds and launches the Flatpak on every pull request.
- Screenshots come from the project site; metainfo is bilingual (EN, FR) with release notes.
```

Likely review remarks: the app shows an empty window without a server (explain in the PR); `--talk-name=org.freedesktop.secrets` is justified by the session key.

## winget (#170)

Package id `untypequicode.Looty`, installer type `nullsoft` (Tauri NSIS), per-user scope, `/S` silent switch, x64, URL `https://github.com/untypequicode/looty/releases/download/desktop-vX.Y.Z/Looty_X.Y.Z_x64-setup.exe`.

First submission (**owner**), because `wingetcreate update` needs an existing package. The manifests for the latest desktop release (0.8.4, schema-validated against the Microsoft 1.10.0 JSON schemas) are in `packaging/winget/manifests/u/untypequicode/Looty/0.8.4/`; regenerate them with `node scripts/winget-manifest.mjs X.Y.Z packaging/winget --download` if a newer desktop release exists. One command does the rest, asking for confirmation before each GitHub action (fork `microsoft/winget-pkgs`, branch, push, PR):

```sh
gh auth status   # logged in as the account that will own the fork
./scripts/winget-first-pr.sh        # or: ./scripts/winget-first-pr.sh 0.8.4
```

The bot validates the installer (hash, silent install, Defender scan). The installer is not code-signed: SmartScreen warnings are not a blocker for winget, but the reviewers may ask; track signing in issue #85. Add a French locale file later if wanted. `winget validate` on Windows is the only check not run here.

Automatic updates (**owner**, once the package is merged):

1. Create a classic GitHub token with `public_repo` scope on an account that has a fork of `winget-pkgs`.
2. Add it as the repository secret `WINGET_TOKEN` (Settings, Secrets and variables, Actions). Never commit it.
3. Add the repository variable `WINGET_ENABLED` = `true`.

From then on `release.yml` runs `winget.yml` after each desktop release: it uploads the computed manifest as an artifact and runs `wingetcreate update untypequicode.Looty --version X.Y.Z --urls <installer>|x64 --submit`. To retry a version: Actions, "winget", "Run workflow". Without the variable or the secret the job is skipped.

## Verification status

F-Droid: see [packaging-fdroid.md](packaging-fdroid.md). Done offline: `appstreamcli validate`, `desktop-file-validate`, yamllint, winget manifests against the Microsoft 1.10.0 JSON schemas. The Flatpak build itself and `flatpak-builder-lint` run in the "Packaging" workflow; see the PR checks. `wingetcreate` and the real winget validation cannot run outside Windows/CI.
