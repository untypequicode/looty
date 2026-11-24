---
title: "API, access tokens and webhooks"
description: "Connect scripts, n8n or bots to Looty with a personal access token, and get signed webhooks."
---

Looty has a documented HTTP API for integrations. The **OpenAPI** specification of the stable routes is served by your instance at `/api/openapi.json` (import it in Postman, Insomnia or a client generator).

## Personal access tokens

Create one in **Settings > Security & access > API tokens**. Send it as `Authorization: Bearer looty_...`.

```sh
curl -H "Authorization: Bearer looty_..." https://looty.example.org/api/workspaces
```

A token acts as you and **never has more rights than you**: if you lose access to a workspace, so does the token. It can be narrower:

- **Read only**: only `GET` requests are accepted.
- **Chosen workspaces**: it can only reach the workspaces you tick; everything else answers `403 token_workspace_scope`. A token limited to workspaces cannot use routes that are not tied to a workspace (mail, settings…).
- **Expiry**: after the date it answers `401 token_expired`.

A token can never create or revoke tokens, nor ask to delete your account. Revoke a token from the same page: it stops working immediately. Tokens are separate from the app passwords used for CalDAV.

The token is shown once, at creation: only a fingerprint is stored.

## Webhooks

Workspace administrators add webhooks in **Settings > Webhooks**: an `https://` address and the events to send.

| Event | Sent when |
| --- | --- |
| `task.created` | a task is created |
| `task.updated` | a task changes or moves |
| `task.completed` | a task becomes completed |
| `page.updated` | a document's title, icon, place or content is saved through the app or the API (not every keystroke of live editing) |
| `event.created` | a calendar event is created |
| `announcement.published` | an announcement is published |

Personal items are never sent, and a personal space has no webhooks.

Each delivery is a `POST` with a JSON body `{ "id", "event", "workspace_id", "created_at", "data" }` and these headers:

- `X-Looty-Event`, `X-Looty-Delivery` (unique id of the delivery, to ignore duplicates)
- `X-Looty-Timestamp` (seconds since 1970)
- `X-Looty-Signature: sha256=<hex>`, the HMAC-SHA256 of `<timestamp>.<raw body>` with the secret shown when you created the webhook.

Verify the signature and reject old timestamps (for example older than 5 minutes). A `2xx` answer means delivered. Otherwise Looty retries after 1 minute, 5 minutes, 30 minutes, 2 hours and 12 hours, then gives up; ten abandoned deliveries in a row switch the webhook off. The **delivery log** (30 days) shows each attempt, and **Send a test** sends a `ping`.

Looty does not call private network addresses (localhost, 10.x, 192.168.x…) unless the instance administrator sets `WEBHOOKS_ALLOW_PRIVATE=true`.
