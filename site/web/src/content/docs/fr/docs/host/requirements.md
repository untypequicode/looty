---
title: "Prérequis"
description: "Machine, Docker ou NixOS, domaine et ports nécessaires pour héberger Looty."
---

- **Une machine** que vous maîtrisez (un petit VPS suffit pour une équipe) : Linux en amd64 ou arm64. Les fichiers compose plafonnent la mémoire à environ 1 Go pour le backend, 768 Mo pour PostgreSQL et quelques centaines de Mo pour le reste ; prévoyez **2 Go de RAM** et quelques Go de disque, plus la place des fichiers envoyés.
- **Docker** avec le plugin Compose. Sous NixOS, activez Docker (`virtualisation.docker.enable = true;`) et utilisez les mêmes fichiers compose : il n’y a pas de module NixOS dédié.
- **Un nom de domaine** dont le DNS pointe vers la machine (et un second, `auth.<domaine>`, si vous utilisez le Pocket ID inclus).
- **Les ports 80 et 443** libres, pour HTTPS. Avec Caddy inclus, les certificats viennent de Let’s Encrypt automatiquement. Derrière un proxy existant, seul ce proxy en a besoin.
- **Un serveur d’envoi d’e-mails** (facultatif) pour les notifications par e-mail.
- **Une méthode de connexion** : un fournisseur OpenID Connect, ou le Pocket ID inclus (voir [connexion](/fr/docs/host/sign-in/oidc/)).
