---
title: "Dépannage"
description: "Problèmes de WebSockets, de collaboration et de CalDAV."
---

## Les mises à jour en direct ou l’édition de documents ne marchent pas

Les deux utilisent des WebSockets. Vérifiez que votre proxy autorise les WebSockets sur `/api/ws` et `/collab`, et que `/collab` va au service de collaboration (port 8082) avec le **préfixe retiré**. Derrière un CDN ou un proxy aux délais courts, augmentez le délai d’inactivité des WebSockets.

## Les documents restent vides ou « hors ligne »

Regardez `docker compose logs collaboration` : le service joint le backend par `BACKEND_URL` (`http://backend:8080` par défaut) et la base par `DATABASE_URL`.

## La connexion redirige vers une erreur

L’URL de redirection chez votre fournisseur doit être exactement `<BASE_URL>/api/auth/callback/<OIDC_PROVIDER_ID>`, et `BASE_URL` doit être l’adresse publique en `https://`.

## CalDAV (téléphone, Thunderbird, DAVx5)

`/dav` et `/.well-known/caldav` doivent être routés vers le backend (port 8080). Utilisez un **mot de passe d’application** généré dans Looty, pas votre mot de passe d’authentification unique. S’abonner depuis Looty à un serveur CalDAV de votre réseau local demande `CALDAV_ALLOW_PRIVATE=true`.

## Une fonction externe est « refusée »

Jupyter, CalDAV et les fournisseurs d’IA en réseau privé ou en http sont refusés par défaut : voir [services](/fr/docs/host/services/).
