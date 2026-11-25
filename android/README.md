# Looty for Android

Native Kotlin app (Jetpack Compose, Material 3 Expressive, Material You colours). Home,
tasks, documents, inbox and settings are native screens that talk to the Looty API; the
collaborative editor and the screens that are not native yet open in a web screen (system
WebView) without the website's menus. The APK has no Google dependencies (no Play Services,
no Firebase). Built with compile/target SDK 37 (Android 17), minimum Android 8.0.

## Features

- **Server setup** on first launch (and from the launcher shortcut "Change server").
  `http://` is only accepted for local servers (`localhost`, `*.local`, `*.lan`, emulator).
- **Sign-in in the system browser** (RFC 8252), so passkeys/WebAuthn and the provider's own
  pages work. Tapping a provider opens a browser tab; once signed in, the browser returns to
  the app with `looty://sso` and a one-time code that the app exchanges with its PKCE secret.
- Only your server opens in the app. Other links (and `target=_blank`) open in the browser.
- File uploads (system picker), downloads (to *Downloads*, including files generated in the
  page such as exports), `looty://w/…` links ("Open in the app"), the back button, and status
  bar colors that follow the page's light or dark theme.
- An "offline" page with *Try again* / *Change server* when the server can't be reached.

Native screens (since 0.2.0, boards and tasks since 0.3.0): **Boards** (swipe between columns, cards, long-press to move / complete / delete with undo), **Task details** (title, status, priority, dates, assignees, labels, description, sub-tasks, attachments, comments), **Home** (stats, today's events, upcoming tasks, boards),
**Tasks** (mine to do / all pending, check off with undo, pull to refresh), **Documents**
(tree, favorites, search, new document), **Inbox** (notifications, mark as read) and **More**
(workspaces, boards, other screens, server, sign out); **New task** bottom sheet.

**Notifications (0.5.0)**, without Google: *More → Notifications*. Instant push through
UnifiedPush (ntfy, Gotify… the server sends Web Push to the endpoint your distributor app gives
the app) or, as a fallback, a periodic check every ~15 minutes (WorkManager). Tapping a
notification opens the matching native screen.

**Moodle (0.5.1)**: the “Sign in” button opens the official portal in a dedicated screen that catches the `moodlemobile://token=…` redirect, so the key never has to be copied by hand.

**Roles (0.5.2)**: the app reads your rights (`access/me`) and hides or disables what the role does not allow (create, edit, move, delete, complete, comment).

**Reading view (0.6.0)**: documents open in a native, read-only view (HTML saved by the editor parsed with jsoup into Compose blocks); the pencil opens the web editor.

**Sharing and widget (0.7.0)**: “Share” → Looty creates a task or a document from a link or text; a “Looty · To do” home-screen widget (Jetpack Glance) lists your tasks.

**Files (0.8.0)**: sharing images, PDFs or other files creates a task with the files attached (10 files, 25 MB each).

**Offline (0.9.0)**: every list is also saved on the device (`files/offline/`, per server, 80 MB cap), the app opens offline on the saved session, and writes go through a persisted outbox (`files/outbox.json`) replayed in order when the server answers; task/page creation uses client-chosen ids so replays are idempotent. Signing out clears both.

**Offline, part 2 (0.10.0)**: boards and favourite/recent documents (with images) are prefetched; files shared offline are queued as `UPLOAD` entries (`files/outbox-files/`); a WorkManager job (`OutboxWorker`, network-constrained) replays the queue even when the app is closed — app and job share one `Outbox` instance and one lock.

**Offline web (0.11.0)**: `offline/WebCache.kt` is a network-first OkHttp cache behind the WebView (`shouldInterceptRequest`): GET responses of the server are saved; when it is unreachable they (and the HTML shell for any route) are served. `/asset-manifest.json` (a Nitro route listing `/_nuxt/`) lets the app prefetch the whole UI. The doc editor's IndexedDB Yjs persistence is enabled in the native app (`isNativeApp`), so offline edits merge on reconnect.

**Updates (0.12.0)**: `update/Updater.kt` gets the download site's address from the server's `/api/releases` (`site`, optional; without it the app falls back to the project's GitHub release `android-v<version>`, using the SHA-256 digest from the GitHub API), reads `<site>/latest.json`, downloads the APK it names (it must live under the site), checks its SHA-256 and installs it with `PackageInstaller` (Android refuses an APK signed with another key). https only (http to 10.0.2.2/localhost in debug builds). `UpdateWorker` checks daily and notifies. Debug builds honour an `updateOverride` preference (`{"version":…,"apk":…}`) for testing.

Next: nothing planned.

## Install

Download `looty-<version>.apk` from the GitHub releases (tags `android-v*`) and open it on
the phone (allow installing from your browser or file manager). Android 8.0+.

## Build

```sh
nix develop .#android          # SDK 36–37, JDK 17, Gradle 9, emulator
cd android
gradle assembleDirectDebug     # app/build/outputs/apk/direct/debug/app-direct-debug.apk
gradle assembleQa              # like the release (R8) but debuggable, for end-to-end tests
adb install -r app/build/outputs/apk/direct/debug/app-direct-debug.apk
```

Outside Nix: `./gradlew assembleDebug` with `ANDROID_HOME` pointing to an SDK.

The debug build (`io.github.untypequicode.looty.debug`) installs next to the release build
and enables WebView debugging (`chrome://inspect`).

### Release signing

Create a key once and keep it safe (every update must be signed with the same key):

```sh
keytool -genkeypair -keystore looty-release.jks -alias looty -keyalg RSA -keysize 4096 -validity 10000
```

Then `android/keystore.properties` (ignored by git):

```properties
storeFile=/path/to/looty-release.jks
storePassword=…
keyAlias=looty
keyPassword=…
```

`gradle assembleDirectRelease` then produces `app/build/outputs/apk/direct/release/app-direct-release.apk`
(about 1 MB). In CI, the same values come from the `ANDROID_KEYSTORE_B64`,
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD` secrets; a new
`android` entry in `backend/releases.json` merged into `main` publishes the APK (see [RELEASING.md](../RELEASING.md)).

### Two builds: `direct` and `fdroid`

`direct` is the APK published on GitHub and the download site, with the built-in updater. `fdroid` has no update code and
not the `REQUEST_INSTALL_PACKAGES` / `UPDATE_PACKAGES_WITHOUT_USER_ACTION` permissions (sources in `app/src/fdroid` and
`app/src/direct`); its *App updates* screen only shows the release notes. It is published in Looty's own F-Droid repository
(see [ADR 0016](../docs/adr/0016-android-fdroid-variant-and-repository.md)).

```sh
gradle -Plooty.unsigned assembleFdroidRelease   # app/build/outputs/apk/fdroid/release/*.apk, unsigned and reproducible
node ../scripts/fastlane-metadata.mjs           # store metadata, from backend/releases.json
```

### Emulator

```sh
echo no | avdmanager create avd -n looty35 -k "system-images;android-35;default;x86_64"
emulator -avd looty35
```

From the emulator, your computer is `10.0.2.2` (for example `10.0.2.2:3000`).

**Calls (0.32.0)**: the microphone and camera work in Looty calls (`WebChromeClient.onPermissionRequest`, only for the app's instance origin; runtime `RECORD_AUDIO`/`CAMERA`). A foreground service (`call/CallService`, types microphone|camera) keeps the call alive with the screen off, with a *Hang up* / *Speaker·Earpiece* notification; audio goes to a headset if present (`call/CallAudio`, `MODE_IN_COMMUNICATION`). The page signals call start/stop through `LootyApp.postMessage({type:'call'…})`; `window.__lootyCaps.screenShare = false` tells the web to hide screen sharing (not available in the Android WebView). Same in both flavors. See `docs/adr/0019-calls-webrtc-mesh.md`.
