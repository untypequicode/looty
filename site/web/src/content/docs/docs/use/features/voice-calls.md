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
- **Who is speaking** is shown by a ring around the avatar.
- **Settings** (gear): choose the microphone and set the volume of each person. If the microphone is unplugged, Looty switches to the default one or tells you to pick another; you keep listening.
- Echo cancellation, noise suppression and automatic gain are on. Audio is Opus, tuned for voice, with silence suppression.

## On Android and desktop

- **Android app**: the first call asks for the microphone (and the camera for video); only your own Looty instance can use them. During a call a notification stays visible with **Hang up** and **Speaker / Earpiece**, so you can lock the phone or open another app and keep talking. Wired, USB and Bluetooth headsets are used automatically, otherwise the speaker. Same in the direct and F-Droid builds. If you refused the permission, allow *Microphone* in Android settings, *Apps*, *Looty*, *Permissions*.
- Leaving the web screen with the back gesture hangs up (the call lives in that screen); use the home gesture to put the app aside. Some phone makers kill background apps to save battery: set Looty to "unrestricted" battery use if calls cut off.
- **Screen sharing is not available in the Android app** (the system web view cannot do it): the button is hidden. Use the browser.
- **Desktop app (Linux)**: the microphone and camera are allowed for Looty's own window (nothing else). **Screen sharing is not supported in the desktop app yet**; use the browser to share a screen. Windows asks for the microphone in its own prompt.

## Limits

- Up to **6 people** per call by default (the administrator can change it with `LOOTY_CALL_MAX_PARTICIPANTS`).
- Same rights as the content: whoever can open the page, project or workspace can join its call, and nobody else.
- On very restrictive networks (some corporate or school firewalls) two people may not manage to connect directly. An administrator can enable a relay server for that case (see below).
- Latency is that of a direct network path: typically well under 150 ms. The automated test reports the measured round-trip time of its two browsers.

For the technical choices see the architecture decision `docs/adr/0019-calls-webrtc-mesh.md`.

## Hosting: relay (TURN) for restrictive networks

Direct connections work on most home and campus networks. On very restrictive ones (symmetric NAT, firewalls that block UDP) a relay is needed. It is **off by default**.

- **Docker (install.sh)**: `install.sh --turn` (or `--upgrade --turn` on an existing install) adds a `coturn` service and a `TURN_SECRET` in `.env`. Looty then offers each signed-in person a temporary relay login (valid 24 hours, `LOOTY_TURN_TTL`); the secret never leaves the server.
- **NixOS**: `services.looty.turn = { enable = true; secretFile = "/run/secrets/looty-turn"; };` and put the same value in the environment file as `LOOTY_TURN_SECRET`.
- **Ports to open** on the server firewall: **3478 UDP and TCP**, and the relay range **49160-49200 UDP** (`TURN_MIN_PORT` / `TURN_MAX_PORT`). Keep the range small: each relayed participant uses a few ports, about 20 ports are plenty for a small instance. Behind NAT (cloud servers), set `TURN_EXTERNAL_IP` to the public address.
- **Bandwidth**: relayed audio costs about 32 kbit/s per direction per relayed participant on your server. A 6-person call where everyone is relayed is under 1 Mbit/s. Only people who cannot connect directly use it.
- **Check**: Administration > Diagnostic has a "Call relay (TURN)" line. Use your own STUN servers with `LOOTY_STUN_URLS` if you do not want the public one.
