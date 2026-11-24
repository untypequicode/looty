---
title: "Keycloak"
description: "Create a Looty client in Keycloak."
---

1. In your realm, *Clients > Create client*: type **OpenID Connect**, client ID `looty`.
2. Turn **Client authentication** on (confidential) and keep the **standard flow**.
3. Valid redirect URIs: the callback URL (see [OpenID Connect](/docs/host/sign-in/oidc/)).
4. In *Credentials*, copy the **client secret**.

```sh
OIDC_ISSUER=https://keycloak.example.org/realms/<realm>
OIDC_CLIENT_ID=looty
OIDC_CLIENT_SECRET=<client secret>
```

Users need an email address in Keycloak (turn on *Email verified* if your policy requires it).
