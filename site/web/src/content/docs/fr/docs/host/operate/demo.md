---
title: "Faire tourner une démo publique"
description: "Une instance de démonstration jetable, remise à zéro chaque nuit, dans sa propre pile."
---

La démo publique de Looty est une pile à part, jamais une vraie instance : la connexion est remplacée par « Entrer dans la démo », et chaque visiteur reçoit une copie jetable de l’espace *Looty Démo* aux données fictives. Rien n’y est conservé.

```sh
mkdir -p /srv/looty-demo && cd /srv/looty-demo
curl -LO https://raw.githubusercontent.com/untypequicode/looty/main/docker-compose.demo.yml
curl -o .env https://raw.githubusercontent.com/untypequicode/looty/main/.env.demo.example
# DB_PASSWORD : openssl rand -hex 16
docker compose -f docker-compose.demo.yml up -d
```

La pile a sa propre base et ses propres volumes (`demo_postgres`, `demo_uploads`), ses propres noms de routeurs Traefik (`looty-demo`) et des limites de mémoire sur chaque conteneur. Faites pointer le DNS de `DEMO_DOMAIN` vers la machine et laissez votre Traefik joindre la pile (voir [Installer](/fr/docs/host/install/)).

## Ménage et remise à zéro de chaque nuit

- La copie d’un visiteur est supprimée 24 heures après sa création, quoi qu’il arrive.
- **Chaque nuit**, à `LOOTY_DEMO_RESET_HOUR` (3 par défaut, dans `CALENDAR_DEFAULT_TZ`, sinon UTC), le serveur supprime **toutes** les copies, expirées ou non : comptes, espaces et fichiers. Les personnes encore dans la démo recommencent avec une nouvelle copie à leur prochaine visite. `LOOTY_DEMO_RESET_HOUR=off` désactive la remise à zéro. Le modèle est généré par le code : il n’y a rien à restaurer.
- `LOOTY_DEMO_MAX_COPIES`, `LOOTY_DEMO_COPIES_PER_IP_HOUR`, `LOOTY_DEMO_MAX_FILE_MB` et `LOOTY_DEMO_MAX_COPY_MB` bornent la charge (voir la [référence des variables](/fr/docs/reference/environment/)).

## Rester à jour et surveillée

Lancez une mise à jour chaque nuit, après la remise à zéro, par exemple avec une entrée cron dans le dossier de la pile :

```sh
30 3 * * * cd /srv/looty-demo && docker compose -f docker-compose.demo.yml pull -q && docker compose -f docker-compose.demo.yml up -d
```

## Partager un réseau Docker avec votre vraie instance

Les services de la démo s’appellent `demo-postgres`, `demo-backend`, `demo-collaboration` et `demo-frontend` (avec des alias `looty-demo-*`) pour ne jamais entrer en collision avec le `postgres` ou le `backend` d’une autre pile sur un réseau partagé comme `traefik-public`. Gardez cette règle pour toute pile ajoutée : des noms ou alias uniques sur un réseau partagé. Lancez `up` sans `--remove-orphans`, sauf si vous donnez les deux fichiers `-f`, sinon les conteneurs de l’autre pile sont supprimés.

Surveillez `https://<DEMO_DOMAIN>/api/health` depuis un moniteur de disponibilité, et l’espace disque libre du volume qui contient `/var/lib/docker`.
