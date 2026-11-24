# Local accounts, two-step verification, first-launch guide and diagnostic

GLOSSARY: *Compte local*, *Double authentification*, *Assistant de premier lancement*, *Diagnostic*. Issues 162, 173, 163, 164 (web 0.45.0). Looty still prefers OpenID Connect; local accounts exist for installs that cannot run a provider.

## Local accounts (issue 162)

Decisions:
- **Off by default**, switched on by `LOOTY_LOCAL_ACCOUNTS=true` (never in the demo). Works next to OpenID Connect; an account is local when it has a password hash. Sign-up is **closed** unless `LOOTY_LOCAL_SIGNUP=open`; administrators invite from the administration, and the person picks a password from a one-use link (7 days). The invitation link counts as proof of the address.
- **Passwords**: argon2id (library defaults, 19 MiB, 2 passes), at least 12 characters, at most 256 bytes, refused if they are the email address, all digits, made of at most three distinct characters, or a common word from an embedded offline list, with or without trailing digits and symbols, or repeated. No composition rules (they do not help). Verification of an unknown address still hashes against a dummy, so response time does not tell whether an account exists.
- **Attempt limits** in the database (`login_attempts`): five failures per address in 15 minutes lock that address, 30 per IP lock the IP; the lock answers the same for existing and unknown accounts. The IP is the **last** `X-Forwarded-For` value (the one our proxy appended; earlier values are client controlled). Without a proxy header all visitors share one bucket: the documentation tells hosts to set the header.
- **Email**: confirmation of the address at open sign-up, and password reset, by one-use links of which only the SHA-256 is stored (`local_tokens`). When no SMTP is configured, sign-up still works and an administrator confirms addresses by hand, and copies reset and invitation links from the administration. Resetting closes the person's other sessions (found by their identifier inside the session data); changing the password keeps the current one. A reset also proves the address.
- **No enumeration**: sign-up on an existing address, and "forgot password" on an unknown one, answer as if they worked.
- **The first account** of an empty instance becomes administrator, **only with a setup code** printed in the backend logs at start (random per start, compared in constant time, counted in the attempt limits). Safest default: without it, anyone reaching a freshly deployed instance before its owner could take it over. OpenID Connect first sign-ins are unchanged.
- The administration shows the warning that local accounts are not recommended.
- Tokens never reach any of this (`/api/auth/*`, `/api/admin/local/*`, `/api/setup/*` are refused to API tokens), since a token could otherwise mint reset links or change factors.

## Two-step verification (issue 173)

- TOTP (RFC 6238, SHA-1, 6 digits, 30 s, one step of tolerance either side) written in `local_auth.rs` with the vectors of the RFC in tests; no QR code (no new front-end dependency): the key is shown for manual entry and as an `otpauth://` link. Adding a QR image is left for later.
- The secret is stored encrypted with the instance key (`crypto.rs`). A code is accepted once (`totp_last_step`). Ten recovery codes (50 bits each), stored as SHA-256 hashes, single use; new codes need the password and a code.
- **Required for administrators**, and for everyone when an administrator turns on "require for everyone". Enforced at sign-in: after the password the person gets a half-open session that can only do the enrolment (`pending_enrol`, 10 minutes); the real session starts after the first code is confirmed. A required person cannot turn it off. An administrator can remove someone's second factor (lost device); this closes their sessions.
- A person promoted to administrator while already signed in keeps their session until they next sign in; the requirement applies at the next sign-in.
- Desktop and Android sign in through the system browser and the web login page, so they get the code step with no app change. OpenID Connect accounts are untouched (their provider owns the second factor).

## First-launch guide (issue 163)

`/setup`, for administrators, until `global_settings.setup_completed_at` is set ("Skip" or "Finish"). The migration sets it on instances that already have accounts, so only new instances see it. Steps: sign-in methods, instance name / default language / time zone, SMTP (the existing email page, with its test email), push (nothing to configure: keys are generated; it states whether the page uses HTTPS), first space and invitation links, link to the diagnostic. It is never offered in the demo nor on a development instance (`LOOTY_DEV_LOGIN=true`, which the end-to-end tests use). Every step is an existing administration page, so each can be redone there; `/setup` itself stays reachable.
- The first administrator is created by the sign-in step that already exists (OpenID Connect, or a local account with the setup code); the guide begins once they are in.
- Default language and time zone are stored on `global_settings` and given to accounts created afterwards (OpenID Connect and local); existing accounts are untouched.

## Diagnostic (issue 164)

`GET /api/admin/doctor` and `backend doctor [--json]` (installed as `looty` in the image, so `docker compose exec backend looty doctor`). Server checks: `BASE_URL` shape and scheme, the public address answers with a valid certificate and the same version, proxy headers as the request carried them, CalDAV discovery, each active OpenID Connect provider (discovery, issuer, the return address to register), SMTP connection (login, no message; a test message uses the existing button), database version and size, uploads writable and free space, secret key, update available (only when `LOOTY_UPDATE_CHECK` is on, as before). The page adds the browser checks: address bar against `BASE_URL`, HTTPS, WebSocket on `/api/ws` and `/collab`.
- A server that cannot reach its own public address only gets a warning (hairpin NAT inside containers is normal); a certificate error is an error.
- The command does not run migrations, exits 1 if any check is an error. Every warning and error links to the troubleshooting page of the documentation.
