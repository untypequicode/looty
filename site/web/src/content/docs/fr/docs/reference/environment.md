---
title: "Variables d’environnement"
description: "Toutes les variables d’environnement du backend, de l’interface web et du service de collaboration de Looty : rôle, valeur par défaut, obligatoire ou non, exemple et version."
---

<!-- Généré par scripts/env-reference.mjs à partir de docs/env-vars.json. Ne pas modifier à la main. -->

Les réglages se font dans le fichier `.env`, à côté du fichier compose. La colonne **Compose** indique si les fichiers compose officiels transmettent la variable de `.env` au conteneur ; pour les autres, ajoutez-la sous `environment:` du service dans un `docker-compose.override.yml`. Une variable vide compte comme non définie.

Cette page est générée à partir de `docs/env-vars.json` et vérifiée en CI contre le code : ajouter une variable sans la documenter fait échouer la CI.

## Déploiement

| Variable | Service | Rôle | Défaut | Obligatoire | Exemple | Depuis | Compose |
|---|---|---|---|---|---|---|---|
| `APP_NAME` | docker compose | Nom de la pile dans Traefik (noms des routeurs). Un mot court. | – | Traefik only | `looty` | 0.1.0 | oui |
| `APP_DOMAIN` | docker compose | Nom de domaine de l'instance. Son DNS doit pointer vers la machine. | – | oui | `looty.example.org` | 0.1.0 | oui |
| `TRAEFIK_NETWORK` | docker compose | Traefik seulement, avec le générateur ou install.sh --traefik-network : nom du réseau Docker externe existant par lequel Traefik joint la pile. Les services y sont rattachés. | – | non | `traefik-public` | 0.50.0 | oui |
| `TRAEFIK_ENTRYPOINT` | docker compose | Traefik seulement, avec le générateur ou install.sh --traefik-entrypoint : point d'entrée de votre Traefik qui sert les routeurs (en général celui de HTTPS). | – | non | `websecure` | 0.50.0 | oui |
| `TRAEFIK_CERT_RESOLVER` | docker compose | Traefik seulement, avec le générateur ou install.sh --traefik-resolver : résolveur de certificats de votre Traefik pour les routeurs. | – | non | `letsencrypt` | 0.50.0 | oui |
| `LOOTY_VERSION` | docker compose | Version des images Docker : « latest » par défaut, ou un numéro de version publié pour rester sur une version précise. | `latest` | non | `0.33.0` | 0.10.0 | oui |
| `BASE_URL` | backend | Adresse publique de l'instance (https://domaine, sans « / » final). Sert aux redirections de connexion, aux e-mails et aux liens. | `http://localhost:3000` | oui | `https://looty.example.org` | 0.1.0 | oui |

## Base de données

| Variable | Service | Rôle | Défaut | Obligatoire | Exemple | Depuis | Compose |
|---|---|---|---|---|---|---|---|
| `LOOTY_DB_DATA` | docker compose | Dossier de l'hôte où PostgreSQL stocke ses données. Un chemin absolu les place ailleurs. | `./data/postgres` | non | `/srv/looty/postgres` | 0.30.0 | oui |
| `DB_NAME` | docker compose | Nom de la base PostgreSQL. | `looty_db` | non | `looty_db` | 0.1.0 | oui |
| `DB_USER` | docker compose | Utilisateur PostgreSQL. | `postgres` | non | `looty_user` | 0.1.0 | oui |
| `DB_PASSWORD` | docker compose | Mot de passe PostgreSQL. À changer. | `postgres` | oui | `a long random password` | 0.1.0 | oui |
| `DATABASE_URL` | backend, collaboration | Adresse de connexion PostgreSQL. Les fichiers compose la fabriquent à partir de DB_USER, DB_PASSWORD et DB_NAME. Le service de collaboration retombe sur une base de développement locale. | – | oui | `postgres://user:pass@postgres:5432/looty_db` | 0.1.0 | non |

## Connexion (OpenID Connect)

| Variable | Service | Rôle | Défaut | Obligatoire | Exemple | Depuis | Compose |
|---|---|---|---|---|---|---|---|
| `OIDC_ISSUER` | backend | Adresse de votre fournisseur OpenID Connect (issuer). Lue seulement au tout premier démarrage ; ensuite, le fournisseur se modifie dans l'administration. | – | oui | `https://zitadel.example.com` | 0.1.0 | oui |
| `OIDC_CLIENT_ID` | backend | Identifiant du client Looty chez le fournisseur. | – | oui | `looty` | 0.1.0 | oui |
| `OIDC_CLIENT_SECRET` | backend | Secret du client Looty chez le fournisseur. | – | oui | – | 0.1.0 | oui |
| `OIDC_PROVIDER_ID` | backend | UUID fixe du fournisseur, pour connaître l'URL de redirection (BASE_URL/api/auth/callback/UUID) avant le premier démarrage. Pris en compte une seule fois, à la création du fournisseur. Générer avec : uuidgen | `random UUID` | no (yes with bundled Pocket ID) | `0b8e6c1e-5d3a-4c1f-9a77-2f6a1d4b9e10` | 0.32.0 | oui |
| `OIDC_CA_FILE` | backend | Fichier PEM d'une autorité de certification interne à accepter pour le fournisseur. À monter dans le conteneur backend. | – | non | `/certs/ca.pem` | 0.32.0 | non |
| `AUTH_DOMAIN` | docker compose | Sous-domaine dédié du Pocket ID inclus. Son DNS pointe vers la même machine. | – | bundled Pocket ID only | `auth.looty.example.org` | 0.32.0 | oui |
| `POCKET_ID_ENCRYPTION_KEY` | docker compose | Clé de chiffrement du Pocket ID inclus. | – | bundled Pocket ID only | `openssl rand -base64 32` | 0.32.0 | oui |
| `POCKET_ID_STATIC_API_KEY` | docker compose | Clé d'administration utilisée par pocket-id-init pour créer le client Looty. Peut être retirée ensuite. | – | bundled Pocket ID only | `openssl rand -hex 32` | 0.32.0 | oui |
| `LOOTY_LOCAL_ACCOUNTS` | backend | Active les comptes locaux (e-mail et mot de passe, double authentification facultative) en plus ou à la place d'OpenID Connect. Déconseillé : un fournisseur d'identité fait mieux. Désactivé par défaut. Voir aussi LOOTY_LOCAL_SIGNUP. | `false` | non | `true` | 0.45.0 | oui |
| `LOOTY_LOCAL_SIGNUP` | backend | Qui peut créer un compte local : closed (par défaut, un administrateur invite) ou open (toute personne qui connaît l'adresse de l'instance, puis confirme son e-mail). Le premier compte d'une instance vide devient administrateur avec le code affiché dans les journaux du backend. | `closed` | non | `open` | 0.45.0 | oui |

## Sécurité et réseau

| Variable | Service | Rôle | Défaut | Obligatoire | Exemple | Depuis | Compose |
|---|---|---|---|---|---|---|---|
| `LOOTY_SECRET_KEY` | backend | Clé qui chiffre les secrets stockés (mots de passe CalDAV, clés d'IA, jetons Jupyter). À garder avec les sauvegardes : sans elle, ces secrets sont perdus. | `generated in uploads/.secret.key` | recommended | `openssl rand -base64 32` | 0.1.0 | oui |
| `LOOTY_DEV_LOGIN` | backend | Développement seulement : permet à n'importe qui de se connecter sous n'importe quelle adresse e-mail. À ne JAMAIS activer sur une vraie instance. | `false` | non | `false` | 0.1.0 | non |
| `CALDAV_ALLOW_PRIVATE` | backend | Autoriser les serveurs CalDAV en réseau privé ou en http (par exemple un Nextcloud interne). | `false` | non | `true` | 0.1.0 | oui |
| `JUPYTER_ALLOW_PRIVATE` | backend | Autoriser les serveurs Jupyter des notebooks en réseau privé ou en http. | `false` | non | `true` | 0.29.0 | oui |
| `LOOTY_TUNNELS` | backend | Active les tunnels PC (mettre on) : chacun peut relier son propre ordinateur à un projet, comme serveur Jupyter par exemple, par une connexion sortante et sans port ouvert. Désactivé par défaut. | `off` | non | `on` | 0.57.0 | oui |
| `AI_ALLOW_PRIVATE` | backend | Autoriser les fournisseurs d'IA en réseau privé ou en http (par exemple Ollama sur la même machine). Aussi autorisé quand CALDAV_ALLOW_PRIVATE vaut true. | `false` | non | `true` | 0.34.1 | non |
| `PUSH_ALLOW_PRIVATE` | backend | Tests seulement : accepte un service de push local (http, réseau privé). | `false` | non | `false` | 0.1.0 | non |
| `LOOTY_METRICS_TOKEN` | backend | Active le point /metrics pour Prometheus (aussi /api/metrics), lu avec l'en-tête Authorization: Bearer <jeton>. 16 caractères au moins ; sans lui le point n'existe pas. | – | non | `a-long-random-secret` | 0.43.0 | non |
| `WEBHOOKS_ALLOW_PRIVATE` | backend | Autorise les webhooks d'espace à appeler des adresses du réseau privé (localhost, 10.x, 192.168.x…) et http simple, par exemple un n8n sur le même réseau. Par défaut les webhooks n'atteignent que des adresses https publiques. | `false` | non | `true` | 0.44.0 | non |

## E-mails

| Variable | Service | Rôle | Défaut | Obligatoire | Exemple | Depuis | Compose |
|---|---|---|---|---|---|---|---|
| `SMTP_HOST` | backend | Serveur d'envoi des e-mails. Repli : mieux vaut configurer Administration > E-mail. Sans SMTP_HOST et SMTP_FROM, l'envoi est désactivé. | – | non | `smtp.example.org` | 0.1.0 | oui |
| `SMTP_FROM` | backend | Adresse de l'expéditeur. | – | with SMTP_HOST | `looty@example.org` | 0.1.0 | oui |
| `SMTP_FROM_NAME` | backend | Nom de l'expéditeur. | `Looty` | non | `Looty` | 0.1.0 | oui |
| `SMTP_PORT` | backend | Port du serveur d'e-mails. | `587 (starttls), 465 (tls), 25 (none)` | non | `587` | 0.1.0 | oui |
| `SMTP_SECURITY` | backend | Sécurité de la connexion : starttls, tls ou none. | `starttls` | non | `tls` | 0.1.0 | oui |
| `SMTP_USERNAME` | backend | Utilisateur du serveur d'e-mails. | – | non | `looty` | 0.1.0 | oui |
| `SMTP_PASSWORD` | backend | Mot de passe du serveur d'e-mails. | – | non | – | 0.1.0 | oui |

## Intégrations et fonctions

| Variable | Service | Rôle | Défaut | Obligatoire | Exemple | Depuis | Compose |
|---|---|---|---|---|---|---|---|
| `CALENDAR_DEFAULT_TZ` | backend | Fuseau horaire par défaut des agendas, des statistiques et des sauvegardes (nom IANA). | `UTC` | non | `Europe/Paris` | 0.1.0 | non |
| `MOODLE_POLL_SECONDS` | backend | Délai entre deux interrogations de Moodle pour les notifications (minimum 10). | `300` | non | `600` | 0.8.0 | non |
| `MOODLE_CACHE_MB` | backend | Taille totale du cache hors ligne de Moodle, en Mo. | `500` | non | `500` | 0.22.2 | non |
| `MOODLE_CACHE_FILE_MB` | backend | Plus gros fichier Moodle conservé dans le cache hors ligne, en Mo. | `25` | non | `25` | 0.22.2 | non |
| `SITES_BASE_URL` | backend | Adresse qui sert les petits sites web des projets, si elle diffère de BASE_URL. | `BASE_URL` | non | `https://sites.looty.example.org` | 0.10.0 | non |
| `SITES_SANDBOX` | backend | Servir les sites web des projets dans un bac à sable. Mettre false pour le désactiver. | `true` | non | `false` | 0.10.0 | non |
| `LATEX_BIN_DIR` | backend | Dossier du compilateur LaTeX, quand il n'est pas dans le PATH. | `PATH` | non | `/usr/local/texlive/bin` | 0.10.0 | non |
| `BACKUP_DIR` | backend | Dossier des sauvegardes faites depuis l'administration. | `backups` | non | `/app/backups` | 0.7.0 | non |

## Démo publique

| Variable | Service | Rôle | Défaut | Obligatoire | Exemple | Depuis | Compose |
|---|---|---|---|---|---|---|---|
| `LOOTY_DEMO` | backend | Démo publique : remplace la connexion par « Entrer dans la démo », une copie jetable par visiteur. Jamais sur une vraie instance ; utilisez docker-compose.demo.yml. | `false` | non | `false` | 0.30.0 | oui |
| `LOOTY_DEMO_MAX_COPIES` | backend | Démo : copies simultanées. | `30` | non | `30` | 0.30.0 | oui |
| `LOOTY_DEMO_COPIES_PER_IP_HOUR` | backend | Démo : copies par adresse et par heure (le reverse proxy doit envoyer X-Forwarded-For). | `5` | non | `5` | 0.30.0 | oui |
| `LOOTY_DEMO_MAX_FILE_MB` | backend | Démo : taille d'un envoi, en Mo. | `2` | non | `2` | 0.30.0 | oui |
| `LOOTY_DEMO_MAX_COPY_MB` | backend | Démo : taille totale par copie, en Mo. | `20` | non | `20` | 0.30.0 | oui |
| `LOOTY_DEMO_RESET_HOUR` | backend | Démo : heure (0 à 23, dans CALENDAR_DEFAULT_TZ, sinon UTC) de la remise à zéro complète de chaque nuit, qui supprime toutes les copies même non expirées. Toute autre valeur, comme off, la désactive. | `3` | non | `3` | 0.50.0 | oui |

## Avancé

| Variable | Service | Rôle | Défaut | Obligatoire | Exemple | Depuis | Compose |
|---|---|---|---|---|---|---|---|
| `PORT` | backend, collaboration | Port d'écoute du service. Les images Docker le fixent déjà. | `8080 (backend), 8082 (collaboration)` | non | `8080` | 0.1.0 | non |
| `LOOTY_UPDATE_CHECK` | backend | Laisse la page d'état de l'instance demander à GitHub (au plus toutes les 6 heures) s'il existe une version web plus récente. Désactivé par défaut : le serveur ne sort pas vers l'extérieur pour cela. | `false` | non | `true` | 0.43.0 | non |
| `LOOTY_STUN_URLS` | backend | Serveurs STUN (séparés par des virgules) que les navigateurs utilisent pour trouver un chemin direct pour les appels vocaux. Le serveur STUN ne voit que les adresses IP des participants, jamais l'audio. Mettez le vôtre pour éviter le serveur public. | `stun:stun.l.google.com:19302` | non | `stun:stun.example.org:3478` | 0.54.0 | non |
| `LOOTY_CALL_MAX_PARTICIPANTS` | backend, collaboration | Nombre maximal initial de personnes dans un appel vocal (2 à 12), utilisé tant que l’administrateur n’a pas fixé de valeur dans Administration > Instance > Appels. Les appels sont de pair à pair : chacun envoie son audio à tous les autres, donc les gros appels consomment davantage de bande passante chez les participants. | `6` | non | `4` | 0.54.0 | non |
| `LOOTY_TURN_URLS` | backend | Adresses du relais TURN (séparées par des virgules, turn: ou turns:) proposées aux navigateurs pour les appels derrière un pare-feu restrictif. Demande LOOTY_TURN_SECRET. Réglée par l'installateur avec --turn. | – | non | `turn:looty.example.org:3478?transport=udp,turn:looty.example.org:3478?transport=tcp` | 0.54.0 | non |
| `LOOTY_TURN_SECRET` | backend | Secret partagé avec coturn (`static-auth-secret`, `use-auth-secret`). Looty s'en sert pour donner à chaque personne connectée un accès TURN temporaire (valable LOOTY_TURN_TTL secondes) ; le secret lui-même n'arrive jamais dans les navigateurs. | – | non | `(random, 32+ characters)` | 0.54.0 | non |
| `LOOTY_TURN_TTL` | backend | Validité des accès TURN temporaires, en secondes (300 à 604800). Un appel plus long perd le relais (les connexions directes ne sont pas touchées). | `86400` | non | `3600` | 0.54.0 | non |
| `TURN_SECRET` | docker compose | Secret partagé du service coturn facultatif (écrit par l'installateur avec --turn). Transmis au backend comme LOOTY_TURN_SECRET. | – | non | – | 0.54.0 | non |
| `TURN_EXTERNAL_IP` | docker compose | Adresse IP publique du serveur quand il est derrière un NAT (instances cloud) : coturn l'annonce comme adresse du relais. | – | non | `203.0.113.10` | 0.54.0 | non |
| `TURN_MIN_PORT` | docker compose | Premier port UDP de la plage de relais de coturn. Gardez la plage petite (environ 20 ports par participant relayé simultané) et ouvrez-la sur le pare-feu. | `49160` | non | `49160` | 0.54.0 | non |
| `TURN_MAX_PORT` | docker compose | Dernier port UDP de la plage de relais de coturn. | `49200` | non | `49200` | 0.54.0 | non |
| `ACCOUNT_DELETION_GRACE_DAYS` | backend | Jours entre la demande de suppression d'un compte et sa suppression effective (1 à 90). D'ici là, la personne peut annuler. | `14` | non | `30` | 0.43.0 | non |
| `LOOTY_SITE_URL` | backend | Adresse du site où les applications cherchent les mises à jour (/api/releases), à la place du site officiel. | `address in releases.json` | non | `https://looty.example.org` | 0.16.7 | non |
| `BACKEND_URL` | interface web, collaboration | Adresse interne du backend, pour l'interface web et le service de collaboration. Les fichiers compose la fixent. | `http://backend:8080` | non | `http://backend:8080` | 0.1.0 | non |
| `COLLAB_URL` | interface web | Adresse interne du service de collaboration, pour l'interface web (/collab). | `http://collaboration:8082` | non | `http://collaboration:8082` | 0.1.0 | non |
