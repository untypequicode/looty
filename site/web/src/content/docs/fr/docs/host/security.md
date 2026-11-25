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

## Tunnels PC

Détails : [Tunnels PC](/fr/docs/host/operate/tunnels/).

Un tunnel PC permet à une personne de relier son propre ordinateur à un projet (Jupyter, LaTeX) par une connexion sortante. **Les membres du projet peuvent alors exécuter du code sur cet ordinateur** : la fonction est désactivée tant que vous ne l’activez pas, et chaque limite se règle chez vous.

- **Activer / désactiver** : Administration > Instance > Tunnels. `LOOTY_TUNNELS=on` ne donne que la valeur initiale, jusqu’au premier enregistrement. La désactivation coupe tout de suite les tunnels en cours.
- **Limites** : tunnels par personne, durée de vie maximale des jetons, flux simultanés et débit par tunnel, types de service autorisés (`jupyter`, `latex`, `http`). Un changement s’applique aux tunnels ouverts en 15 secondes au plus ; un jeton plus long que le nouveau maximum ne se connecte plus.
- **Confirmation** : créer un tunnel demande de confirmer (`ack_remote_code`) que les membres peuvent exécuter du code sur son PC. Un tunnel peut être réservé à son propriétaire (`owner_only`) : les autres membres du projet ne peuvent pas s’en servir.
- **Audit** : Administration > Instance montre qui a créé, attaché, connecté, déconnecté et révoqué quel tunnel, et quand. Les événements plus anciens que la conservation (90 jours par défaut) sont supprimés. Les jetons n’y figurent jamais.
- **Jetons** : stockés hachés, montrés une seule fois, révocables (par leur propriétaire ou un administrateur), revérifiés toutes les 15 secondes : un jeton révoqué, expiré ou d’un compte suspendu est coupé. L’instance n’atteint jamais le réseau du PC, seulement les services que l’agent déclare.
