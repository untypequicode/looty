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

### Délais et connexions longues

L'édition en direct (`/collab`) et les notifications (`/api/ws`) sont des WebSockets qui restent ouverts des heures. Le serveur de collaboration envoie un ping toutes les 15 secondes, mais un proxy qui coupe les connexions sur sa propre minuterie provoque des déconnexions périodiques (souvent toutes les une à deux minutes) qui laissent les pages en lecture seule jusqu'à la reconnexion. Relevez ces limites :

- **Traefik** : réglez les délais du point d'entrée, par exemple `--entrypoints.websecure.transport.respondingTimeouts.readTimeout=0` et `--entrypoints.websecure.transport.respondingTimeouts.idleTimeout=300s` (un délai de lecture de 60 s et un délai d'inactivité de 120 s coupent les WebSockets inactifs).
- **nginx** : `proxy_read_timeout 3600s;` et `proxy_send_timeout 3600s;` sur les emplacements `/collab` et `/api/ws`, avec `proxy_http_version 1.1` et les en-têtes `Upgrade` / `Connection`.
- **Cloudflare** : une connexion sans trafic pendant 100 secondes est fermée. Le ping de 15 secondes la maintient, mais ne placez pas les chemins WebSocket derrière une règle qui les met en tampon ou les réécrit.

## NixOS

Le flake fournit un module NixOS, `services.looty` : voir [NixOS](/fr/docs/host/nixos/).
