---
title: "Zitadel"
description: "Create a Looty client in Zitadel."
---

1. In Zitadel, create a **project**, then an **application** of type *Web*, authentication method **Code** (PKCE or basic).
2. Redirect URI: the callback URL (see [OpenID Connect](/docs/host/sign-in/oidc/)). Add your `BASE_URL` as post-logout URI if asked.
3. Under the project settings, make sure the token contains the user's email (*User Info inside ID Token* on the application).
4. Copy the **client ID** and **client secret** into `.env`.

```sh
OIDC_ISSUER=https://<your-instance>.zitadel.cloud
OIDC_CLIENT_ID=<client id>
OIDC_CLIENT_SECRET=<client secret>
```

The issuer is the base address of your Zitadel (without a path).
