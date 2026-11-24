---
title: "Authentik"
description: "Create a Looty provider and application in Authentik."
---

1. *Applications > Providers > Create*: **OAuth2/OpenID Provider**, client type **Confidential**.
2. Redirect URI (strict): the callback URL (see [OpenID Connect](/docs/host/sign-in/oidc/)). Keep the default scopes (`openid`, `email`, `profile`).
3. Create an **Application** that uses this provider and give the right users or groups access.
4. Copy the client ID and secret.

```sh
OIDC_ISSUER=https://authentik.example.org/application/o/<application-slug>/
OIDC_CLIENT_ID=<client id>
OIDC_CLIENT_SECRET=<client secret>
```

Use the **OpenID Configuration Issuer** shown on the provider page as the issuer, including the trailing slash.
