---
title: "Pocket ID inclus"
description: "Pas de fournisseur d’identité ? Faites tourner Pocket ID à côté de Looty, déjà relié."
---

Les fichiers `*.pocket-id.*` ajoutent [Pocket ID](https://pocket-id.org) (connexion par passkeys) sur son propre sous-domaine `auth.<domaine>`, déjà relié à Looty : à chaque démarrage, le service ponctuel `pocket-id-init` crée le client OIDC de Looty dans Pocket ID (et ne change rien s’il existe).

1. Faites aussi pointer le DNS de `auth.<domaine>` vers le serveur, puis prenez les fichiers (avec Caddy ; derrière Traefik, `docker-compose.pocket-id.yml` et pas de Caddyfile) :

```sh
curl -L -o docker-compose.yml https://github.com/untypequicode/looty/releases/latest/download/docker-compose.caddy.pocket-id.yml
curl -L -o Caddyfile https://github.com/untypequicode/looty/releases/latest/download/Caddyfile.pocket-id
curl -LO https://github.com/untypequicode/looty/releases/latest/download/pocket-id-init.sh
curl -o .env https://raw.githubusercontent.com/untypequicode/looty/main/.env.example
```

2. Dans `.env`, en plus du domaine, de la base et de `LOOTY_SECRET_KEY`, remplacez le bloc OIDC par :

```sh
AUTH_DOMAIN=auth.<domaine>
OIDC_ISSUER=https://auth.<domaine>
OIDC_CLIENT_ID=looty
OIDC_CLIENT_SECRET=<openssl rand -hex 32>
OIDC_PROVIDER_ID=<uuidgen>
POCKET_ID_ENCRYPTION_KEY=<openssl rand -base64 32>
POCKET_ID_STATIC_API_KEY=<openssl rand -hex 32>
```

3. `docker compose up -d`, puis `docker compose logs pocket-id-init` doit se terminer par `prêt`.
4. **Tout de suite**, ouvrez `https://auth.<domaine>/setup` et créez votre compte **avec votre adresse e-mail**, puis ajoutez une passkey. Tant que ce compte n’existe pas, la première personne qui ouvre cette page devient administratrice de Pocket ID.
5. Connectez-vous à Looty avec *Continuer avec SSO* : premier compte, vous êtes l’administrateur de Looty. Ajoutez les autres personnes dans Pocket ID (*Administration > Users*).

Derrière Traefik, Looty joint `https://auth.<domaine>` par l’hôte (`extra_hosts: host-gateway`) : Traefik doit écouter sur le port 443 de l’hôte. `POCKET_ID_STATIC_API_KEY` est une clé d’administration : une fois le client créé, vous pouvez la retirer de `.env`.

## Si la configuration automatique échoue

Lisez `docker compose logs pocket-id-init backend`, corrigez la cause et relancez `docker compose up -d`. Ou créez le client à la main : dans Pocket ID, *Administration > OIDC Clients > Add*, nom `Looty`, callback `<BASE_URL>/api/auth/callback/<OIDC_PROVIDER_ID>` (exactement), PKCE activé. Puis donnez l’identifiant et le secret à Looty :

```sh
docker compose exec postgres psql -U <DB_USER> -d <DB_NAME> -c "UPDATE oauth_providers SET client_id = '<identifiant>', client_secret = '<secret>' WHERE id = '<OIDC_PROVIDER_ID>'"
docker compose restart backend
```
