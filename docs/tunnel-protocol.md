# Looty tunnel protocol, version 1

This document specifies the protocol between a Looty **instance** (relay, `backend/src/tunnel_relay.rs`) and an **agent** (`looty-bridge`, running on a person's PC). It is written so that the agent can be implemented independently. Design rationale and threat model: `docs/adr/0020-pc-tunnel.md`.

Words MUST, SHOULD and MAY are used in the RFC 2119 sense. "Instance" and "agent" are the two peers; the agent always connects, but **only the instance opens streams**.

## 1. Transport and authentication

- The agent opens a WebSocket (RFC 6455) to `wss://<instance>/api/tunnels/agent` (`ws://` only for local tests).
- Upgrade request headers:
  - `Authorization: Bearer lty_tun_<token>` (the tunnel token, shown once when the tunnel is created). Required.
  - `Sec-WebSocket-Protocol: looty-tunnel.v1`. The agent SHOULD offer it; the instance selects it.
- Refusals happen **before** the upgrade, as HTTP responses with a JSON body `{"error": "<code>"}`:
  - `401 tunnel_token_invalid` (unknown token, wrong format, suspended account), `401 tunnel_token_revoked`, `401 tunnel_token_expired`;
  - `404 not_found` when tunnels are disabled on the instance (`LOOTY_TUNNELS` is not `on`);
  - `500 internal_error`.
  The agent MUST NOT retry on 401 without operator action (a revoked or expired token never works again). On network errors, 404 and 5xx it retries with exponential backoff (1 s doubling to 60 s, with jitter) and resets the delay after a connection that lasted more than 60 s.
- One connection per token: a new authenticated connection replaces the old one, which is closed with code 4009 `replaced`. An agent that sees 4009 MUST NOT reconnect (another agent is using the token).
- All TCP/TLS/proxy handling is the WebSocket client's business; the agent SHOULD honour `HTTPS_PROXY`.

## 2. Frames

Everything after the upgrade uses two WebSocket frame types. Any other data frame type is not used. WebSocket ping/pong frames are allowed and ignored (they only keep proxies happy); the protocol has its own `ping`/`pong` messages (section 6).

- **Text frame**: one UTF-8 JSON object, the **control message**. The field `t` names its type. Unknown `t` or unparsable JSON is a protocol violation (section 9).
- **Binary frame**: one **data frame**.

### 2.1 Data frame

```
 0        4   5
 +--------+---+------------------+
 | id u32 | f | payload (0..64KiB)|
 +--------+---+------------------+
```

- `id`: stream id, unsigned 32 bit, big endian.
- `f`: flags byte. Bit 0 (`0x01`) `TEXT`: the message is text (WebSocket streams only). Bit 1 (`0x02`) `MORE`: the message continues in the next data frame of the same stream. All other bits MUST be zero.
- `payload`: 0 to 65536 bytes (`max_frame`). The whole WebSocket message (header + payload) is therefore at most 65541 bytes; peers MUST refuse bigger messages.
- For **HTTP streams**, data frames carry the body as a plain byte sequence; flags MUST be 0 and the frame boundaries mean nothing.
- For **WebSocket streams**, a message is one data frame, or several: every frame but the last has `MORE`; `TEXT` is set on the first frame of a message (and SHOULD be set on all of them). The receiver concatenates until a frame without `MORE`. A reassembled message is at most `max_message` bytes (8 MiB).

## 3. Handshake

1. After the upgrade, the agent MUST send `hello` within **10 seconds** (as its first message); otherwise the instance closes with code 4400.
2. The instance answers `welcome`. No stream is opened before `welcome` is sent.

```json
{"t":"hello","v":1,"agent":"looty-bridge/0.1.0 linux-x86_64",
 "services":[{"name":"jupyter","kind":"jupyter"},{"name":"tex","kind":"latex"}]}
```

```json
{"t":"welcome","v":1,"ping_interval":15,
 "limits":{"max_streams":16,"max_frame":65536,"initial_window":262144,
           "max_header_bytes":16384,"max_message":8388608,"max_services":16}}
```

- `v`: protocol version, an integer. If the instance does not support the agent's version it replies `{"t":"bye","code":"unsupported_version","message":"supported: 1"}` and closes with code 4426. Versions only change for incompatible changes; within version 1 no message or field is added or removed (unknown fields in a known message are ignored).
- `agent`: free text for logs and the status page (control characters removed, 64 characters kept).
- `services`: the **declared services** (section 7). May be empty.
- `welcome.limits` are the limits the agent MUST respect; they may change between instances, so the agent MUST read them and not hard-code them. `ping_interval` is in seconds.

## 4. Streams

A stream is one HTTP exchange or one WebSocket connection to a local service. Stream ids are chosen by the instance: unsigned 32 bit integers, starting at 1 and increasing, never reused during a connection.

### 4.1 `open` (instance to agent)

```json
{"t":"open","id":7,"service":"jupyter","proto":"http","method":"POST",
 "path":"/api/kernels?x=1","headers":[["authorization","token abc"],["content-type","application/json"]]}
```

- `service`: a name the agent declared. The agent MUST refuse (with `error`) any `open` for a name it does not serve, and MUST connect only to the **local address configured by its operator for that name**. It MUST NOT build an address from `service`, `path` or `headers`.
- `proto`: `"http"` or `"ws"`.
- `method`: upper-case HTTP method (`GET` for `ws`).
- `path`: path and query, always starting with `/`, never `//`, never containing `..`, control characters, spaces or `\`, at most 2048 characters. The agent appends it to the local base URL of the service and SHOULD validate these rules again.
- `headers`: list of `[name, value]` pairs (lower-case names), at most 64 pairs and `max_header_bytes` in total. The agent MUST set `Host` itself to the local address, MUST drop hop-by-hop headers (`connection`, `upgrade`, `keep-alive`, `transfer-encoding`, `te`, `trailer`, `proxy-*`) and for `ws` MUST generate its own `Sec-WebSocket-Key`, version and (if needed) `Origin` matching the local address.

### 4.2 HTTP streams

1. The instance sends `open`, then the request body as data frames (none for an empty body), then `end`.
2. The agent sends `head`, then the response body as data frames, then `end`. The agent MAY send `head` before it has received the whole request.

```json
{"t":"head","id":7,"status":201,"headers":[["content-type","application/json"]]}
```

`status` is 100 to 599 (the agent does not forward 1xx interim responses; it does not follow redirects, it returns 3xx as they are). Hop-by-hop response headers MUST be dropped; `content-length` and `transfer-encoding` are not needed (the body ends at `end`). The instance reads at most 64 MiB of response and sends at most 16 MiB of request body.

### 4.3 WebSocket streams

1. The instance sends `open` with `proto: "ws"`.
2. The agent connects to the local WebSocket. If accepted it sends `head` with `status: 101` (and the negotiated headers, which the instance ignores). If refused it sends `head` with the HTTP status the local server answered (401, 403, 404...) followed by `end`, or `error` if it could not connect at all.
3. Messages flow both ways as data frames (section 2.1), text or binary.
4. Either side closes with `end`: the receiver of `end` closes its WebSocket side normally (code 1000) and sends `end` in return if it has not yet. If the local WebSocket closes or fails, the agent sends `end` (close) or `error` (failure).

### 4.4 `end` and `error`

```json
{"t":"end","id":7}
{"t":"error","id":7,"code":"unreachable","message":"connection refused"}
```

- `end`: the sender will send nothing more on this stream (half-close). Data frames after `end` from the same side are ignored. A stream is finished when both sides have sent `end`, or when either side sent `error`.
- `error`: terminates the stream immediately in both directions; frames still in flight are ignored. `code` is a short identifier (`[A-Za-z0-9_]`, at most 48 characters). `message` is optional text for logs, never shown as is to users.
- Codes the **agent** SHOULD use: `unknown_service`, `unreachable` (cannot connect locally), `refused` (local server refused the connection), `timeout`, `protocol` (the local server broke HTTP or WebSocket), `too_large`, `internal`.
- Codes the **instance** sends: `cancelled` (the instance gave up: client left, timeout, kernel closed), `flow_control` (credit violation), `bad_head` (invalid `head`), `tunnel_closed` (internal, never on the wire).
- An `error` or `end` for an unknown id (already finished) is ignored.

## 5. Flow control

Each direction of each stream has a **credit window** measured in bytes of data-frame payload, so that a slow consumer never makes a peer buffer without bound.

- When a stream opens, each side may send `initial_window` bytes (from `welcome.limits`, 256 KiB) to the other.
- Sending a data frame uses `max(len(payload), 1)` bytes of credit (an empty frame costs 1). A sender MUST NOT send a frame when it does not have enough credit: it waits for credit, and gives up with `error` `timeout` after 60 s without any.
- A receiver gives credit back after it has **consumed** the bytes (delivered them to the local socket or to the application), with:

```json
{"t":"window","id":7,"add":65536}
```

  `add` is at least 1. It SHOULD be sent per frame consumed or coalesced, at least once the consumed bytes reach half the window. The total credit of a sender never exceeds `initial_window`: a `window` that would push it above is a violation and the stream is closed with `error` `flow_control`.
- A receiver that holds more unconsumed bytes than `initial_window` for a stream has been overrun: the instance closes the stream with `error` `flow_control`; the agent SHOULD do the same.
- The window applies to data frames only. Control messages are not limited by credit (but see section 8).

## 6. Keep-alive and latency

```json
{"t":"ping","n":42}
{"t":"pong","n":42}
```

- Either side may send `ping`; the other MUST answer with `pong` carrying the same `n` as soon as possible.
- The instance pings every `ping_interval` seconds (15) and measures the round trip: it is the **latency** shown in the interface. The agent SHOULD NOT need to ping, but MAY.
- If the instance receives nothing (any frame) for **45 seconds**, it closes the connection with code 4008 `timeout`. The agent SHOULD close and reconnect if it receives nothing for 45 seconds.

## 7. Services

- A service is `{"name": ..., "kind": ...}`. `name`: 1 to 32 characters among `a-z`, `0-9`, `-`, `_`; unique within the list. `kind`: `jupyter` (Jupyter server REST and kernel WebSocket: the instance sends the Jupyter API paths, `/api/...`), `latex` (compile service, defined by issue #275), `http` (generic HTTP service). At most `max_services` (16) services.
- The instance can open streams **only** to a declared `(name, kind)`; a project that targets a service that is not declared (or whose kind does not match) gets an explicit error.
- The agent may change the list at any time (a service starts or stops) with the full list:

```json
{"t":"services","services":[{"name":"jupyter","kind":"jupyter"}]}
```

  Streams already open keep running. An invalid list is a protocol violation.
- The local address of a service (`http://127.0.0.1:8888`, for example) is known only to the agent and is never sent to the instance.
- The Jupyter token of the PC's server, when there is one, is given by the project owner in Looty (stored encrypted); the instance sends it in the `authorization: token ...` header of `open`. An agent MAY also inject its own.

## 8. Limits (version 1 values)

| Limit | Value | When exceeded |
| --- | --- | --- |
| Concurrent streams per tunnel | 16 | instance: stream not opened (`jupyter_tunnel_busy`) |
| Data frame payload | 65536 bytes | violation, connection closed (1002) |
| Credit window per stream and direction | 262144 bytes | stream closed `flow_control` |
| Headers (open and head) | 64 pairs, 16 KiB | open refused / stream closed `bad_head` |
| Path | 2048 characters | open refused |
| WebSocket message | 8 MiB | stream failed |
| HTTP request body / response body | 16 MiB / 64 MiB | request refused / stream failed |
| Declared services | 16 | violation |
| `hello` delay | 10 s | close 4400 |
| First byte (`head`) of a response | 30 s | stream cancelled |
| HTTP idle (no data) | 60 s | stream cancelled |
| WebSocket idle (no message) | 30 min | stream failed |
| Silence on the connection | 45 s | close 4008 |

The agent SHOULD apply its own timeouts to local connections (connect 5 s) and answer `error` `timeout` rather than leave a stream hanging. The agent SHOULD NOT let more than the window per stream be buffered in memory.

## 9. Closing and errors

- **Protocol violation** by the agent (unparsable message, unknown `t`, `hello` twice, `open` sent by the agent, bad frame size or flags, invalid `services`): the instance sends `bye` and closes with code **1002**, reason being a short identifier (`bad_message`, `bad_frame`, `bad_services`, `unexpected_message`).
- `bye` (instance to agent, just before closing): `{"t":"bye","code":"revoked"}` with codes `revoked` (close 4001: the token was revoked), `expired` (4002), `replaced` (4009), `timeout` (4008), `unsupported_version` (4426), or a violation identifier (1002). The agent MUST NOT retry on `revoked` and `expired` and SHOULD log the reason.
- When the connection closes, in either direction, all streams are dead: the instance fails the pending requests with a "tunnel offline" error, and the agent MUST close every local connection it opened for these streams.
- Close code summary: 1000 normal, 1002 protocol violation, 4001 revoked, 4002 expired, 4008 timeout, 4009 replaced, 4400 bad or missing hello, 4426 unsupported version.

## 10. Status and logging (instance side, informative)

`GET /api/tunnels/{id}/status` (owner of the token) and the `tunnel` object of `GET /api/projects/{id}/jupyter` give `connected`, `latency_ms` (last ping round trip), `since_secs`, `agent`, declared `services`, open `streams` and `bytes_in` / `bytes_out`. The instance logs connection, disconnection (with duration and byte counts), refusals and violations with the tunnel id, never the token.

## 11. Minimal agent loop (informative)

1. Connect with the token; send `hello` with the services configured on the PC; wait for `welcome`.
2. For each text message: `ping` answers `pong`; `open` allocates stream state (`credit_out = initial_window`, `unconsumed = 0`) and starts a task that connects locally to the service address plus `path`; `end`, `error`, `window` update the stream; `bye` is logged.
3. For each binary frame: find the stream by id, push the payload to the local connection (HTTP request body or WebSocket message, reassembling on `MORE`), and send `window` once written.
4. Local to instance: for HTTP, `head`, data frames (wait for credit), `end`; for WebSocket, `head` 101, one message per frame group, `end` on close.
5. On connection loss, drop everything local and reconnect with backoff.
