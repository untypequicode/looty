---
title: "Keycloak"
description: "Créer un client Looty dans Keycloak."
---

1. Dans votre realm, *Clients > Create client* : type **OpenID Connect**, identifiant `looty`.
2. Activez **Client authentication** (confidentiel) et gardez le **flux standard**.
3. URI de redirection valides : l’URL de callback (voir [OpenID Connect](/fr/docs/host/sign-in/oidc/)).
4. Dans *Credentials*, copiez le **secret du client**.

```sh
OIDC_ISSUER=https://keycloak.example.org/realms/<realm>
OIDC_CLIENT_ID=looty
OIDC_CLIENT_SECRET=<secret du client>
```

Les utilisateurs doivent avoir une adresse e-mail dans Keycloak (activez *Email verified* si votre politique l’exige).
