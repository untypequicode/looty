---
title: "Voice calls"
description: "Talk with the people working on the same page, project or workspace, directly from the browser."
---

A **voice call** lets the people who can open a page or a project talk while they work. Audio goes **directly between participants** (WebRTC): the Looty server only introduces the browsers to each other and never hears or stores anything.

## Start or join a call

- Click **Start call** in the header of a page or a project. If a call is already running, the button reads **Join call · 2** with the number of people in it, and shows their names on hover.
- Your browser asks for the microphone the first time. If you refuse, the call bar explains how to allow it again.
- The call bar floats at the bottom of the screen and **stays while you change page**: keep talking while you open a board or another document. Use **Leave** (or close the tab) to hang up.

## In the call

- **Mute**: the microphone button, or `Ctrl`+`Alt`+`M` (`Cmd`+`Alt`+`M` on Mac). Others see who is muted.
- **Push to talk** (optional, off by default): in the call settings, tick *Push to talk*. The microphone stays muted until you **hold a key** (the backtick key by default; *Change key* picks another). Releasing the key, or leaving the window, mutes you again. The key is ignored while you type in a text field and with `Ctrl`, `Alt` or `Cmd` held. The choice is kept on this device.
- **Who is speaking** is shown by a ring around the avatar.
- **Settings** (gear): choose the microphone and set the volume of each person. If the microphone is unplugged, Looty switches to the default one or tells you to pick another; you keep listening.
- Echo cancellation, noise suppression and automatic gain are on. Audio is Opus, tuned for voice, with silence suppression.

## On Android and desktop

- **Android app**: the first call asks for the microphone (and the camera for video); only your own Looty instance can use them. During a call a notification stays visible with **Hang up** and **Speaker / Earpiece**, so you can lock the phone or open another app and keep talking. Wired, USB and Bluetooth headsets are used automatically, otherwise the speaker. Same in the direct and F-Droid builds. If you refused the permission, allow *Microphone* in Android settings, *Apps*, *Looty*, *Permissions*.
- Leaving the web screen with the back gesture hangs up (the call lives in that screen); use the home gesture to put the app aside. Some phone makers kill background apps to save battery: set Looty to "unrestricted" battery use if calls cut off.
- **Screen sharing is not available in the Android app** (the system web view cannot do it): the button is hidden. Use the browser.
- **Desktop app (Linux)**: the microphone and camera are allowed for Looty's own window (nothing else). **Screen sharing is not supported in the desktop app yet**; use the browser to share a screen. Windows asks for the microphone in its own prompt.

## Camera and screen sharing

- **Camera**: the camera button turns your camera on or off; the call settings let you pick the device. You see your own preview and a thumbnail of everyone with a camera on, and the person speaking is highlighted.
- **Share your screen**: the monitor button lets you share the whole screen, a window or a browser tab. System sound is shared when your browser allows it (Chrome and Edge, for a tab or a whole screen). Everyone sees the shared screen large, with **who is sharing**; **Stop sharing** is one click, in the call bar or under the preview, and closing the browser's own "stop sharing" bar works too.
- **Full screen** and **picture in picture** are on every video (hover it).
- The screen is sent sharp but with few frames per second (5 to 15), which is what text needs; cameras are 640x360 at 24 fps for two people and lighter as more people join.
- **Audio keeps priority**: when the network is weak the video drops quality first, and Looty lowers the video bitrate when it measures packet loss or high latency.
- **Cameras are limited to 4 people per call**. In a mesh every camera is sent once per other participant, which would not hold beyond that on a home connection. If a fifth person joins, cameras are turned off with a message; screen sharing and voice stay available up to the call limit (6 by default). For bigger groups a media server (SFU) would be needed.

## Connection quality and recovery

- A colored dot in the call bar shows the worst link: **good** (latency under 150 ms, loss under 3 %, jitter under 30 ms), **fair** (up to 300 ms, 8 % loss, 60 ms jitter) or **poor** beyond. **Call settings > Connection details** shows latency, loss, jitter and bitrate for each participant.
- After a network change (Wi-Fi to mobile data, cable unplugged) or a drop, each link restarts by itself (ICE restart) with growing delays (1 s, 2 s, 4 s ... up to 30 s) and is rebuilt from scratch if it keeps failing. You stay in the call.
- If the connection to the Looty server is lost, voice and video **continue between participants**; a notice appears and the call is resynchronised when the server is back. Session revalidations never end a call.
- Measured on the automated test with two browsers: round-trip time under 150 ms required (usually a few milliseconds on a local network).

## Limits

- Up to **6 people** per call by default. The administrator changes it (2 to 12) in Administration > Instance > Calls. When a call is full, joining shows a clear message.
- The administrator can also **turn calls off**, or allow only voice (no camera) or no screen sharing. The buttons then disappear or explain why; the limits are enforced by the server, not only hidden in the interface.
- Same rights as the content: whoever can open the page, project or workspace can join its call, and nobody else.
- On very restrictive networks (some corporate or school firewalls) two people may not manage to connect directly. An administrator can enable a relay server for that case (see below).
- Latency is that of a direct network path: typically well under 150 ms. The automated test reports the measured round-trip time of its two browsers.

For the technical choices see the architecture decision `docs/adr/0019-calls-webrtc-mesh.md`.

## Privacy

- **Audio, video and shared screens go directly from browser to browser** (WebRTC, encrypted with DTLS-SRTP). They do not go through the Looty server, which only relays the connection setup. **Nothing is recorded or stored**, by Looty or by anyone else.
- In a call every participant connects to every other one, so **participants can see each other's IP addresses** (a normal consequence of a direct connection). If that is a concern, ask the administrator to enable the relay (TURN): then media goes through the relay server of your instance and addresses are not exchanged with peers.
- Without a relay, the public STUN server (Google by default, `LOOTY_STUN_URLS` to change it) learns that your browser asks for its public address, never the content of the call.
- The server knows who is in a call (names shown in the join button, same rights as the content) and logs only counters such as "room full", never the content.

## Administration settings

Administration > Instance > **Calls**:

| Setting | Effect | Default |
|---|---|---|
| Enable calls | Off: nobody can start or join a call, the buttons disappear. | on |
| Maximum people per call | 2 to 12. Beyond it, joining is refused with "This call is full". | 6 (`LOOTY_CALL_MAX_PARTICIPANTS` gives the initial value) |
| Allow cameras | Off: voice only, the camera button disappears and any camera stream is ignored by the server. | on |
| Allow screen sharing | Off: the share button disappears and shares are ignored by the server. | on |

Changes apply to new connections at once and to people already connected within two minutes. Camera calls stay limited to 4 people whatever the maximum.

## Hosting: relay (TURN) for restrictive networks

Direct connections work on most home and campus networks. On very restrictive ones (symmetric NAT, firewalls that block UDP) a relay is needed. It is **off by default**.

- **Docker (install.sh)**: `install.sh --turn` (or `--upgrade --turn` on an existing install) adds a `coturn` service and a `TURN_SECRET` in `.env`. Looty then offers each signed-in person a temporary relay login (valid 24 hours, `LOOTY_TURN_TTL`); the secret never leaves the server.
- **NixOS**: `services.looty.turn = { enable = true; secretFile = "/run/secrets/looty-turn"; };` and put the same value in the environment file as `LOOTY_TURN_SECRET`.
- **Ports to open** on the server firewall: **3478 UDP and TCP**, and the relay range **49160-49200 UDP** (`TURN_MIN_PORT` / `TURN_MAX_PORT`). Keep the range small: each relayed participant uses a few ports, about 20 ports are plenty for a small instance. Behind NAT (cloud servers), set `TURN_EXTERNAL_IP` to the public address.
- **Bandwidth**: relayed audio costs about 32 kbit/s per direction per relayed participant on your server. A 6-person call where everyone is relayed is under 1 Mbit/s. Only people who cannot connect directly use it.
- **Check**: Administration > Diagnostic has a "Call relay (TURN)" line. Use your own STUN servers with `LOOTY_STUN_URLS` if you do not want the public one.
