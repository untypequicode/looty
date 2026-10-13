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
`releases.json` has no tag yet, it runs the tests, builds, creates the tag
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

- **Docker images** are public packages of the repository on ghcr.io; nothing to set up
  (the workflow uses `GITHUB_TOKEN`). Make the packages public once in the GitHub package
  settings if you want to pull them without logging in.

## Building by hand

- Docker images: **Docker** workflow (manual run, tags `latest` and `sha-<commit>`), or
  `docker build ./backend` (add `--build-arg WITH_LATEX=0` for an image without TeX Live).
- Desktop: `./desktop/scripts/build-deb.sh`, or `nix build .#looty-desktop`.
- Android: `cd android && ./gradlew assembleRelease`.

Pushing a tag `desktop-v<version>` or `android-v<version>` by hand still works and runs the
matching workflow alone.
