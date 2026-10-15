# Looty desktop (Linux and Windows)

Native client for a Looty server, built with [Tauri 2](https://tauri.app): a Rust core and
the system web engine (WebKitGTK on Linux, WebView2 on Windows; no bundled browser, ~12 MB binary).
Linux: NixOS and Debian. Windows: 10 and 11.

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

### Updates (Debian / Ubuntu)

The `.deb` also installs Looty's signed apt repository (`/etc/apt/sources.list.d/looty.sources` and its key in
`/usr/share/keyrings/`). After the first manual install, updates arrive like any other package:

```sh
sudo apt update && sudo apt upgrade          # or let unattended-upgrades do it
```

To receive them automatically with *unattended-upgrades*, add `"Looty:stable";` to `Unattended-Upgrade::Allowed-Origins`.
Delete the `looty.sources` file to opt out. Nix users update with their flake (`nix flake update looty`, or
`nix profile upgrade`; the flake needs read access to the repository). Windows has no auto-update yet.

**Maintainer setup (once)**: the apt repository is published on the Looty download site (see `site/README.md`).
Run `desktop/scripts/apt-key.sh` (creates the signing key; it uses `nix shell nixpkgs#gnupg` if `gpg` is missing), commit the
public key `desktop/packaging/looty-archive-keyring.gpg`, add the secrets `APT_GPG_PRIVATE_KEY` (and `APT_GPG_PASSPHRASE` if
the key has one), the repository variable `SITE_URL` and the secret `SITE_UPLOAD_TOKEN`, and set `"site"` in
`backend/releases.json`. Every `desktop-v*` release then publishes the signed repository at `<site>/apt`
(`desktop/scripts/apt-publish.sh`, which keeps only the latest version). Without a site, the repository is published on
GitHub Pages instead (`desktop/scripts/apt-publish-pages.sh`, branch `gh-pages`: enable Pages once in the repository
settings) at `https://<owner>.github.io/<repo>/apt`. Without the key, only the plain `.deb` is published and the package
does not add the repository. Adding the repository by hand (replace `SITE` by your site address):

```sh
sudo install -d /etc/apt/keyrings
curl -fsSL SITE/apt/looty-archive-keyring.asc | sudo tee /etc/apt/keyrings/looty.asc >/dev/null
echo "deb [signed-by=/etc/apt/keyrings/looty.asc] SITE/apt stable main" | sudo tee /etc/apt/sources.list.d/looty.list
sudo apt update && sudo apt install looty
```

### Windows 10 / 11

Download `Looty_<version>_x64-setup.exe` from the GitHub releases (tags `desktop-v*`) and run it.
It installs for your user only (no administrator rights) and registers the `looty://` links.
Windows SmartScreen may say the publisher is unknown, because the installer is not signed yet:
*More info* → *Run anyway*. WebView2 (the web engine) is part of Windows 10/11; the installer
downloads it if it is missing.

What works the same as on Linux: sign-in in your browser (passkeys included), tray icon with the
unread badge and today's tasks, notifications (click to open, *Mark as done* button), tabs,
offline mode, *Launch at login*, `looty://` links, quick add, local LaTeX compilation (needs a
TeX distribution such as MiKTeX on your PATH), and the global shortcuts **Ctrl+Alt+N** (new task)
and **Ctrl+Alt+L** (open Looty). Downloads open the standard *Save as* window (same tray toggle as on Linux), and a red
dot on the taskbar icon shows unread notifications. Spell checking is Windows's own, and the tray
icon may be hidden in the "^" overflow of the taskbar until you drag it out. Build it yourself with
`cargo tauri build --bundles nsis` in `desktop/src-tauri` (Windows, Rust and the Tauri CLI needed).

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

## Tabs

In the app, tabs use the same shortcuts as a browser: **Ctrl+T** new tab, **Ctrl+W** close,
**Ctrl+Shift+T** reopen the last closed tab, **Ctrl+Tab** / **Ctrl+Shift+Tab** next and
previous, **Ctrl+1…9** go to a tab, **Ctrl+PageUp/PageDown**. Ctrl+click or middle-click a
link to open it in a new tab. (On the website these are Alt+T, Alt+W… because the browser
keeps Ctrl+T and Ctrl+W for itself.)

## Shortcuts and command line

**Global shortcuts.** On desktops that support the XDG *GlobalShortcuts* portal (GNOME 48+,
KDE Plasma, Hyprland with xdg-desktop-portal-hyprland), Looty registers **Ctrl+Alt+N**
(new task) and **Ctrl+Alt+L** (open Looty) by itself. Your desktop may ask you to confirm
once, and lists them in its keyboard settings, where you can change the keys. They work
from any application, on Wayland and X11. To turn this off, set `global_shortcuts = false`
in `~/.config/looty/config.toml`.

**Without portal support**, bind a key in your desktop to `looty --quick-add` (it opens the
quick-add window, starting Looty if needed):

- **GNOME**: Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts → `+`,
  command `looty --quick-add`.
- **KDE Plasma**: System Settings → Keyboard → Shortcuts → Add New → Command or Script,
  command `looty --quick-add`.
- **Hyprland**: in `hyprland.conf`: `bind = SUPER, N, exec, looty --quick-add`.

**Command line** (also usable from launchers such as rofi, scripts, or a bar button):

| Command | What it does |
|---------|--------------|
| `looty --quick-add` | Quick-add window |
| `looty --new-task "Buy milk"` | Quick-add window with the title filled in |
| `looty --inbox` | Open your inbox |
| `looty --my-tasks` | Open your tasks |
| `looty --open /w/<workspace>/doc/<page>` | Open a page (see *Links*) |

The launcher's right-click menu (GNOME, KDE) has *New task*, *Inbox* and *My tasks*.

Due-date and assignment notifications have a **Mark as done** button next to *Open*.

## Links

`looty://` links open a page in the app, e.g. `looty://w/<workspace>/doc/<page>`. On the
website, *Open in the desktop app* is in the menu of every document and task. From a
terminal: `looty --open /w/<workspace>/doc/<page>`. Only Looty pages can be opened this
way (a link can never sign you out or call the API).

The `.deb` registers the `looty://` scheme automatically. With Nix, if links don't open:
`xdg-mime default looty.desktop x-scheme-handler/looty`.

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

- The session (server cookies) is held by the Rust process, not by the web view, so the
  tray and notifications can work while the window is closed. It is encrypted
  (`~/.local/share/looty/session.enc`, AES-256-GCM) with a key kept in the system keyring
  (GNOME Keyring, KWallet, or any Secret Service). Without a keyring (e.g. a bare
  Hyprland setup) it falls back to `session.json`, readable by you only.
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
3. **Offline**: cached interface and data, offline editing (wiki merge, queued task
   changes), sync status.
4. **Polish** (this version): `looty://` links, quick-add shortcut, keyring-encrypted
   session, automated packaging.

## Releases

`./desktop/scripts/release.sh 0.2.0` sets the version; add the matching entry to
`backend/releases.json` and merge into `main`: the *Release* workflow builds the `.deb` (tested on a clean
Debian 12) and the Windows installer, publishes the `desktop-v0.2.0` GitHub release, and the *Nix cache*
workflow builds the Nix package. See [RELEASING.md](../RELEASING.md).
