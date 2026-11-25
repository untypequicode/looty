---
title: "Tunnels PC"
description: "Laisser chacun utiliser son ordinateur comme serveur Jupyter par une connexion sortante : LOOTY_TUNNELS, réglages d'administration, proxy inverse et délais WebSocket, audit et sécurité."
---

Un **tunnel PC** permet à une personne de relier son ordinateur à un projet, pour que l'instance l'utilise comme serveur Jupyter (Python, R...) tant que son agent (`looty-bridge`) tourne. L'agent ouvre un WebSocket sortant vers votre instance : **aucun port entrant** n'est nécessaire sur le PC, et rien ne change sur le réseau de votre serveur. Pour le côté utilisateur, voir [Utiliser mon PC comme serveur Jupyter ou R](/fr/docs/use/features/pc-tunnel/).

:::caution
**Les membres d'un projet peuvent exécuter du code sur le PC qui y est connecté**, avec les droits de l'utilisateur qui y lance Jupyter. La fonction est **désactivée par défaut**. Ne l'activez que si vos utilisateurs en ont conscience.
:::

## L'activer

- Définissez `LOOTY_TUNNELS=on` dans l'environnement du backend (voir la [référence des variables](/fr/docs/reference/environment/)). Cela ne donne que la **valeur initiale** de l'interrupteur.
- Après le premier enregistrement, **Administration > Instance > Tunnels** fait foi : l'interrupteur, puis les limites ci-dessous. Désactiver et enregistrer coupe aussitôt tous les tunnels en cours ; les projets reviennent à leur moteur par défaut (Pyodide, WebR).
- Non disponible dans la démo publique.

## Ce que l'instance peut atteindre

Uniquement les **services que l'agent déclare** (`jupyter`, `latex`, `http`), à une adresse configurée **sur le PC**, jamais choisie par l'instance. L'instance ne peut atteindre aucun autre port du PC ni de son réseau, et ne peut rien faire quand l'agent n'est pas connecté. Votre serveur ne contacte aucun réseau privé pour les tunnels : `JUPYTER_ALLOW_PRIVATE` ne les concerne pas. Le navigateur ne parle jamais au PC : seul le serveur Looty le fait, comme pour tout serveur Jupyter de projet.

## Réglages d'administration

Administration > Instance > Tunnels (valeurs par défaut entre parenthèses) :

- **Activé** (non).
- **Jetons actifs par personne** (20).
- **Durée de vie maximale d'un jeton** (365 jours ; un jeton est créé avec 30 jours par défaut).
- **Flux simultanés par tunnel** (16) : requêtes et connexions de noyau en parallèle.
- **Débit par tunnel** (illimité), dans les deux sens.
- **Types de service autorisés** : `jupyter`, `latex`, `http`.
- **Conservation de l'audit** (90 jours).

Les changements s'appliquent aux tunnels déjà ouverts en **15 secondes** au plus (l'instance revérifie chaque connexion à la base et à ces réglages à chaque ping) : un jeton révoqué, expiré, supprimé ou d'un compte suspendu, un type qui n'est plus autorisé ou une durée au-dessus du nouveau maximum coupe la connexion. Les refus utilisent des codes explicites que l'interface traduit (désactivé, trop de jetons, type non autorisé, durée trop longue, propriétaire seulement).

## Modèle de sécurité

- **Jetons** : `lty_tun_...`, liés à une personne et à un espace, stockés hachés, montrés une seule fois à la création, révocables par leur propriétaire ou par un administrateur. Ils n'authentifient que la connexion du tunnel : ils ne peuvent pas appeler le reste de l'API, et les jetons d'accès personnels ne peuvent pas gérer les tunnels.
- **Confirmation** : créer un tunnel demande à la personne de confirmer que les membres peuvent exécuter du code sur son PC.
- **Propriétaire seulement** : un tunnel peut être réservé à son propriétaire, modifiable à tout moment.
- **Relais borné** : 16 flux, trames de 64 Kio, en-têtes de 16 Kio, messages de 8 Mio, corps de 16 Mio en requête et 64 Mio en réponse, délais, une connexion par jeton.
- **Audit** : Administration > Instance liste les tunnels créés, rattachés, connectés, déconnectés (avec durée et octets), révoqués, les types refusés, les changements « propriétaire seulement » et de réglages, conservés pendant la durée de rétention. Les jetons n'y figurent jamais ; les refus de jetons inconnus à la connexion sont dans les journaux du serveur, pas dans l'audit.
- Un seul processus backend : l'état des tunnels est en mémoire. Un redémarrage déconnecte les agents, qui se reconnectent seuls avec un délai croissant.

## Proxy inverse

Les agents se connectent à `wss://<votre instance>/api/tunnels/agent` (une montée en WebSocket, authentifiée par le jeton de tunnel dans l'en-tête `Authorization`). Les fichiers Caddy et Traefik par défaut de ce guide transmettent déjà les WebSockets. Comme pour l'édition en direct, un proxy qui coupe les connexions inactives doit être réglé : l'instance envoie un ping toutes les **15 secondes** et ferme un tunnel silencieux après 45 secondes, donc un délai de lecture du proxy de plus d'environ une minute convient normalement, mais un délai plus court provoque des déconnexions périodiques.

- **Traefik** : délais du point d'entrée comme dans [Délais et connexions longues](/fr/docs/host/install/#délais-et-connexions-longues) (`readTimeout=0`, `idleTimeout=300s`).
- **nginx** : `proxy_http_version 1.1;`, les en-têtes `Upgrade` et `Connection`, et `proxy_read_timeout 3600s; proxy_send_timeout 3600s;` sur `/api/tunnels/agent`. Aucune limite de taille de corps de requête ne s'applique au tunnel lui-même.
- **Cloudflare** : un WebSocket proxifié inactif pendant 100 secondes est fermé ; le ping de 15 secondes le maintient. Ne mettez pas ce chemin en tampon et ne le réécrivez pas.

L'agent doit aussi joindre votre instance en HTTPS depuis le réseau de la personne ; rien d'autre de votre côté.

## Sans rapport avec les appels

Le relais TURN des [appels vocaux](/fr/docs/use/features/voice-calls/) est une autre affaire : les tunnels n'utilisent ni UDP, ni TURN, ni port supplémentaire, seulement le point d'accès HTTPS existant.

## Diagnostiquer

- Un agent qui reçoit `404` à la connexion : les tunnels sont désactivés (interrupteur ou `LOOTY_TUNNELS`).
- `401` : jeton inconnu, révoqué, expiré ou compte suspendu ; l'agent s'arrête et la personne crée un nouveau tunnel.
- Déconnexions répétées toutes les une à deux minutes : délais du proxy (ci-dessus).
- Le statut (connecté, latence, version de l'agent, services déclarés, octets) est dans la fenêtre du projet ; l'audit et les journaux du serveur donnent l'historique.

Conception et protocole : `docs/adr/0020-pc-tunnel.md` et `docs/tunnel-protocol.md` dans le dépôt.
