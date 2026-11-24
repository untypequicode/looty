---
title: "Sécurité"
description: "Secrets, LOOTY_DEV_LOGIN et ce qu’il faut exposer."
---

- **Secrets** : mettez un `DB_PASSWORD` solide et un `LOOTY_SECRET_KEY` aléatoire (`openssl rand -base64 32`). Ne commitez jamais `.env`. Gardez `LOOTY_SECRET_KEY` avec vos sauvegardes.
- **`LOOTY_DEV_LOGIN` ne doit jamais être activé** sur une vraie instance : il permet à n’importe qui de se connecter sous n’importe quel utilisateur. Les fichiers compose ne le définissent pas.
- **`LOOTY_DEMO`** sert uniquement à une démo publique, dans sa propre pile (`docker-compose.demo.yml`).
- **Exposition** : seul le proxy doit écouter sur les ports 80 et 443. PostgreSQL et les trois services restent sur le réseau Docker interne : ne publiez pas leurs ports.
- **Réseaux privés** : laissez `CALDAV_ALLOW_PRIVATE`, `JUPYTER_ALLOW_PRIVATE` et `AI_ALLOW_PRIVATE` à `false` sauf besoin ; ils permettent aux utilisateurs de faire contacter à votre serveur des machines de votre réseau.
- **Connexion** : limitez qui peut créer un compte chez votre fournisseur d’identité. Avec Google, limitez l’écran de consentement.
- **Mises à jour** : appliquez-les régulièrement, après une sauvegarde.
