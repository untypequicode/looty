---
title: "Héberger Looty"
description: "Auto-héberger Looty : prérequis, installation, connexion, exploitation et sécurité."
---

Looty ne propose pas d’hébergement : **cette documentation est le chemin pour avoir votre propre instance**. Ce sont trois petits services plus PostgreSQL, livrés en images Docker pour amd64 et arm64.

| Service | Rôle |
|---|---|
| `backend` | API, fichiers, CalDAV, e-mails (Rust) |
| `frontend` | Interface web (Nuxt) |
| `collaboration` | Édition de documents en temps réel (Hocuspocus, Yjs) |

## Suivre le guide

1. [Vérifiez les prérequis](/fr/docs/host/requirements/).
2. [Installation rapide](/fr/docs/host/quick-install/), ou [installation détaillée](/fr/docs/host/install/) avec Caddy, Traefik ou un autre proxy.
3. [Choisissez comment on se connecte](/fr/docs/host/sign-in/oidc/) (OpenID Connect recommandé).
4. Configurez [e-mails, push, Moodle, Jupyter et stockage](/fr/docs/host/services/).
5. Prévoyez [sauvegardes](/fr/docs/host/operate/backup/), [mises à jour](/fr/docs/host/operate/updates/) et lisez la [liste de contrôle sécurité](/fr/docs/host/security/).

Tous les réglages sont listés dans la [référence des variables d’environnement](/fr/docs/reference/environment/).
