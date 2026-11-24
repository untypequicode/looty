---
title: "Installation rapide"
description: "Démarrer une instance Looty avec les fichiers de la release et un fichier .env."
---

:::tip
Le [**générateur de configuration**](/fr/install/) fabrique ces fichiers dans votre navigateur (un zip, ou une configuration NixOS) et n’envoie rien nulle part : les secrets sont tirés par votre navigateur. La commande ci-dessous fait la même chose sur votre machine : elle y génère les secrets et ne les prend jamais en argument.
:::

## Une commande

```sh
curl -fsSL https://looty.untypequicode.fr/install.sh | sh -s -- --domain looty.exemple.fr --proxy caddy --name "Mon association" --auth bundled-oidc
```

`--auth` vaut `oidc` (votre fournisseur : le script demande le secret du client dans le terminal) `bundled-oidc` (Pocket ID inclus) ou `local` ([comptes locaux](/fr/docs/host/sign-in/local-accounts/), déconseillés). Aussi : `--proxy traefik`, `--version x.y.z`, `--dir /opt/looty` (défaut), `--force` pour écraser une installation, `--upgrade` pour mettre à jour les fichiers en gardant `.env`. Sans paramètre, il pose les questions. Il vérifie Docker, Compose, les ports 80 et 443 (Caddy) et le DNS, écrit les fichiers et propose de lancer `docker compose up -d`. Le script est aussi sur [GitHub](https://raw.githubusercontent.com/untypequicode/looty/main/deploy/install.sh) : lisez-le avant de l’exécuter. Sous NixOS, voir [NixOS](/fr/docs/host/nixos/).

## À la main

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
