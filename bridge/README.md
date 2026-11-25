# looty-bridge

Connects your PC to a Looty instance so a project can use it as its Jupyter (Python, R...) or LaTeX server while the tunnel is up. The bridge opens **one outgoing HTTPS/WebSocket connection** to the instance: no port to open, no firewall change. It speaks tunnel protocol v1 ([`docs/tunnel-protocol.md`](../docs/tunnel-protocol.md), design in [ADR 0020](../docs/adr/0020-pc-tunnel.md)).

Step-by-step guide, troubleshooting and hosting notes: [Use my PC as a Jupyter or R server](https://looty.untypequicode.fr/docs/use/features/pc-tunnel/) and [PC tunnels for hosts](https://looty.untypequicode.fr/docs/host/operate/tunnels/).

Security in short:

- Connecting a Jupyter gives the **editors of the project code execution on that Jupyter** (and with the rights of the user running it). Keep Jupyter's token on and bound to `127.0.0.1`, or use the container below.
- The bridge only serves the services you declare with `--service`, at the address you give. It never builds an address from what the instance sends.
- TLS is strict (built-in Mozilla roots, no way to disable it). Plain `http://` instances are only accepted on loopback, for tests.
- The tunnel token is never printed. Prefer `LOOTY_TOKEN` to `--token` so it stays out of your shell history.

## 1. In Looty

Create a tunnel token for your project (it is shown once, `lty_tun_...`). It expires and can be revoked.

## 2. Run it

Pick one.

### Docker Compose (Jupyter with Python and R included)

From a checkout of the repository:

```sh
cat > .env <<'ENV'
LOOTY_INSTANCE=https://looty.example.org
LOOTY_TOKEN=lty_tun_...
JUPYTER_TOKEN=some-long-random-string
ENV
docker compose -f docker-compose.bridge.yml --profile jupyter up
```

In Looty, give the project the same `JUPYTER_TOKEN`. Services are `jupyter` (Jupyter with Python and R kernels, published on `127.0.0.1:8888` only). Without `--profile jupyter` only the bridge starts, for a Jupyter you already run: set `BRIDGE_SERVICES=jupyter=http://host.docker.internal:8888`. The `latex` profile starts a TeX Live container reserved for the LaTeX compile service (not available yet).

### Nix

```sh
export LOOTY_TOKEN=lty_tun_...
nix run github:untypequicode/looty#bridge -- --instance https://looty.example.org --service jupyter=http://127.0.0.1:8888
```

### Binary

Download `looty-bridge-<version>-<os>-<arch>` (Linux x86_64 and aarch64, macOS, Windows) from the releases tagged `bridge-v*`, then:

```sh
LOOTY_TOKEN=lty_tun_... ./looty-bridge --instance https://looty.example.org --service jupyter=http://127.0.0.1:8888
```

To start your own Jupyter first: `jupyter lab --no-browser --ServerApp.ip=127.0.0.1 --IdentityProvider.token=<token>`.

## Options

| Option | Environment | Meaning |
| --- | --- | --- |
| `--instance URL` | `LOOTY_INSTANCE` | Instance address (`https://...`). |
| `--token T` | `LOOTY_TOKEN` | Tunnel token. `--token-env NAME` reads another variable. |
| `--service NAME=URL` | `LOOTY_SERVICES` (comma separated) | Local service to expose, repeatable. `NAME:KIND=URL` sets the kind (`jupyter`, `latex`, `http`); by default `jupyter` is `jupyter`, `latex` and `tex` are `latex`, others `http`. Only `http://` local URLs. |

Behaviour: reconnects with exponential backoff (1 s to 60 s, with jitter); stops for good when the token is revoked, expired or used by another agent (exit code 1); Ctrl+C closes cleanly. Status lines go to stderr.

Not done yet: `HTTPS_PROXY` support (direct connection only) and an all-in-one `--spawn-jupyter` (use the Compose file).

## Development

```sh
cd bridge
cargo test    # unit tests and an integration test against a fake relay
```

Releases: push a tag `bridge-v<version>` (binaries and the `ghcr.io/untypequicode/looty-bridge` image are built by `.github/workflows/bridge.yml`).
