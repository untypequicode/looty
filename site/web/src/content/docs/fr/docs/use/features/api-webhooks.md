---
title: "API, jetons d’accès et webhooks"
description: "Relier des scripts, n8n ou des bots à Looty avec un jeton d’accès personnel, et recevoir des webhooks signés."
---

Looty a une API HTTP documentée pour les intégrations. La spécification **OpenAPI** des routes stables est servie par votre instance sur `/api/openapi.json` (à importer dans Postman, Insomnia ou un générateur de client).

## Jetons d’accès personnels

Créez-en un dans **Réglages > Sécurité et accès > Jetons d’API**. Envoyez-le avec `Authorization: Bearer looty_...`.

```sh
curl -H "Authorization: Bearer looty_..." https://looty.example.org/api/workspaces
```

Un jeton agit en votre nom et **n’a jamais plus de droits que vous** : si vous perdez l’accès à un espace, le jeton aussi. Il peut être plus étroit :

- **Lecture seule** : seules les requêtes `GET` sont acceptées.
- **Espaces choisis** : il n’atteint que les espaces cochés ; le reste répond `403 token_workspace_scope`. Un jeton limité à des espaces ne peut pas utiliser les routes qui ne dépendent pas d’un espace (courrier, réglages…).
- **Expiration** : passé la date, il répond `401 token_expired`.

Un jeton ne peut jamais créer ni révoquer de jetons, ni demander la suppression de votre compte. Révoquez un jeton depuis la même page : il cesse de fonctionner immédiatement. Les jetons sont distincts des mots de passe d’application utilisés pour CalDAV.

Le jeton n’est affiché qu’une fois, à la création : seule son empreinte est conservée.

## Webhooks

Les administrateurs d’un espace ajoutent des webhooks dans **Réglages > Webhooks** : une adresse `https://` et les événements à envoyer.

| Événement | Envoyé quand |
| --- | --- |
| `task.created` | une tâche est créée |
| `task.updated` | une tâche change ou se déplace |
| `task.completed` | une tâche passe à terminée |
| `page.updated` | le titre, l’icône, la place ou le contenu d’un document est enregistré par l’application ou l’API (pas chaque frappe de l’édition en direct) |
| `event.created` | un événement du calendrier est créé |
| `announcement.published` | une annonce est publiée |

Les éléments personnels ne sont jamais envoyés, et un espace perso n’a pas de webhooks.

Chaque livraison est un `POST` avec un corps JSON `{ "id", "event", "workspace_id", "created_at", "data" }` et ces en-têtes :

- `X-Looty-Event`, `X-Looty-Delivery` (identifiant unique de la livraison, pour ignorer les doublons)
- `X-Looty-Timestamp` (secondes depuis 1970)
- `X-Looty-Signature: sha256=<hex>`, le HMAC-SHA256 de `<horodatage>.<corps brut>` avec le secret affiché à la création du webhook.

Vérifiez la signature et refusez les horodatages trop anciens (par exemple plus de 5 minutes). Une réponse `2xx` vaut livraison réussie. Sinon Looty réessaie après 1 minute, 5 minutes, 30 minutes, 2 heures et 12 heures, puis abandonne ; dix livraisons abandonnées d’affilée désactivent le webhook. Le **journal des livraisons** (30 jours) montre chaque tentative, et **Envoyer un test** envoie un `ping`.

Looty n’appelle pas les adresses du réseau privé (localhost, 10.x, 192.168.x…) sauf si l’administrateur de l’instance définit `WEBHOOKS_ALLOW_PRIVATE=true`.
