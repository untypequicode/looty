---
title: "PostgreSQL upgrades"
description: "Move the database to a new PostgreSQL major version or to a new folder."
---

A PostgreSQL major version cannot open the data of another one. To go from PostgreSQL 15 to 18 on an older stack, run `scripts/pg-upgrade.sh [docker-compose.prod.yml]` on the server, in the folder of the compose file. It dumps the database, stops the stack, switches the compose file to the new image and a new volume, restores and restarts. Expect a few minutes of downtime. The old volume is **not** deleted: to roll back, put back the saved compose file (`.pre-pg18`) and `docker compose up -d`.

## Move the database folder

Installations that still use the old Docker volume `postgres_data_18` must **not just pull**: the new compose file would start on an empty database. From the folder of your compose file:

```sh
docker compose exec -T postgres pg_dump -U "$DB_USER" -Fc "$DB_NAME" > looty-backup.dump   # safety net
docker compose down                                                                          # never with -v
VOL=$(docker volume ls -q | grep postgres_data_18)
mkdir -p data/postgres
docker run --rm -v "$VOL":/from -v "$PWD/data/postgres":/to alpine cp -a /from/. /to/
docker compose up -d
```

Delete the old volume (`docker volume rm "$VOL"`) once everything works. To use another folder, set `LOOTY_DB_DATA=/that/folder` and copy into it instead.
