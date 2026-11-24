---
title: "NixOS"
description: "Faire tourner Looty sous NixOS avec le module services.looty : secrets dans un fichier, PostgreSQL local, Caddy ou nginx, Pocket ID inclus en option."
---

Le flake de Looty fournit `nixosModules.default`. Il lance les mêmes images de release que l’installation Docker Compose (serveur, interface web, édition en direct), en unités systemd via Podman, avec un PostgreSQL local, un proxy configuré par les options NixOS habituelles (certificats HTTPS compris) et une sauvegarde quotidienne de la base.

Le [générateur de configuration](/fr/install/) écrit cette configuration et le fichier de secrets pour vous, dans votre navigateur.

## Configuration

```nix
{
  inputs.looty.url = "github:untypequicode/looty";
  # …
  outputs = { nixpkgs, looty, ... }: {
    nixosConfigurations.monhote = nixpkgs.lib.nixosSystem {
      modules = [
        looty.nixosModules.default
        {
          services.looty = {
            enable = true;
            domain = "looty.exemple.fr";
            instanceName = "Mon association";
            # auth = "oidc" (votre fournisseur, défaut) ou "bundled-oidc" (Pocket ID sur auth.<domaine>)
            auth = "bundled-oidc";
            version = "0.44.1"; # à figer ; "latest" par défaut
            environmentFile = "/run/secrets/looty.env"; # agenix, sops-nix, ou un fichier à vous
          };
        }
      ];
    };
  };
}
```

Les ports 80 et 443 sont ouverts pour le proxy. Options : `proxy` (`caddy` par défaut, `nginx`, ou `none` pour router vous-même les ports locaux), `database.createLocally` (`false` : mettez votre `DATABASE_URL` dans le fichier d’environnement), `ports.*`, `backup.enable`, `openFirewall`. Avec `none` ou votre propre proxy, suivez le tableau de routage de l’[installation détaillée](/fr/docs/host/install/).

## Secrets

Le fichier d’environnement contient des lignes `CLE=valeur` et **ne doit pas être dans le magasin Nix** (un chemin écrit en chaîne comme `/run/secrets/…`, pas un littéral de chemin). Générez les valeurs sur l’hôte :

```sh
cat > looty.env <<ENV
DB_PASSWORD=$(openssl rand -hex 16)
LOOTY_SECRET_KEY=$(openssl rand -base64 32)
# bundled-oidc :
OIDC_CLIENT_SECRET=$(openssl rand -hex 32)
OIDC_PROVIDER_ID=$(uuidgen)
POCKET_ID_ENCRYPTION_KEY=$(openssl rand -hex 32)
POCKET_ID_STATIC_API_KEY=$(openssl rand -hex 32)
# oidc (votre fournisseur) à la place :
# OIDC_ISSUER=https://idp.exemple.fr
# OIDC_CLIENT_ID=looty
# OIDC_CLIENT_SECRET=…
ENV
```

`DB_PASSWORD` ne contient que des lettres et des chiffres. Toute autre [variable de Looty](/fr/docs/reference/environment/) (`SMTP_*`…) peut être ajoutée au même fichier. Avec agenix ou sops-nix, faites pointer `environmentFile` vers le chemin déchiffré.

## Pocket ID inclus

Avec `auth = "bundled-oidc"`, le module active `services.pocket-id` de nixpkgs sur `https://auth.<domaine>` (un sous-domaine, avec son enregistrement DNS), et une unité ponctuelle y crée le client de Looty avec le même script que l’installation Compose. Après le premier démarrage, créez le premier compte Pocket ID sur `https://auth.<domaine>/setup` : **cette page reste ouverte à tout le monde tant que le premier compte n’existe pas**. Il devient administrateur de Pocket ID, et le premier compte de Looty en est l’administrateur.

## Vérifications

`nix flake check` démarre deux machines virtuelles : l’une avec Caddy et la base locale (mot de passe, fichier de secrets, routage de `/`, `/api` et `/collab`), l’autre avec le vrai Pocket ID et la création du client OIDC. Les trois images de Looty y sont remplacées par de petites images de substitution (la machine virtuelle n’a pas de réseau) ; le vrai serveur et l’édition en direct sont éprouvés par les tests de la pile Compose en CI.

## Comptes locaux

`auth = "local"` est réservé au mode comptes locaux et est refusé tant que ce mode n’existe pas.
