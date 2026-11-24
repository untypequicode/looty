---
title: "Backup and restore"
description: "Back up the database and uploads with the provided scripts, and restore them."
---

Three ways, which you can combine:

- **Automatic with Compose**: tick *Automatic database backup* in the [configuration generator](/install/) (or `install.sh --backup`). A `db-backup` service dumps the database every 24 hours into `data/db-backups` and keeps the last 7. Uploaded files (`backend/uploads`), `.env` and `LOOTY_SECRET_KEY` are not in it: copy them, and the dumps, off the machine.
- **From the app**: *Administration > Backups* makes full and scheduled backups.
- **With the scripts**, run from the folder of your compose file (where `.env` is):

```sh
./scripts/backup.sh                          # database (pg_dump) and uploads into ./backups, keeps the last 7
./scripts/restore.sh backups/backup_<date>.sql.gz   # restores a backup (and the uploads of the same date)
```

Also keep a copy of `LOOTY_SECRET_KEY`: without it the encrypted secrets (CalDAV passwords, AI keys, Jupyter tokens) cannot be read. Copy backups **off the machine**. Test a restore on a spare machine before you need it.

PostgreSQL data lives in `./data/postgres`: you can also stop the stack and copy that folder, or use `pg_dump`.
