---
title: "Installation détaillée"
description: "Docker Compose avec Caddy, avec Traefik, derrière un autre proxy, et sous NixOS."
---

Les fichiers compose sont produits depuis `deploy/templates/` par `deploy/render.sh` : modifiez les modèles, pas les fichiers produits. Épinglez une version avec `LOOTY_VERSION=<x.y.z>` dans `.env`.

## Avec Caddy (pas de proxy existant)

Suivez l’[installation rapide](/fr/docs/host/quick-install/). Le `Caddyfile` route tout et obtient les certificats HTTPS.

## Avec Traefik

Prenez seulement le fichier compose ; le routage est dans ses étiquettes :

```sh
curl -LO https://github.com/untypequicode/looty/releases/latest/download/docker-compose.yml
curl -o .env https://raw.githubusercontent.com/untypequicode/looty/main/.env.example
```

Les étiquettes ne fixent ni point d’entrée ni certificat : Traefik doit joindre le réseau de la pile et servir HTTPS sur ses points d’entrée, par exemple `--entrypoints.websecure.http.tls.certResolver=<résolveur>`. `APP_NAME` et `APP_DOMAIN` dans `.env` nomment les routeurs et la règle d’hôte.

## Derrière un autre reverse proxy

Utilisez le fichier compose Traefik et routez, WebSockets activés :

| Chemin | Cible |
|---|---|
| `/api`, `/uploads`, `/pages`, `/dav`, `/.well-known/caldav` | backend, port 8080 |
| `/collab` | collaboration, port 8082, **préfixe retiré** |
| tout le reste | frontend, port 3000 |

Le `Caddyfile` en est un exemple qui fonctionne. Transmettez les en-têtes `Host` et `X-Forwarded-*`.

## NixOS

Il n’y a pas de module NixOS pour le serveur. Lancez les fichiers compose avec Docker activé sous NixOS (voir [prérequis](/fr/docs/host/requirements/)). Seule l’*app de bureau* est empaquetée en flake.
