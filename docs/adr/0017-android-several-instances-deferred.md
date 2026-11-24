# Android: several instances in one app is specified but not built yet

GLOSSARY: *Instance de l'app*. Issue 143 (child of 136).

The app knows one instance (`Config.server`, the `looty` preferences, one offline store, one set of widgets, UnifiedPush registration and DAVx5 account). Supporting several is a data-model change across all of them, and the issue itself says the open questions (offline database, widgets, DAVx5) must be settled before specifying. Nothing is built in android 0.30.0 and issue 143 stays open.

## Decision for when it is built

- **One "connection" record per instance and account** (origin, account email, session, UnifiedPush endpoint, last workspace), with an active one. Everything stored on the device is **namespaced by the connection id**: offline data, Moodle files, the web cache, preferences, widget settings. No sharing between connections, so a sign-out removes exactly one namespace.
- **Widgets choose their connection when added** (the configuration screen already exists); a widget keeps its connection id.
- **DAVx5**: one phone account per connection (account name includes the instance host); the existing setup is repeated per connection.
- **UnifiedPush**: one registration instance per connection (the library supports an instance id), so each instance gets its own endpoint.
- **Links**: a link of a connected but inactive instance switches to it (no browser offer); a link of an unknown instance keeps today's behaviour (offered in the browser), which is also ADR 0004's rule.
- **Migration**: the existing single instance becomes connection `default` with its data moved in place, so an update loses nothing.

## Why not now

It touches every storage path at once and cannot be verified without devices and a toolchain in this change. Links already go through `InstanceLinks`, which is where the "inactive but connected instance" rule will plug in. The parent issue 136 therefore stays open on this last sub-issue.
