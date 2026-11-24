---
title: "Mises à jour et épinglage de version"
description: "Mettre à jour une instance et rester sur une version précise."
---

Par défaut les fichiers compose utilisent les images `latest` :

```sh
docker compose pull && docker compose up -d
```

Pour rester sur une version, mettez `LOOTY_VERSION=<x.y.z>` dans `.env` (la version de la release web) et lancez les mêmes commandes quand vous décidez d’avancer. Lisez les notes de version (dans l’app, *Nouveautés*, ou sur GitHub) avant de mettre à jour, et **sauvegardez d’abord**. Les migrations de la base se font automatiquement au démarrage.

Quand une version change les fichiers compose (par exemple le stockage de PostgreSQL en 0.30), les notes de version le disent : téléchargez le nouveau fichier compose de la release et comparez-le au vôtre.
