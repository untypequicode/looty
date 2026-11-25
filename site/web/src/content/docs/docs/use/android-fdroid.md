---
title: "Android: the Looty F-Droid repository"
description: "Install and update Looty on Android with F-Droid or Droid-ify, from the Looty repository."
---

Looty publishes its own [F-Droid](https://f-droid.org) repository. Add it once in F-Droid, Droid-ify or Neo Store, and Looty then installs and updates like any other app. This build has **no built-in updater** and does not ask for the permission to install apps: your F-Droid client does the updating. It is otherwise the same app as the APK on the [download page](/#download), with the same identifier and the same signing key, so you can switch from one to the other without losing your data.

Looty is not in the main F-Droid catalogue yet. Until it is, use this repository.

## Add the repository

On your phone, open this link, or scan the QR code with the F-Droid client:

[fdroidrepos://looty.untypequicode.fr/fdroid/repo](fdroidrepos://looty.untypequicode.fr/fdroid/repo)

![QR code of the Looty F-Droid repository](/fdroid/repo/index.png)

If you type the address yourself, it is `https://looty.untypequicode.fr/fdroid/repo`.

## Check the key

F-Droid shows the repository fingerprint when you add it. It must be the one published on this site, the SHA-256 fingerprint of the repository's signing key:

`98AB977C06DF41102B93943862203BDFABF197A589B68955A3138882FB6D218C`

[Fingerprint of the repository key](/fdroid/fingerprint.txt)

The QR code above already contains it, so scanning it is the safest way to add the repository. If the fingerprint differs, do not add the repository.

## Updates

Only the latest version is kept in the repository. In F-Droid, pull to refresh, or enable automatic updates for the repository. Looty's *App updates* screen then only shows what's new in each version.

## Build it yourself

The F-Droid build is `./gradlew assembleFdroidRelease` in the `android/` folder of the [source code](https://github.com/untypequicode/looty). It has no proprietary dependency, and two builds of the same version give the same unsigned APK.
