---
title: "Google"
description: "Sign in with Google accounts."
---

1. In the [Google Cloud console](https://console.cloud.google.com/), create a project, configure the **OAuth consent screen**, then *Credentials > Create credentials > OAuth client ID*, type **Web application**.
2. Authorized redirect URI: the callback URL (see [OpenID Connect](/docs/host/sign-in/oidc/)).
3. Copy the client ID and secret.

```sh
OIDC_ISSUER=https://accounts.google.com
OIDC_CLIENT_ID=<id>.apps.googleusercontent.com
OIDC_CLIENT_SECRET=<secret>
```

:::caution
Any Google account can reach the sign-in page. Restrict the consent screen to *Internal* (Google Workspace) or keep the app in testing with a list of users, otherwise anyone with a Google account could create an account on your instance.
:::
