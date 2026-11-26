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

## Fichiers envoyés

Depuis la 0.66.0, les fichiers sous `/uploads/` (images et pièces jointes des pages, des tâches, avatars) ne sont **plus publics**.

- Chaque fichier est lié à la page, à la tâche ou à l’espace pour lequel il a été envoyé, et n’est servi qu’à une personne connectée (cookie de session, jeton d’API, applications Android et bureau) qui peut lire cette page, cette tâche ou cet espace. Retirez l’accès : le fichier cesse de se charger aussitôt, même pour qui en connaît encore l’adresse. La réponse est `404` pour une personne connectée sans accès et `401` sans connexion.
- Publics par choix : les avatars et l’icône de l’instance. Une page partagée par **lien public** sert ses propres images à toute personne qui a le lien, et celles-là seulement, tant que le lien existe.
- Aucun cache partagé ne garde un fichier (`Cache-Control: private, no-cache`, `Vary: Cookie`) ; les médias acceptent les requêtes par plages.
- Les fichiers d’avant la mise à jour sont rattachés pendant la migration à la première page, tâche ou avatar qui les cite. Les fichiers cités nulle part et sans auteur connu sont listés comme **sans propriétaire** (ligne de journal au démarrage et compteur `unattributed_files` de l’aperçu du stockage dans l’API d’administration) et seuls les administrateurs de l’instance peuvent les ouvrir.
- Un client qui ne peut pas envoyer d’identifiants peut demander `GET /api/uploads/{nom}/link`, un lien signé valable 10 minutes. En dernier recours, `LOOTY_UPLOADS_PUBLIC=true` rétablit l’ancien comportement ouvert.
- Les exports, sauvegardes et e-mails envoyés depuis Looty lisent les fichiers sur le serveur, pas par cette adresse : rien ne change, sauf qu’un e-mail ne peut pas joindre un fichier que son expéditeur ne peut pas ouvrir.

## Tunnels PC

Détails : [Tunnels PC](/fr/docs/host/operate/tunnels/).

Un tunnel PC permet à une personne de relier son propre ordinateur à un projet (Jupyter, LaTeX) par une connexion sortante. **Les membres du projet peuvent alors exécuter du code sur cet ordinateur** : la fonction est désactivée tant que vous ne l’activez pas, et chaque limite se règle chez vous.

- **Activer / désactiver** : Administration > Instance > Tunnels. `LOOTY_TUNNELS=on` ne donne que la valeur initiale, jusqu’au premier enregistrement. La désactivation coupe tout de suite les tunnels en cours.
- **Limites** : tunnels par personne, durée de vie maximale des jetons, flux simultanés et débit par tunnel, types de service autorisés (`jupyter`, `latex`, `http`). Un changement s’applique aux tunnels ouverts en 15 secondes au plus ; un jeton plus long que le nouveau maximum ne se connecte plus.
- **Confirmation** : créer un tunnel demande de confirmer (`ack_remote_code`) que les membres peuvent exécuter du code sur son PC. Un tunnel peut être réservé à son propriétaire (`owner_only`) : les autres membres du projet ne peuvent pas s’en servir.
- **Audit** : Administration > Instance montre qui a créé, attaché, connecté, déconnecté et révoqué quel tunnel, et quand. Les événements plus anciens que la conservation (90 jours par défaut) sont supprimés. Les jetons n’y figurent jamais.
- **Jetons** : stockés hachés, montrés une seule fois, révocables (par leur propriétaire ou un administrateur), revérifiés toutes les 15 secondes : un jeton révoqué, expiré ou d’un compte suspendu est coupé. L’instance n’atteint jamais le réseau du PC, seulement les services que l’agent déclare.
