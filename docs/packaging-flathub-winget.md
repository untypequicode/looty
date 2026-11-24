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

Design choices: the app id on Flathub is `io.github.untypequicode.looty` (the Tauri identifier `app.looty.desktop` is only used for Windows toasts). The built-in updater is compiled for Windows only (`#[cfg(windows)]`), so the Flatpak never updates itself: Flathub does. Permissions: network, Wayland/X11, DRI, and D-Bus names for notifications, the tray icon (StatusNotifier) and the secret service (session encryption key). No filesystem access. The metainfo licence field assumes `GPL-3.0-only` (the repository `LICENSE` is GPL-3.0); change it if the project is "or later".

## Flathub (#169)

Check locally (needs `flatpak`, `flatpak-builder`, Python with `aiohttp tomlkit PyYAML`):

```sh
./scripts/flatpak-prepare.sh
cd packaging/flatpak
flatpak-builder --user --install --install-deps-from=flathub --force-clean build io.github.untypequicode.looty.yml
flatpak run io.github.untypequicode.looty
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest io.github.untypequicode.looty.yml
```

Submission (**owner**), for a release that already has its `desktop-vX.Y.Z` tag:

1. `./scripts/flatpak-prepare.sh && node scripts/flatpak-render.mjs --flathub /tmp/looty-flathub`
2. Copy into the submission: `/tmp/looty-flathub/io.github.untypequicode.looty.yml`, `packaging/flatpak/cargo-sources.json`, `packaging/flatpak/io.github.untypequicode.looty.metainfo.xml`, and add the `shared-modules` submodule (`git submodule add https://github.com/flathub/shared-modules.git`). In the Flathub manifest, the `file` source for the metainfo and `cargo-sources.json` then sit next to the manifest. The desktop file and icons come from the tag.
3. Fork `flathub/flathub`, create a branch from `new-pr` and open a PR titled "Add io.github.untypequicode.looty" with those files at the root of the app directory, following https://docs.flathub.org/docs/for-app-authors/submission. Answer the review questions (original project, maintainer, name ownership). Flathub verifies `io.github.untypequicode` through the GitHub account.
4. After acceptance a `flathub/io.github.untypequicode.looty` repository is created. For each later desktop release, regenerate steps 1-2 against the new tag and open a PR there (or enable the Flathub external-data checker/bot if you prefer).
5. Screenshots: the metainfo uses `https://looty.untypequicode.fr/shots/*-light-en.webp`; Flathub requires them to be reachable and a stable size, so replace them with desktop-window screenshots when you have them.

Likely review remarks: the app shows an empty window without a server (explain in the PR); `--talk-name=org.freedesktop.secrets` is justified by the session key.

## winget (#170)

Package id `untypequicode.Looty`, installer type `nullsoft` (Tauri NSIS), per-user scope, `/S` silent switch, x64, URL `https://github.com/untypequicode/looty/releases/download/desktop-vX.Y.Z/Looty_X.Y.Z_x64-setup.exe`.

First submission (**owner**), because `wingetcreate update` needs an existing package:

```sh
node scripts/winget-manifest.mjs X.Y.Z out --download      # needs the published release
winget validate --manifest out/manifests/u/untypequicode/Looty/X.Y.Z   # on Windows
```

Fork `microsoft/winget-pkgs`, copy `out/manifests/...` into the same path, and open a PR. The bot validates the installer (hash, silent install, Defender scan). The installer is not code-signed: SmartScreen warnings are not a blocker for winget, but the reviewers may ask; track signing in issue #85. Add a French locale file later if wanted.

Automatic updates (**owner**, once the package is merged):

1. Create a classic GitHub token with `public_repo` scope on an account that has a fork of `winget-pkgs`.
2. Add it as the repository secret `WINGET_TOKEN` (Settings, Secrets and variables, Actions). Never commit it.
3. Add the repository variable `WINGET_ENABLED` = `true`.

From then on `release.yml` runs `winget.yml` after each desktop release: it uploads the computed manifest as an artifact and runs `wingetcreate update untypequicode.Looty --version X.Y.Z --urls <installer>|x64 --submit`. To retry a version: Actions, "winget", "Run workflow". Without the variable or the secret the job is skipped.

## Verification status

Done offline: `appstreamcli validate`, `desktop-file-validate`, yamllint, winget manifests against the Microsoft 1.10.0 JSON schemas. The Flatpak build itself and `flatpak-builder-lint` run in the "Packaging" workflow; see the PR checks. `wingetcreate` and the real winget validation cannot run outside Windows/CI.
