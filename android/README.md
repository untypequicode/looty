# Looty for Android

Native Kotlin shell around the Looty web app: the system WebView (Chromium) displays your
Looty server, and the Android side handles everything the web view can't do on its own.
The APK has no Google dependencies (no Play Services, no Firebase).

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

Next phases: notifications (UnifiedPush/ntfy, with a periodic check as fallback), quick add,
and offline reading.

## Install

Download `looty-<version>.apk` from the GitHub releases (tags `android-v*`) and open it on
the phone (allow installing from your browser or file manager). Android 8.0+.

## Build

```sh
nix develop .#android          # SDK 35, JDK 17, Gradle, emulator
cd android
gradle assembleDebug           # app/build/outputs/apk/debug/app-debug.apk
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
