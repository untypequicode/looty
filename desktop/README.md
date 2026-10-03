# Looty desktop (Linux)

Native client for a Looty server, built with [Tauri 2](https://tauri.app): a Rust core and
the system WebKitGTK engine (no bundled browser, ~12 MB binary). Linux only: NixOS and Debian.

## Install

### NixOS / Nix (flake)

```sh
# Try it
nix run github:untypequicode/looty

# Install in your profile
nix profile install github:untypequicode/looty#looty-desktop
```

In a NixOS configuration:

```nix
# flake.nix inputs
inputs.looty.url = "github:untypequicode/looty";

# configuration.nix
environment.systemPackages = [ inputs.looty.packages.${pkgs.system}.looty-desktop ];
```

### Debian / Ubuntu (.deb)

```sh
./desktop/scripts/build-deb.sh          # needs Docker; output in desktop/dist/
sudo apt install ./desktop/dist/Looty_0.1.0_amd64.deb
```

The package targets Debian 12 (bookworm) and newer, and Ubuntu 22.04+.

## First launch

Enter your server's address (e.g. `looty.example.com`) and sign in as on the web,
SSO included. The session is kept by the app between launches. To switch servers:
tray icon → *Change server…*.

## Tray icon by desktop environment

| Desktop  | Tray icon |
|----------|-----------|
| KDE Plasma | Works out of the box. |
| Hyprland | Needs a bar with a tray, e.g. Waybar with the `tray` module. |
| GNOME    | Needs the *AppIndicator and KStatusNotifierItem Support* extension (`gnome-shell-extension-appindicator` on Debian; `gnomeExtensions.appindicator` on NixOS). |

Closing the window hides it in the tray. Launching Looty again (menu, launcher,
`looty`) brings the window back; *Quit* in the tray menu exits.
To quit on close instead, set `close_to_tray = false` in `~/.config/looty/config.toml`.

## How it works

The window loads `http://127.0.0.1:<random port>`, served by a proxy inside the app
that forwards the interface, API, files and WebSockets (realtime, collaborative editing)
to your server.

- The session (server cookies) is held by the Rust process in
  `~/.local/share/looty/session.json` (mode 0600), not by the web view, so the tray
  and notifications can work while the window is closed.
- Only the app's window can use the proxy: requests must carry the right `Host`
  and an access cookie set at startup from a random secret, which only exists in the
  app's own web view profile,
  so other local programs and websites get `403`.
- The interface comes from your server, so the app always matches the deployed
  version of Looty.
- Links to other sites open in your default browser. During an SSO sign-in, every page
  of the chain stays in the app (e.g. Zitadel → GitHub → Zitadel): identity providers
  tie the sign-in to the window that started it.

Files: `~/.config/looty/config.toml` (server, preferences),
`~/.local/share/looty/` (session), `~/.local/share/app.looty.desktop/` (web view data).

## Development

```sh
nix develop .#desktop
cd desktop/src-tauri
cargo run
```

Debug builds accept `LOOTY_START_PATH=/some/path` to open a page at startup (ignored in
release builds). Logs: `RUST_LOG=looty=debug`.

### NVIDIA

With the proprietary NVIDIA driver, WebKitGTK may show a blank window; the app sets
`WEBKIT_DISABLE_DMABUF_RENDERER=1` automatically when `/proc/driver/nvidia` exists.

## Roadmap

1. **Shell** (this version): window, server setup, login (SSO), basic tray, single
   instance, Nix flake, `.deb`.
2. **System integration**: native notifications, tray with my tasks and quick
   check-off, quick task add, unread badge, autostart.
3. **Offline**: cached interface and data, offline editing (wiki merge, queued task
   changes), sync status.
4. **Polish**: `looty://` links, global shortcut, keyring, automated packaging.
