---
title: "Diagnostiquer son instance"
description: "La page Diagnostic, la commande doctor, et les journaux."
---

## La page Diagnostic et `looty doctor`

*Administration > Diagnostic* vérifie que l’instance est bien installée, et indique OK, un avertissement ou une erreur pour chaque point, avec un lien vers le [guide de dépannage](/fr/docs/host/operate/troubleshooting/) :

- `BASE_URL` correspond à l’adresse que vous utilisez, HTTPS et le certificat sont valides, le proxy transmet `Host`, `X-Forwarded-Proto` et `X-Forwarded-For` ;
- les WebSocket `/api/ws` (temps réel) et `/collab` (édition en direct) s’ouvrent depuis votre navigateur ;
- la découverte CalDAV (`/.well-known/caldav`), chaque fournisseur OpenID Connect (découverte, adresse de retour à déclarer), le SMTP (connexion, et e-mail de test sur demande) ;
- la base de données (version, taille), l’espace disque libre, le dossier des fichiers, la clé de chiffrement et la dernière version publiée (si `LOOTY_UPDATE_CHECK` est activée).

Les mêmes contrôles, sans ceux qui demandent un navigateur, se lancent dans un terminal et sortent avec le code 1 si l’un est une erreur :

```sh
docker compose exec backend looty doctor          # ajoutez --json pour des scripts
```

L’assistant de premier lancement, sur `/setup`, se termine par un lien vers cette page.

## Journaux

```sh
docker compose ps                         # tous les services doivent tourner (postgres healthy)
docker compose logs --tail 100 backend    # API, connexion, e-mails, migrations
docker compose logs --tail 100 collaboration frontend
curl -s https://<domaine>/api/health      # {"version": "..."} quand le backend répond
```

La page de connexion indique quand le fournisseur OpenID Connect est injoignable ou a rejeté Looty ; le journal du backend nomme alors la cause (mauvais issuer, mauvais secret, URL de redirection inconnue). Mettez `RUST_LOG=debug` sur le backend pour plus de détails le temps de l’enquête.
