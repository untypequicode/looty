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

Next phases: native reading view, widget and sharing,
offline cache.

## Install

Download `looty-<version>.apk` from the GitHub releases (tags `android-v*`) and open it on
the phone (allow installing from your browser or file manager). Android 8.0+.

## Build

```sh
nix develop .#android          # SDK 36–37, JDK 17, Gradle 9, emulator
cd android
gradle assembleDebug           # app/build/outputs/apk/debug/app-debug.apk
gradle assembleQa              # like the release (R8) but debuggable, for end-to-end tests
adb install -r app/build/outputs/apk/debug/app-debug.apk
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

`gradle assembleRelease` then produces `app/build/outputs/apk/release/app-release.apk`
(about 1 MB). In CI, the same values come from the `ANDROID_KEYSTORE_B64`,
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD` secrets; pushing
a tag `android-v<versionName>` publishes the APK.

### Emulator

```sh
echo no | avdmanager create avd -n looty35 -k "system-images;android-35;default;x86_64"
emulator -avd looty35
```

From the emulator, your computer is `10.0.2.2` (for example `10.0.2.2:3000`).
