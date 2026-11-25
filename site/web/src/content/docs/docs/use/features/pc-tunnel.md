---
title: "Use my PC as a Jupyter or R server"
description: "Connect your own computer to a Looty project with one outgoing connection, to run Jupyter, Python and R notebooks on it: prerequisites, step by step, commands, security and troubleshooting."
---

A **PC tunnel** lets Looty use **your own computer** as the Jupyter server of a project, **for as long as the tunnel is up**. A small program, `looty-bridge` (the *agent*), runs on your PC and opens **one outgoing connection** to your Looty instance. You open no port and change nothing on your router or firewall, and it works behind NAT or a home box.

It is meant for heavy needs the browser cannot cover: a lot of memory, a GPU, your own data, every package of your machine.

## WebR and Pyodide, or your PC?

| | In the browser (Pyodide, WebR) | Your PC (tunnel) |
| --- | --- | --- |
| Install | Nothing | Docker or Nix (or a downloaded binary) |
| Available | Always, even offline once loaded | Only while the agent runs on your PC |
| Packages | Those built for WebAssembly | All packages of your Jupyter |
| Power | Your browser's memory and CPU | Your PC's memory, CPU, GPU |
| Interrupt a calculation | R: restart only | Real interruption |
| Who can run code | Each person, in their own browser | Everyone who can edit the project, **on your PC** |

Start with the browser engines (see [Projects and notebooks](/docs/use/features/projects-notebooks/)). Use your PC when they are not enough.

:::note
The administrator must have turned PC tunnels on (they are off by default). If you see no **Use my PC** button, ask them: [hosting side](/docs/host/operate/tunnels/). The feature is not available in the public demo.
:::

## Prerequisites

- A project you can **edit**, in a Looty instance where tunnels are enabled.
- On the PC, **one** of: Docker with Docker Compose (recommended, everything is included), Nix, or the `looty-bridge` binary plus a Jupyter server you already run.
- The PC must be able to reach your instance over HTTPS (an HTTP proxy that only allows plain TCP is not supported yet: the agent connects directly).
- The PC must stay on, and the agent running, while you work.

## Step by step

1. In the notebook, open the **Engine** dialog, pick **Jupyter server**, then **Use my PC**.
2. Choose the service (Jupyter with Python, R or other kernels) and the address of Jupyter on your PC (`http://127.0.0.1:8888` by default). Read the warning (see Security below) and tick the confirmation.
3. Looty creates the **tunnel token** (`lty_tun_...`) and shows it **once**, together with the line to run on your PC, with your instance address already filled in. Copy it. If you lose the token, revoke the tunnel and create another.
4. Run the line on your PC (below). The dialog shows the live status: waiting for the agent, then connected (machine name, latency).
5. The kernels of your Jupyter appear like those of any Jupyter server: pick one (for example Python 3 or `ir` for R) and run your cells.
6. To stop, press Ctrl+C on the PC, or use **Disconnect and revoke** in Looty, which cuts the connection immediately. All your tunnels are listed, with a revoke button, in **Settings, PC tunnels**.

## The commands

The token can be given with `LOOTY_TOKEN` (preferred, it stays out of your shell history) or `--token`.

### Docker Compose (Jupyter with Python and R included)

From a checkout of the repository, create a `.env` file:

```sh
LOOTY_INSTANCE=https://looty.example.org
LOOTY_TOKEN=lty_tun_...
JUPYTER_TOKEN=some-long-random-string
```

then start the agent and a Jupyter with Python and R kernels (published on `127.0.0.1:8888` only):

```sh
docker compose -f docker-compose.bridge.yml --profile jupyter up
```

In Looty, give the project the same `JUPYTER_TOKEN` (stored encrypted, never shown again). Without `--profile jupyter` only the agent starts, for a Jupyter you already run: set `BRIDGE_SERVICES=jupyter=http://host.docker.internal:8888`.

### Nix

```sh
export LOOTY_TOKEN=lty_tun_...
nix run github:untypequicode/looty#bridge -- --instance https://looty.example.org --service jupyter=http://127.0.0.1:8888
```

### Binary

Download `looty-bridge-<version>-<os>-<arch>` (Linux x86_64 and aarch64, macOS, Windows) from the releases tagged `bridge-v*` on [GitHub](https://github.com/untypequicode/looty/releases), then:

```sh
LOOTY_TOKEN=lty_tun_... ./looty-bridge --instance https://looty.example.org --service jupyter=http://127.0.0.1:8888
```

To start your own Jupyter first (loopback only, with a token):

```sh
jupyter lab --no-browser --ServerApp.ip=127.0.0.1 --IdentityProvider.token=<token>
```

### Options

| Option | Environment | Meaning |
| --- | --- | --- |
| `--instance URL` | `LOOTY_INSTANCE` | Instance address (`https://...`). |
| `--token T` | `LOOTY_TOKEN` | Tunnel token. |
| `--service NAME=URL` | `LOOTY_SERVICES` (comma separated) | Local service to expose, repeatable. `NAME:KIND=URL` sets the kind (`jupyter`, `latex`, `http`). Only `http://` local addresses. |

The agent reconnects by itself after a network loss (waiting 1 s, doubling up to 60 s). It stops for good when the token is revoked, expired or used by another agent.

## Which kernels, and R

The tunnel carries whatever your Jupyter offers. With the Docker Compose file you get Python 3 and R (IRkernel, `ir`, with the usual R packages of the `r-notebook` image). With your own Jupyter, install the kernels you want (`IRkernel` for R, Julia, and so on). R notebooks on a PC run with real interruption, SVG plots and kernel completion, and the [R panels](/docs/use/features/projects-notebooks/) work the same.

LaTeX: the `latex` kind of service is reserved for a LaTeX compile service on your PC. It is not available yet; the Docker Compose `latex` profile only prepares a TeX Live container.

## Security and privacy

**Connecting a Jupyter to a project lets everyone who can edit that project run code on your PC**, with the rights of the user running Jupyter, each in their own kernel. Looty asks you to confirm this when you create the tunnel.

- **Only declared services are reachable.** The instance can only talk to the services your agent declares, at the address **you** configured on the PC. It cannot reach another port of your PC or anything on your network, and it can do nothing when the agent is off.
- **No inbound port.** The agent connects out to the instance; nothing listens on your PC for the tunnel.
- **Owner only.** You can restrict a tunnel to yourself: other members of the project then cannot use it, and notebooks tell them so. You can change it at any time.
- **Tokens.** The tunnel token is tied to you and one space, expires (30 days by default), is stored hashed on the instance, shown once, and only works for the tunnel connection: it cannot call the rest of the Looty API. Revoking closes the connection at once. Keep it secret, like a password.
- **Audit.** The instance administrator can see when tunnels were created, attached, connected, disconnected and revoked. Tokens never appear in it.
- **The browser never sees your PC.** Only the Looty server talks to the tunnel, as with any Jupyter server of a project. Your Jupyter's own token is stored encrypted and never displayed.
- **Your PC is trusted by the project.** Output from a stolen or forged agent could look like real results: revoke a token you think leaked.

Good practice: attach the tunnel to a project only you can edit (for example in your personal space) or tick *Owner only*; run Jupyter in a container or as an unprivileged user; bind it to `127.0.0.1`; keep Jupyter's token on.

## Limits

- 16 simultaneous requests or kernel connections per tunnel (the administrator can lower it), and an optional bandwidth cap.
- Requests up to 16 MiB, responses up to 64 MiB, kernel messages up to 8 MiB.
- A computation that sends nothing for 30 minutes on its kernel connection is cut; an HTTP request with no data for 60 seconds is cancelled.
- One agent per token: starting a second one with the same token stops the first.
- If the PC goes offline, notebooks show a clear message and nothing is rerouted silently. Relaunch the agent, or switch the notebook back to the browser engine in the meantime. The work already saved in the notebook is not lost.

## Troubleshooting

| What you see | What to do |
| --- | --- |
| No **Use my PC** button | Tunnels are off on this instance (ask the administrator) or the demo is used. |
| Agent exits at once, "revoked" or "expired" | The token is no longer valid. Create a new tunnel in Looty. |
| Agent keeps retrying, instance replies 404 | PC tunnels are turned off in Administration. |
| Agent says "replaced" and stops | Another agent started with the same token. Keep only one. |
| Status stays "waiting for the agent" | Check the instance address (`https://`, no path), the token, and that the PC can reach the instance. A proxy that cuts idle WebSockets after a short delay also causes this: tell your administrator ([hosting](/docs/host/operate/tunnels/)). |
| Connected, but no kernels listed | The Jupyter address is wrong or Jupyter is not running. With Docker use `jupyter=http://jupyter:8888` (the Compose default) or `host.docker.internal` for a Jupyter on the host. |
| Jupyter refuses (403 or 401) | The Jupyter token given in Looty does not match the one of your Jupyter (`JUPYTER_TOKEN`). |
| "Tunnel is busy" | Too many simultaneous requests on the tunnel; wait or ask the administrator to raise the limit. |
| Kind not allowed | The administrator restricted the kinds of service allowed. |

For the protocol and the design, see the repository: `docs/tunnel-protocol.md`, `bridge/README.md` and the architecture decision `docs/adr/0020-pc-tunnel.md`.
