---
title: "Bundled Pocket ID"
description: "No identity provider? Run Pocket ID next to Looty, already linked."
---

The `*.pocket-id.*` files add [Pocket ID](https://pocket-id.org) (sign-in with passkeys) on its own subdomain `auth.<domain>`, already linked to Looty: on every start, the one-shot `pocket-id-init` service creates Looty's OIDC client in Pocket ID (and changes nothing if it exists).

1. Point the DNS of `auth.<domain>` to the server too, then take the files (with Caddy; behind Traefik, `docker-compose.pocket-id.yml` and no Caddyfile):

```sh
curl -L -o docker-compose.yml https://github.com/untypequicode/looty/releases/latest/download/docker-compose.caddy.pocket-id.yml
curl -L -o Caddyfile https://github.com/untypequicode/looty/releases/latest/download/Caddyfile.pocket-id
curl -LO https://github.com/untypequicode/looty/releases/latest/download/pocket-id-init.sh
curl -o .env https://raw.githubusercontent.com/untypequicode/looty/main/.env.example
```

2. In `.env`, besides the domain, database and `LOOTY_SECRET_KEY`, replace the OIDC block with:

```sh
AUTH_DOMAIN=auth.<domain>
OIDC_ISSUER=https://auth.<domain>
OIDC_CLIENT_ID=looty
OIDC_CLIENT_SECRET=<openssl rand -hex 32>
OIDC_PROVIDER_ID=<uuidgen>
POCKET_ID_ENCRYPTION_KEY=<openssl rand -base64 32>
POCKET_ID_STATIC_API_KEY=<openssl rand -hex 32>
```

3. `docker compose up -d`, then `docker compose logs pocket-id-init` should end with `prêt`.
4. **Right away**, open `https://auth.<domain>/setup` and create your account **with your email address**, then add a passkey. Until this account exists, whoever opens that page first becomes the Pocket ID administrator.
5. Sign in to Looty with *Continuer avec SSO*: as the first account, you are Looty's administrator. Add other people in Pocket ID (*Administration > Users*).

Behind Traefik, Looty reaches `https://auth.<domain>` through the host (`extra_hosts: host-gateway`): Traefik must listen on port 443 of the host. `POCKET_ID_STATIC_API_KEY` is an admin key: once the client exists you may remove it from `.env`.

## If the automatic setup fails

Read `docker compose logs pocket-id-init backend`, fix the cause and run `docker compose up -d` again. Or create the client by hand: in Pocket ID, *Administration > OIDC Clients > Add*, name `Looty`, callback `<BASE_URL>/api/auth/callback/<OIDC_PROVIDER_ID>` (exactly), PKCE on. Then give the client ID and secret to Looty:

```sh
docker compose exec postgres psql -U <DB_USER> -d <DB_NAME> -c "UPDATE oauth_providers SET client_id = '<client ID>', client_secret = '<secret>' WHERE id = '<OIDC_PROVIDER_ID>'"
docker compose restart backend
```
