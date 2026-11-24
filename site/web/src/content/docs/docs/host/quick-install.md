---
title: "Quick install"
description: "Get a Looty instance running with the release files and a .env file."
---

:::note
The browser **install generator** (zip) and the `curl … | sh` command are planned (see the design decision *install generator runs in the browser*). Until they are published, use the steps below: they are what the generator will produce for you.
:::

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
