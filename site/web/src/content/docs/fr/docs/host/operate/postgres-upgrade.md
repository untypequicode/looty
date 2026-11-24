---
title: "Montées de version PostgreSQL"
description: "Passer la base à une nouvelle version majeure de PostgreSQL ou à un nouveau dossier."
---

Une version majeure de PostgreSQL ne peut pas ouvrir les données d’une autre. Pour passer de PostgreSQL 15 à 18 sur une ancienne pile, lancez `scripts/pg-upgrade.sh [docker-compose.prod.yml]` sur le serveur, dans le dossier du fichier compose. Il sauvegarde la base, arrête la pile, passe le fichier compose à la nouvelle image et à un nouveau volume, restaure et redémarre. Comptez quelques minutes d’indisponibilité. L’ancien volume n’est **pas** supprimé : pour revenir en arrière, remettez le fichier compose sauvegardé (`.pre-pg18`) et `docker compose up -d`.

## Déplacer le dossier de la base

Les installations qui utilisent encore l’ancien volume Docker `postgres_data_18` ne doivent **pas se contenter de mettre à jour** : le nouveau fichier compose démarrerait sur une base vide. Depuis le dossier de votre fichier compose :

```sh
docker compose exec -T postgres pg_dump -U "$DB_USER" -Fc "$DB_NAME" > looty-backup.dump   # filet de sécurité
docker compose down                                                                          # jamais avec -v
VOL=$(docker volume ls -q | grep postgres_data_18)
mkdir -p data/postgres
docker run --rm -v "$VOL":/from -v "$PWD/data/postgres":/to alpine cp -a /from/. /to/
docker compose up -d
```

Supprimez l’ancien volume (`docker volume rm "$VOL"`) quand tout fonctionne. Pour un autre dossier, mettez `LOOTY_DB_DATA=/ce/dossier` et copiez dedans.
