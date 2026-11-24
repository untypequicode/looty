---
title: "Zitadel"
description: "Créer un client Looty dans Zitadel."
---

1. Dans Zitadel, créez un **projet**, puis une **application** de type *Web*, méthode d’authentification **Code** (PKCE ou basic).
2. URI de redirection : l’URL de callback (voir [OpenID Connect](/fr/docs/host/sign-in/oidc/)). Ajoutez votre `BASE_URL` comme URI de déconnexion si demandé.
3. Dans les réglages, assurez-vous que le jeton contient l’e-mail de l’utilisateur (*User Info inside ID Token* sur l’application).
4. Copiez l’**identifiant** et le **secret** du client dans `.env`.

```sh
OIDC_ISSUER=https://<votre-instance>.zitadel.cloud
OIDC_CLIENT_ID=<identifiant du client>
OIDC_CLIENT_SECRET=<secret du client>
```

L’issuer est l’adresse de base de votre Zitadel (sans chemin).
