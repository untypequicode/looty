---
title: "Flathub and winget"
description: "Install the Looty desktop app from Flathub on Linux or with winget on Windows."
---

The desktop app is packaged for two package managers. Both install the same app as the downloads on the [website](/#download).

:::note
These listings are being set up and may not be available yet. Until they are, use the `.deb`, Nix or the Windows installer from [Install the apps](/docs/use/apps/).
:::

## Linux: Flathub

```sh
flatpak install flathub io.github.untypequicode.looty
flatpak run io.github.untypequicode.looty
```

- Flathub updates the app: `flatpak update`. The app's own update check is not used.
- Permissions are minimal: network (to reach your server), notifications, the tray icon and the system keyring. Downloads and *Save as* go through the system file dialog, so Looty cannot read your other folders.
- The global quick-add shortcut and the `looty://` links depend on your desktop environment.

## Windows: winget

```powershell
winget install untypequicode.Looty
winget upgrade untypequicode.Looty
```

- The installer is the same per-user installer as on the website. It is not signed yet, so Windows SmartScreen may warn about an unknown publisher; winget shows the installer's hash and checks it before running it.
- `winget upgrade` installs new versions.

## Packagers

The manifests are built from this repository. The maintainer steps (submission to Flathub and to the winget catalog) are in the [packaging runbook](https://github.com/untypequicode/looty/blob/main/docs/packaging-flathub-winget.md).
