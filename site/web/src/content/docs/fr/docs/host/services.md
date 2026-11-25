---
title: "E-mails, push, Moodle, Jupyter et stockage"
description: "Services facultatifs : e-mails SMTP, notifications push, Moodle, Jupyter et stockage des fichiers."
---

## E-mails (SMTP)

À configurer dans *Administration > E-mail* (recommandé), ou par variables en repli : `SMTP_HOST`, `SMTP_FROM` (les deux pour l’activer), `SMTP_PORT`, `SMTP_SECURITY` (`starttls`, `tls`, `none`), `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM_NAME`. Les fichiers compose les transmettent depuis `.env`.

## Notifications push

- **Web Push** (navigateurs et web app iPhone) fonctionne sans réglage : Looty crée sa propre clé VAPID au premier besoin et la stocke chiffrée. L’instance doit être servie en HTTPS et pouvoir joindre les services push des navigateurs.
- **UnifiedPush** (Android) : les gens installent un distributeur UnifiedPush (par exemple ntfy) sur leur téléphone et le choisissent dans l’app. Vous pouvez héberger votre propre distributeur ; rien à régler dans Looty.

## Moodle

Chaque personne relie son propre compte Moodle depuis ses paramètres ; rien n’est requis sur l’instance. Réglages facultatifs : `MOODLE_POLL_SECONDS`, `MOODLE_CACHE_MB`, `MOODLE_CACHE_FILE_MB` (non transmis par les fichiers compose : ajoutez-les dans un `docker-compose.override.yml`).

## Jupyter

Les serveurs Jupyter des projets ne sont contactés que par le serveur Looty. Par défaut, les serveurs en réseau privé ou en `http` sont refusés : mettez `JUPYTER_ALLOW_PRIVATE=true` si votre serveur Jupyter tourne sur votre réseau local ou sur le même hôte. Pour que chacun utilise son propre ordinateur comme serveur Jupyter sans ouvrir de port, voir [Tunnels PC](/fr/docs/host/operate/tunnels/).

## CalDAV et fournisseurs d’IA

Les serveurs CalDAV externes en réseau privé demandent `CALDAV_ALLOW_PRIVATE=true` ; les fournisseurs d’IA en réseau privé (par exemple Ollama) demandent `AI_ALLOW_PRIVATE=true`.

## Stockage des fichiers

Les fichiers envoyés sont dans `./backend/uploads` et les sauvegardes dans `./backend/backups`, à côté du fichier compose (montés dans le backend). Les données PostgreSQL sont dans `./data/postgres` (`LOOTY_DB_DATA` les déplace). Incluez-les toutes dans vos sauvegardes. LaTeX demande un compilateur dans l’image backend ou dans `LATEX_BIN_DIR`.
