---
title: "Google"
description: "Se connecter avec des comptes Google."
---

1. Dans la [console Google Cloud](https://console.cloud.google.com/), créez un projet, configurez l’**écran de consentement OAuth**, puis *Identifiants > Créer des identifiants > ID client OAuth*, type **Application Web**.
2. URI de redirection autorisée : l’URL de callback (voir [OpenID Connect](/fr/docs/host/sign-in/oidc/)).
3. Copiez l’identifiant et le secret du client.

```sh
OIDC_ISSUER=https://accounts.google.com
OIDC_CLIENT_ID=<id>.apps.googleusercontent.com
OIDC_CLIENT_SECRET=<secret>
```

:::caution
N’importe quel compte Google peut atteindre la page de connexion. Limitez l’écran de consentement à *Interne* (Google Workspace) ou gardez l’app en test avec une liste d’utilisateurs, sinon toute personne ayant un compte Google pourrait créer un compte sur votre instance.
:::
