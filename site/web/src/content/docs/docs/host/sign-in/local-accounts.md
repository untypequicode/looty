---
title: "Local accounts (not recommended)"
description: "Optional email and password accounts, with two-step verification, and why OpenID Connect is still preferred."
---

Looty prefers **OpenID Connect**: password storage, resets and second factors are better handled by a dedicated identity provider. If you have none, the easiest path is the [bundled Pocket ID](/docs/host/sign-in/pocket-id/).

Local accounts exist for installs where that is too much. They are **off by default**:

```sh
LOOTY_LOCAL_ACCOUNTS=true    # turns them on, next to OpenID Connect if you also have it
LOOTY_LOCAL_SIGNUP=closed    # closed (default: an administrator invites) or open
```

## How they behave

- **First account.** On an empty instance the first account becomes the administrator. To stop a stranger from taking it before you, the sign-in page asks for a **setup code** printed in the backend logs at start: `docker compose logs backend | grep "setup"`.
- **Sign-up is closed.** Administrators invite people in *Administration > Local accounts*: the person gets a link (by email when SMTP is set up, otherwise you copy it) to choose a password. With `LOOTY_LOCAL_SIGNUP=open`, anyone can create an account and must confirm their email address (or an administrator confirms it).
- **Passwords** are hashed with argon2id, must have 12 characters or more, and are checked offline against a list of very common passwords.
- **Attempts are limited** per account and per IP address: five failures lock the account for 15 minutes, with the same answer whether it exists or not. Put Looty behind a proxy that sets `X-Forwarded-For`, or the limit by address sees only the proxy.
- **Forgotten password.** With SMTP, a one-hour link is emailed. Without it, an administrator copies a reset link from *Local accounts*. Resetting closes the person's other sessions.
- **Two-step verification (TOTP).** Any authenticator app works. It is **required for administrators**: an administrator without it can enter their password but is held at the setup screen until it is configured. Administrators can require it for everyone, and reset it for someone who lost their device. People also get 10 single-use recovery codes. The desktop and Android apps sign in through the browser, so they get the same code step.

:::caution
`LOOTY_DEV_LOGIN=true` is a **development** switch that lets anyone sign in as any email address. It is not a local-account feature. **Never enable it on a real instance.**
:::
