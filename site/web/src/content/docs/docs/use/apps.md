---
title: "Install the apps"
description: "Looty on Android, Linux, Windows, iPhone and iPad, and how to work offline."
---

All downloads are on the [Looty website](/#download). Each app connects to your instance and keeps your boards, documents and calendar on the device.

## Android

- Download the APK from the website (Android 8 or later) and allow installing from this source the first time. The app has no Google services and updates itself from within the app. Prefer F-Droid? Add the [Looty F-Droid repository](/docs/use/android-fdroid/): that build is updated by F-Droid.
- **Links open in the app**: a `looty://` link or a link to your instance offers to open in the app.
- **Widgets** on the home screen, and **sharing**: share text or a file from any app to Looty.
- Notifications use UnifiedPush (see [notifications](/docs/use/features/notifications/)).
- To see your Looty calendars in the phone's Calendar app, see [calendar](/docs/use/features/calendar/).

## Linux

- Debian and Ubuntu: install the `.deb`; it also adds the signed repository, so `sudo apt update && sudo apt upgrade` keeps Looty current.
- NixOS and Nix: `nix run github:untypequicode/looty`.
- Flatpak: see [Flathub and winget](/docs/use/flathub-winget/).

## Windows

Windows 10 and 11. The installer is for your user only. Windows may warn about an unknown publisher: *More info*, then *Run anyway*. There is no automatic update yet. With winget: see [Flathub and winget](/docs/use/flathub-winget/).

## iPhone and iPad

There is no App Store app. Open your instance in Safari, then *Share > Add to Home Screen*: Looty opens full screen and can send notifications.

## Offline

The Android and desktop apps keep your data on the device. You can read and edit while disconnected; documents merge when you reconnect. Moodle files can also be kept for offline use (see [Moodle](/docs/use/features/moodle/)).
