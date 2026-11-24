---
title: "Quick install"
description: "Get a Looty instance running with the release files and a .env file."
---

:::note
The browser **install generator** (zip) is planned (see the design decision *install generator runs in the browser*). The command below does the same on your machine: it generates the secrets there and never takes them as arguments.
:::

## One command

```sh
curl -fsSL https://looty.untypequicode.fr/install.sh | sh -s -- --domain looty.example.org --proxy caddy --name "My association" --auth bundled-oidc
```

`--auth` is `oidc` (your own provider: the script asks for the client secret in the terminal) or `bundled-oidc` (Pocket ID included). Also: `--proxy traefik`, `--version x.y.z`, `--dir /opt/looty` (default), `--force` to overwrite an installation, `--upgrade` to update the files while keeping `.env`. Without parameters it asks the questions. It checks Docker, Compose, ports 80 and 443 (Caddy) and DNS, writes the files and offers to run `docker compose up -d`. The script is also on [GitHub](https://raw.githubusercontent.com/untypequicode/looty/main/deploy/install.sh): read it before running it. On NixOS, see [NixOS](/docs/host/nixos/).

## By hand

With the bundled [Caddy](https://caddyserver.com) (automatic HTTPS; ports 80 and 443 free, domain pointing to the machine):

```sh
curl -L -o docker-compose.yml https://github.com/untypequicode/looty/releases/latest/download/docker-compose.caddy.yml
curl -LO https://github.com/untypequicode/looty/releases/latest/download/Caddyfile
curl -o .env https://raw.githubusercontent.com/untypequicode/looty/main/.env.example
```

Edit `.env`:

- `APP_DOMAIN` and `BASE_URL=https://<domain>`;
- `DB_PASSWORD`;
- `LOOTY_SECRET_KEY` (`openssl rand -base64 32`);
- your OpenID Connect provider (`OIDC_ISSUER`, `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`), or the bundled Pocket ID ([see sign-in](/docs/host/sign-in/pocket-id/)).

Then start it:

```sh
docker compose up -d
```

Open `https://<domain>` and sign in: the first account becomes the administrator. Every variable is described in the [reference](/docs/reference/environment/).
