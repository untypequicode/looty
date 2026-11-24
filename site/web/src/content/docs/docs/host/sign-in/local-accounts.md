---
title: "Local accounts (not recommended)"
description: "Why Looty has no built-in passwords, and what to use instead."
---

Looty has **no built-in password accounts**: people always sign in through an OpenID Connect provider. This is on purpose: Looty does not store passwords, reset flows or second factors, and a dedicated identity provider does it better.

If you have no provider, **use the [bundled Pocket ID](/docs/host/sign-in/pocket-id/)**: it is a few extra lines in the same compose file and gives you passkeys.

:::danger
`LOOTY_DEV_LOGIN=true` is a **development** switch that lets anyone sign in as any email address. It is not a local-account feature. **Never enable it on a real instance.**
:::
