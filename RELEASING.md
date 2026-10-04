# Publishing a version

Looty has three things to release: the **web app** (server + interface, deployed
together), the **Linux desktop app** and the **Android app**. All release notes live in
[`backend/releases.json`](backend/releases.json), in English and French. They are shown in
the app ("What's new" window after an update, *What's new* page in the workspace menu), and
they are used to tell desktop and Android users that a newer version of their app exists.

## Web

1. Add an entry at the top of `backend/releases.json` with `"platform": "web"`, the new
   version, today's date, a title and the notes (`**bold**` and `` `code` `` are allowed;
   the English and French lists must have the same number of items).
2. Set the same version in `backend/Cargo.toml`.
3. `cargo test` (in `backend/`) checks that both match.
4. Deploy. Users see the "What's new" window once, and open tabs show a *Reload* banner
   within 10 minutes.

## Desktop (Linux)

1. Bump `version` in `desktop/src-tauri/Cargo.toml` and `desktop/src-tauri/tauri.conf.json`.
2. Add a `"platform": "desktop"` entry to `backend/releases.json` (same version).
3. Push the tag `desktop-v<version>`: the *Desktop* workflow builds the Nix package and the
   `.deb` and publishes them on the GitHub release page.
4. Deploy the server: desktop apps that are older get a banner and a system notification
   (once per version) pointing to the release page.

## Android

1. Bump `versionName` (and `versionCode`) in `android/app/build.gradle.kts`.
2. Add a `"platform": "android"` entry to `backend/releases.json`.
3. Push the tag `android-v<version>`: the *Android* workflow builds and publishes the signed APK.
4. Deploy the server: older apps get the update banner.

The backend tests also check that the latest desktop and Android entries match
`tauri.conf.json` and `build.gradle.kts`, so a forgotten entry fails CI.
