---
title: "OpenID Connect (recommended)"
description: "How Looty signs people in with an OpenID Connect provider."
---

Looty signs people in through **OpenID Connect (OIDC)**: your organisation's identity provider, or one you run. This is the recommended method.

## What Looty needs from the provider

- A **client** for Looty (confidential, with a secret), using the authorization code flow with **PKCE**.
- Scopes `openid email profile`: Looty needs an **email address** for each account.
- Redirect (callback) URL: `<BASE_URL>/api/auth/callback/<OIDC_PROVIDER_ID>`. Set `OIDC_PROVIDER_ID` to a UUID (`uuidgen`) in `.env` to know it **before** the first start.

## Settings

```sh
OIDC_ISSUER=https://id.example.org
OIDC_CLIENT_ID=looty
OIDC_CLIENT_SECRET=...
OIDC_PROVIDER_ID=<uuid>   # optional but handy
```

Looty reads these variables **only on its very first start**; afterwards, edit the provider in *Administration > Authentication providers*. The first account to sign in becomes the administrator. A provider whose certificate comes from an internal authority needs `OIDC_CA_FILE`.

## Provider guides

[Zitadel](/docs/host/sign-in/zitadel/) · [Keycloak](/docs/host/sign-in/keycloak/) · [Authentik](/docs/host/sign-in/authentik/) · [Google](/docs/host/sign-in/google/) · [Bundled Pocket ID](/docs/host/sign-in/pocket-id/) · [Local accounts](/docs/host/sign-in/local-accounts/)
