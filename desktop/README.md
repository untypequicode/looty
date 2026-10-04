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

Enter your server's address (e.g. `looty.example.com`) and sign in. SSO sign-in
happens in your default browser, where passkeys, WebAuthn and security keys work
(the app's web engine, WebKitGTK, doesn't support them); the browser then hands the
session back to the app (RFC 8252: one-time code + PKCE over the local address) and the
window comes back logged in. *Sign in in this window instead* keeps the whole sign-in in
the app, and is used automatically with servers that predate this feature.
The session is kept by the app between launches. To switch servers:
tray icon → *Change server…*.

## Tray menu and notifications

The app keeps working in the background when the window is closed:

- **System notifications** for Looty notifications (assignments, mentions, comments,
  due dates…), shown by your desktop (GNOME, KDE, or mako/dunst/swaync on Hyprland).
  They follow the *Push* choices in Looty → Settings → Notifications, and stay quiet
  while the Looty window is focused. Click one to open the task or page.
- **Tray icon** with a red badge when you have unread notifications, and a menu with:
  your inbox, your overdue tasks and tasks due today (each with *Open* and *Mark as
  done*), *New task…* (a small quick-add window), *All my tasks*, and toggles for
  notifications and *Launch at login* (starts hidden in the tray).
- **Downloads** (exports, attachments) go to your Downloads folder, with a
  notification to open the folder.

## Offline mode

When the server can't be reached, Looty keeps working with a banner at the top
(*Offline — showing saved data*):

- **Reading**: everything you have opened (boards, tasks, wiki pages, inbox, images)
  is kept in a private local copy (`~/.cache/looty/`, readable by you only) and shown
  from there, even after restarting the app offline.
- **Tasks**: create (also from the tray's quick add), edit, move, check off, delete,
  labels, assignees and comments work offline. Changes are queued, applied to the
  local copy, and replayed in order as soon as the server is back (*Syncing…*, then
  a notification). A change the server refuses (e.g. on a task deleted meanwhile) is
  reported by name.
- **Wiki**: pages you have opened on this device are stored locally with their full
  collaborative history: edit them offline and they merge automatically with other
  people's changes when you reconnect. A page never opened on this device is shown
  read-only until you are back online.
- Not available offline: uploads, settings, and creating boards or columns.

Signing out clears the local copy.

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

1. **Shell**: window, server setup, login (SSO), basic tray, single
   instance, Nix flake, `.deb`.
2. **System integration**: native notifications, tray with my tasks and
   check-off, quick task add, unread badge, launch at login, downloads.
3. **Offline** (this version): cached interface and data, offline editing (wiki merge,
   queued task changes), sync status.
4. **Polish**: `looty://` links, global shortcut, keyring, automated packaging.
