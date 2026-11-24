---
title: "NixOS"
description: "Run Looty on NixOS with the services.looty module: secrets in a file, local PostgreSQL, Caddy or nginx, optional bundled Pocket ID."
---

The Looty flake provides `nixosModules.default`. It runs the same release images as the Docker Compose install (server, web interface, live editing), as systemd units through Podman, with a local PostgreSQL, a reverse proxy configured through the usual NixOS options (HTTPS certificates included) and a daily database dump.

## Configuration

```nix
{
  inputs.looty.url = "github:untypequicode/looty";
  # …
  outputs = { nixpkgs, looty, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        looty.nixosModules.default
        {
          services.looty = {
            enable = true;
            domain = "looty.example.org";
            instanceName = "My association";
            # auth = "oidc" (your provider, default) or "bundled-oidc" (Pocket ID on auth.<domain>)
            auth = "bundled-oidc";
            version = "0.44.1"; # pin it; "latest" by default
            environmentFile = "/run/secrets/looty.env"; # agenix, sops-nix, or a file you write
          };
        }
      ];
    };
  };
}
```

Ports 80 and 443 are opened for the proxy. Options: `proxy` (`caddy` default, `nginx`, or `none` to route the local ports yourself), `database.createLocally` (`false`: put your own `DATABASE_URL` in the environment file), `ports.*`, `backup.enable`, `openFirewall`. With `none` or your own proxy, follow the routing table in the [detailed install](/docs/host/install/).

## Secrets

The environment file holds `KEY=value` lines and **must not be in the Nix store** (a path as a string such as `/run/secrets/…`, not a path literal). Generate the values on the host:

```sh
cat > looty.env <<ENV
DB_PASSWORD=$(openssl rand -hex 16)
LOOTY_SECRET_KEY=$(openssl rand -base64 32)
# bundled-oidc:
OIDC_CLIENT_SECRET=$(openssl rand -hex 32)
OIDC_PROVIDER_ID=$(uuidgen)
POCKET_ID_ENCRYPTION_KEY=$(openssl rand -hex 32)
POCKET_ID_STATIC_API_KEY=$(openssl rand -hex 32)
# oidc (your provider) instead:
# OIDC_ISSUER=https://idp.example.org
# OIDC_CLIENT_ID=looty
# OIDC_CLIENT_SECRET=…
ENV
```

`DB_PASSWORD` is letters and digits only. Any other [Looty variable](/docs/reference/environment/) (`SMTP_*`…) can be added to the same file. With agenix or sops-nix, point `environmentFile` at the decrypted path.

## Bundled Pocket ID

With `auth = "bundled-oidc"`, the module enables `services.pocket-id` from nixpkgs on `https://auth.<domain>` (a sub-domain, with its own DNS record), and a one-shot unit creates the Looty client in it with the same script as the Compose install. After the first start, create the first Pocket ID account at `https://auth.<domain>/setup`: **that page stays open to anyone until the first account exists**. It then becomes administrator, and Looty's first account is Looty's administrator.

## Checks

`nix flake check` starts two virtual machines: one with Caddy and the local database (password, secrets file, routing of `/`, `/api` and `/collab`), one with the real Pocket ID and the OIDC client creation. The three Looty images are replaced there by small stand-ins (the virtual machine has no network); the real server and live editing are exercised by the Compose stack tests in CI.

## Local accounts

`auth = "local"` is reserved for the local-accounts mode and is refused until that mode exists.
