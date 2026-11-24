---
title: "Authentik"
description: "Créer un fournisseur et une application Looty dans Authentik."
---

1. *Applications > Providers > Create* : **OAuth2/OpenID Provider**, type de client **Confidential**.
2. URI de redirection (strict) : l’URL de callback (voir [OpenID Connect](/fr/docs/host/sign-in/oidc/)). Gardez les portées par défaut (`openid`, `email`, `profile`).
3. Créez une **Application** qui utilise ce fournisseur et donnez l’accès aux bonnes personnes ou groupes.
4. Copiez l’identifiant et le secret du client.

```sh
OIDC_ISSUER=https://authentik.example.org/application/o/<slug-de-l-application>/
OIDC_CLIENT_ID=<identifiant du client>
OIDC_CLIENT_SECRET=<secret du client>
```

Utilisez comme issuer l’**OpenID Configuration Issuer** affiché sur la page du fournisseur, barre oblique finale comprise.
