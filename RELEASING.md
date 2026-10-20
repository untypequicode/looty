# Publishing a version

Looty has three things to release: the **web app** (server + interface, deployed
together), the **Linux desktop app** and the **Android app**. All release notes live in
[`backend/releases.json`](backend/releases.json), in English and French. They are shown in
the app ("What's new" window after an update, *What's new* page in the workspace menu), they
tell desktop and Android users that a newer version of their app exists, and they become the
text of the GitHub releases.

## The short version

1. Bump the version of what you release (see the table below).
2. Add the matching entry at the top of `backend/releases.json`.
3. `cargo test` in `backend/` (checks that versions and notes match).
4. Merge into `main`.

The **Release** workflow then does everything else: for each platform whose latest version in
`releases.json` has no tag yet, it runs the tests (the **CI** workflow), builds, creates the tag
`<platform>-v<version>` and publishes the GitHub release with the notes and the files.

| Platform | Version to bump | Built and published |
|---|---|---|
| web | `backend/Cargo.toml` | Docker images `ghcr.io/untypequicode/looty-{backend,frontend,collaboration}:<version>` (and `:latest`), for amd64 and arm64; the release has a `docker-compose.yml` with images pinned to that version |
| desktop | `desktop/src-tauri/Cargo.toml` and `desktop/src-tauri/tauri.conf.json` | `.deb` (Debian 12 / Ubuntu 22.04+, install-tested on a clean Debian) and the Windows installer (`*-setup.exe`, unsigned); Nix package pushed to the binary cache for x86_64 and aarch64 |
| android | `versionName` and `versionCode` in `android/app/build.gradle.kts` | APK, signed when the keystore secrets are set (otherwise a debug APK) |

Releasing only one platform is fine: the others are skipped because their tag already exists.
To rebuild a release (for example after fixing CI), run **Release** by hand with
`force` = `web`, `desktop` and/or `android`.

### Writing `releases.json` entries

`"platform"`, `"version"`, `"date"` (YYYY-MM-DD), `"title"` and `"notes"` in `en` and `fr`.
`**bold**` and `` `code` `` are allowed; the English and French lists must have the same number
of items.

## Where people download the apps

The repository is public, so by default everything comes from **GitHub**: the apps are release assets, the Android
app updates itself from the latest `android-v*` release (checking the SHA-256 digest GitHub publishes), and the apt
repository can live on GitHub Pages (branch `gh-pages`; enable Pages once in the repository settings).

You can also run the optional **Looty download site** (`site/`: a presentation page, downloads, `/latest.json`, apt
repository; one tiny container, see `site/README.md`). When `SITE_URL` and `SITE_UPLOAD_TOKEN` are set, the
**Release** workflow uploads the signed APK, the `.deb`, the Windows installer and the apt repository there too, the
server tells the apps its address (`"site"` in `backend/releases.json`, or `LOOTY_SITE_URL`) and they prefer it to
GitHub. Without them, those steps are skipped and the apt repository goes to GitHub Pages.

The apt repository is signed: run `desktop/scripts/apt-key.sh` once, commit the public key
`desktop/packaging/looty-archive-keyring.gpg` and add the secret `APT_GPG_PRIVATE_KEY` (and `APT_GPG_PASSPHRASE`).

## After a release

- **Server:** pull the new images and restart (`docker compose -f docker-compose.prod.yml pull && docker compose -f docker-compose.prod.yml up -d`),
  or pin a version with `LOOTY_VERSION=<version>` in `.env`. Users see the "What's new"
  window once; open tabs show a *Reload* banner within 10 minutes.
- **Desktop and Android:** once the server runs the new `releases.json`, older apps get an
  update banner (and, on the desktop, a system notification once per version) pointing to
  the download site (or the GitHub release page when no site is set). The Android app can
  download and install the update itself; Debian and Ubuntu users get it with `apt upgrade`.

## One-time repository setup

- **Android signing:** secrets `ANDROID_KEYSTORE_B64` (keystore in base64),
  `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`.
- **Nix binary cache (Cachix):** create a cache on [cachix.org](https://app.cachix.org)
  (for example `looty`; names are global, so pick another one if it is taken), then add the repository variable `CACHIX_CACHE` (the cache name)
  and the secret `CACHIX_AUTH_TOKEN` (a write token). The **Nix cache** workflow pushes
  every desktop build of `main` (until the token is set, it still builds and only skips the push). The cache `looty.cachix.org` and its public key
  are already declared in `flake.nix` (`nixConfig`): Nix offers to trust it the first time,
  or pass `--accept-flake-config`.

  **Goal: Nix users on x86_64/aarch64 Linux never compile.** Covered: `looty-desktop`,
  `nix develop` and `nix develop .#desktop`. Not covered: `.#android` (the Android SDK is not
  redistributable).
  - **Pins:** each push on `main` pins `main`, `main-shell-desktop` and `main-shell-default`
    per system (last 3 revisions). Each desktop release pins `release-<version>-<system>`
    forever. Pinned paths are never evicted.
  - **Eviction:** when the cache is full, Cachix deletes the least recently used *unpinned*
    paths (emails at 85 % and 100 %). Plan: free. Usage on 2026-10-20: 277 MiB. Check the
    storage on app.cachix.org now and then; above 80 % of the limit, remove old release pins
    in the cache's *Pins* page (or `cachix pin` with `--keep-revisions`).
  - **Check:** the **Nix cache check** workflow builds the desktop package and the inputs of both dev shells with `--max-jobs 0` (any
    compilation fails it; the shell wrapper itself is never downloaded, `mkShell` forbids it). It runs after each push on `main`, for each desktop release, every
    Monday and by hand. A failure opens one issue labelled `nix-cache`; it is closed by the
    next green run. To fix: run **Nix cache** by hand on `main` (it rebuilds and pushes), then
    **Nix cache check**.
  - A pull request that changes `desktop/` or `flake.lock` fills the cache only once merged,
    so the cache check right after the merge is what covers that short window.

- **Docker images** are public packages of the repository on ghcr.io; nothing to set up
  (the workflow uses `GITHUB_TOKEN`). Make the packages public once in the GitHub package
  settings if you want to pull them without logging in.

## Building by hand

- Docker images: **Docker** workflow (manual run, tags `latest` and `sha-<commit>`), or
  `docker build ./backend` (add `--build-arg WITH_LATEX=0` for an image without TeX Live).
- Desktop: `./desktop/scripts/build-deb.sh`, or `nix build .#looty-desktop`.
- Android: `cd android && ./gradlew assembleRelease`.

Pushing a tag by hand publishes nothing: releases always go through `releases.json` and the **Release**
workflow (run it by hand with `force` to rebuild one).

## Workflows

| Workflow | Runs on | Does |
|---|---|---|
| **CI** | pull requests and `main` (server code), called by Release | backend tests, translations check, frontend build, collaboration server check |
| **Release** | `backend/releases.json` changed on `main`, or by hand | everything in this document |
| **Docker** | called by Release, or by hand | multi-arch images on ghcr.io |
| **Desktop** | pull requests touching `desktop/`, called by Release | `.deb` (install-tested) and Windows installer |
| **Android** | pull requests touching `android/`, called by Release | APK |
| **Nix cache** | pull requests and `main` touching `desktop/` or the flake | desktop package and dev shells for x86_64 and aarch64, pushed and pinned in Cachix from `main` |
| **Nix cache check** | after **Nix cache** on `main`, called by Release, every Monday, by hand | builds everything from the cache only; opens an issue if something would be compiled |
| **Site** | `site/` changes, tags `site-v*` | site tests and its Docker image |
| **Site sync** | by hand | refills the download site from the latest GitHub releases |
