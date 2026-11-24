---
title: "Sauvegarde et restauration"
description: "Sauvegarder la base et les fichiers avec les scripts fournis, et les restaurer."
---

Deux moyens, combinables :

- **Depuis l’app** : *Administration > Sauvegardes* fait des sauvegardes complètes et planifiées.
- **Avec les scripts**, lancés depuis le dossier de votre fichier compose (là où est `.env`) :

```sh
./scripts/backup.sh                          # base (pg_dump) et fichiers dans ./backups, garde les 7 dernières
./scripts/restore.sh backups/backup_<date>.sql.gz   # restaure une sauvegarde (et les fichiers de la même date)
```

Gardez aussi une copie de `LOOTY_SECRET_KEY` : sans elle, les secrets chiffrés (mots de passe CalDAV, clés d’IA, jetons Jupyter) ne se lisent plus. Copiez les sauvegardes **hors de la machine**. Testez une restauration sur une machine de rechange avant d’en avoir besoin.

Les données PostgreSQL sont dans `./data/postgres` : vous pouvez aussi arrêter la pile et copier ce dossier, ou utiliser `pg_dump`.
