---
title: "Troubleshooting"
description: "WebSockets, collaboration and CalDAV problems."
---

## Real-time updates or document editing do not work

Both use WebSockets. Check that your proxy allows WebSocket upgrades on `/api/ws` and `/collab`, and that `/collab` goes to the collaboration service (port 8082) with the **prefix stripped**. Behind a CDN or a proxy with short timeouts, raise the idle timeout for WebSockets.

## Documents stay empty or "offline"

Check `docker compose logs collaboration`: the service reaches the backend at `BACKEND_URL` (`http://backend:8080` by default) and the database through `DATABASE_URL`.

## Sign-in redirects to an error

The redirect URL at your provider must be exactly `<BASE_URL>/api/auth/callback/<OIDC_PROVIDER_ID>`, and `BASE_URL` must be the public `https://` address.

## CalDAV (phone, Thunderbird, DAVx5)

`/dav` and `/.well-known/caldav` must be routed to the backend (port 8080). Use an **app password** generated in Looty, not your single sign-on password. Subscribing to a CalDAV server on your LAN from Looty needs `CALDAV_ALLOW_PRIVATE=true`.

## An external feature is "refused"

Jupyter, CalDAV and AI providers on a private network or over http are refused by default: see [services](/docs/host/services/).
