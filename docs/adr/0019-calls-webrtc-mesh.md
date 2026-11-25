# Voice (and later video) calls: WebRTC peer-to-peer mesh, signaling on the collaboration server

Issues #261 (this ADR), #262 (signaling), #263 (voice MVP) and #264 (STUN/TURN), under #269. Goal: talk while working together, with **almost no cost on the server** and **no perceptible latency**.

Decision:
- **Media never goes through Looty.** Browsers connect to each other with WebRTC in a **full mesh** (each participant has one peer connection to every other one). The server only relays *signaling* (a few kilobytes per call: join, leave, offer, answer, ICE candidates, mute state, participant list). No transcoding, no mixing, no recording: CPU, RAM and bandwidth of the instance stay flat.
- **Target: 2 to 6 people** per call. The cap is `LOOTY_CALL_MAX_PARTICIPANTS` (default 6, admin setting planned in #268). In a mesh every person uploads their audio N-1 times; at 32 kbit/s Opus this is trivial for voice up to ~6 people, but it would not hold for video of larger groups.
- **Audio**: Opus tuned for voice by SDP munging (`useinbandfec=1`, `usedtx=1`, mono, 32 kbit/s average) and `RTCRtpSender.setParameters` (bitrate cap, high priority). Echo cancellation, noise suppression and automatic gain control are requested from the browser in `getUserMedia`.
- **ICE**: STUN by default (`stun:stun.l.google.com:19302`, override with `LOOTY_STUN_URLS`), served to the browser by `GET /api/calls/config`. **TURN is optional and off by default** (#264): with `LOOTY_TURN_URLS` and `LOOTY_TURN_SECRET` set, the same endpoint adds a TURN server with a **temporary login per signed-in person** (coturn REST-API style: username `expiry:userId`, password `base64(HMAC-SHA1(secret, username))`, valid `LOOTY_TURN_TTL` seconds, 24 h by default). The shared secret never reaches the browser. The installers (`install.sh --turn`, the compose template, the NixOS module) can run coturn next to Looty; coturn refuses relaying to private address ranges, so it cannot be used to reach the instance's internal network. Relayed audio does go through the TURN server (still not decoded or stored): about 32 kbit/s per direction per relayed participant, so a small relay port range (49160-49200) is enough for a small instance. The doctor checks the settings and the TCP port.
- **A call belongs to a context** (a page, a project or a workspace) and **survives navigation**: the call state lives at module level in the web app and the call bar is mounted once in `app.vue`.

## Signaling transport: Hocuspocus stateless messages (not a new WebSocket endpoint)

Two options were considered.

1. **A dedicated authenticated WebSocket in the Rust backend (axum).** Full control, but it needs a second room registry, a second reconnection/revalidation story in the browser, a second proxy route (Traefik, Caddy, nginx, the Nuxt relay, the desktop app proxy) and cookie authentication code that already exists elsewhere.
2. **The existing collaboration server (Hocuspocus).** Already routed everywhere (`/collab`), already authenticates with the session cookie or API token by asking the backend, already revalidates rights (every 120 s and on `NOTIFY looty_access`), already handles reconnection, pings, proxy idle timeouts and refusal rate limiting. Hocuspocus has *stateless messages* (opaque strings, no document change) and a per-connection `onStateless` hook, which is exactly a signaling channel.

**Chosen: option 2.** A call room is a Hocuspocus document named `call:<page|project|workspace>:<uuid>` that never loads or stores anything (the Database extension returns nothing and skips storing; the connection is read-only). The room logic is a small pure class (`collaboration/calls.js`, in memory only, unit tested without a server). The cost is one empty in-memory document per room that has a connection, removed by Hocuspocus when the last connection leaves.

Rights: `GET /api/calls/{kind}/{id}/access` (backend, `calls.rs`) applies **the same rights as the content**: reading the page, the project or the workspace is enough to hear and speak. The collaboration server calls it on connection and on every revalidation. There is no write right in a call, so nobody is "read-only".

## Behaviour that matters

- **Presence before joining.** Every connection to a room is an *observer* and receives the participant list; "join" makes it a participant. Pages and projects open an observer connection while displayed (and release it when the page is kept in memory by tabs), so the button can show "Join call · 2".
- **Revalidation never drops a call.** Media is peer-to-peer, so losing the signaling WebSocket (rights revalidation closing it with 4403, network change, proxy timeout) does not stop audio. A participant keeps their place for 15 s after the connection closes and takes it back by joining with the same peer id (chosen by the client, stable for the call). If rights were really removed, reconnection is refused and the place expires; the audio connections then fail on their own when the remaining participants drop the peer.
- **Limits.** 32 KB per message (a voice SDP is ~2 KB), a token bucket per connection (burst 120, 60 messages/s: ICE candidates arrive in bursts), participant cap per room. The sender of a signal is always deduced from the connection, never from the message, and a peer id cannot be taken over by another account.
- **No glare.** For each pair the smaller peer id sends the offer; the other waits. After an ICE failure the same side retries.
- **Logging.** Refusals use the existing collaboration refusal logs (reason, status, document); calls log full rooms, rate limits, oversize messages and peer departures.

## Alternatives not taken

- **SFU (LiveKit, mediasoup, Janus).** The right answer above ~6 people or for video-heavy use, and it handles network changes better. But it is a new service to host, with UDP ports, a constant CPU/bandwidth cost per call, and TURN/TLS configuration; it breaks "very light on the server" for the common case (small groups working together). Left as a separate, optional self-hosted issue if mesh limits are reached; the signaling room and the `useCall` composable are the only parts that would change.
- **MCU (server mixing).** Highest server cost (decode, mix, encode), added latency, no benefit for voice.
- **Third-party hosted calls (Jitsi, Whereby links).** Leaves Looty, no context, no access control tied to the content.

## Limits and costs

- Mesh upload: `(N-1) × 32 kbit/s` per participant for voice; video (#265) will make this the limiting factor, so video will have a lower cap.
- Without TURN, people behind symmetric NAT or restrictive firewalls cannot connect to each other. STUN alone works for most home and campus networks.
- Public STUN (default) shows participants' IP addresses to that STUN server; self-hosters can point `LOOTY_STUN_URLS` to their own.
- Instance cost: one JSON message of ~2 KB per offer/answer and ~200 bytes per ICE candidate, once per pair. Memory per room is a few hundred bytes per participant plus an empty Yjs document; no CPU at idle (no timers except the 15 s grace after a disconnect).

## Latency budget and measurement

Target: **under 150 ms mouth-to-ear** on a normal network. Budget: capture and encode ~20 ms (Opus 20 ms frames, `minptime=10`), network one way (direct path) 10 to 60 ms, jitter buffer 20 to 60 ms, playout 10 to 20 ms. Server is not on the path, so the instance's load does not change it.

Measurement: the e2e test `e2e/tests/voice-call.mjs` joins two browsers with fake microphones and checks that audio bytes are received on both sides, then reads the round-trip time of the selected ICE candidate pair (`currentRoundTripTime`) and fails above 150 ms; the value is printed in the test output. Real one-way latency adds the jitter buffer and codec delay; `inbound-rtp.jitterBufferDelay / jitterBufferEmittedCount` can be read from `getStats()` for the same estimate in the field.

## Addendum: Android app and desktop app (#267)

The call itself stays in the web page (the WebView runs the same `useCall`); the native shells only provide what the web engine cannot: permissions, staying alive in the background, audio routing.

**Android** (`android/.../call/`, same code in the `direct` and `fdroid` flavors):
- `WebChromeClient.onPermissionRequest` grants `AUDIO_CAPTURE`/`VIDEO_CAPTURE` only when the request origin equals the instance the app is configured for (`CallPolicy.need`; the same rule as the `LootyApp` message listener, so with several instances (ADR 0017) only the active instance's page can use the microphone and camera). Any other resource (MIDI, protected media) is denied. Missing `RECORD_AUDIO`/`CAMERA` are asked at runtime at the first call, not at install.
- **Bridge**: web to native with the existing `LootyApp.postMessage`: `{type:'call', active, title, muted, video}` (sent by `plugins/native-call.client.ts`, which watches `callState`) and `{type:'callSpeaker', on}`; native to web with a `looty:call-hangup` DOM event (the notification's *Hang up* button; the plugin calls `leave()`). No new endpoint, nothing sent to the server.
- **Foreground service** `CallService` (type `microphone` and `camera` when granted; `FOREGROUND_SERVICE_*` permissions) with an ongoing, silent notification (*Hang up*, *Speaker/Earpiece*), a partial wake lock (6 h cap) and `AudioManager.MODE_IN_COMMUNICATION`. It is started by the visible activity (Android 14 forbids starting a microphone service from the background) and stopped when the page reports the end of the call or the web screen is finished. The call lives in the web screen: leaving it with the back gesture ends the call; the home gesture or screen-off does not.
- **Audio routing** (`CallAudio`): headset (wired, USB, Bluetooth SCO/LE, hearing aid) if present, else speaker (default, hands-free) or earpiece; `setCommunicationDevice` on Android 12+, `setSpeakerphoneOn`/Bluetooth SCO before.
- **Screen sharing**: `getDisplayMedia` does not exist in the Android WebView. The page script exposes `window.__lootyCaps = { screenShare: false }` before load; the web reads it with `canShareScreen()` (`utils/nativeCaps.ts`) and hides the share button.

**Desktop (Tauri)**: on Linux, WebKitGTK denies `getUserMedia` unless the application answers the `permission-request` signal, and `enable-media-stream` is off by default. The app turns both on and allows microphone/camera only when the page is the app's own local proxy origin (`enable_media` in `main.rs`); display capture and everything else is denied. On Windows, WebView2 shows its own permission prompt (nothing to do). **Screen sharing is not supported in the desktop app** (WebKitGTK needs the xdg-desktop-portal ScreenCast flow, not wired; WebView2 needs a picker); the capability flag is only set by the Android app for now, so on desktop the share button may be shown and fail: the browser remains the way to share a screen.

Not verified on a physical device: Bluetooth routing, Android 14/15 foreground-service restrictions on vendor ROMs, and calls with the screen off for a long time (battery optimisation by the vendor can still kill the process).
