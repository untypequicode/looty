---
title: "Installation rapide"
description: "Démarrer une instance Looty avec les fichiers de la release et un fichier .env."
---

:::note
Le **générateur d’installation** dans le navigateur (zip) et la commande `curl … | sh` sont prévus (voir la décision *le générateur d’installation tourne dans le navigateur*). En attendant leur publication, suivez les étapes ci-dessous : ce sont celles que le générateur fera pour vous.
:::

Avec [Caddy](https://caddyserver.com) inclus (HTTPS automatique ; ports 80 et 443 libres, domaine pointant vers la machine) :

```sh
curl -L -o docker-compose.yml https://github.com/untypequicode/looty/releases/latest/download/docker-compose.caddy.yml
curl -LO https://github.com/untypequicode/looty/releases/latest/download/Caddyfile
curl -o .env https://raw.githubusercontent.com/untypequicode/looty/main/.env.example
```

Modifiez `.env` :

- `APP_DOMAIN` et `BASE_URL=https://<domaine>` ;
- `DB_PASSWORD` ;
- `LOOTY_SECRET_KEY` (`openssl rand -base64 32`) ;
- votre fournisseur OpenID Connect (`OIDC_ISSUER`, `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`), ou le Pocket ID inclus ([voir connexion](/fr/docs/host/sign-in/pocket-id/)).

Puis démarrez :

```sh
docker compose up -d
```

Ouvrez `https://<domaine>` et connectez-vous : le premier compte devient l’administrateur. Chaque variable est décrite dans la [référence](/fr/docs/reference/environment/).
