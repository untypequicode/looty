# Android: several instances in one app

Status: accepted, implemented in android 0.31.0 (it was specified and deferred in 0.30.0).

GLOSSARY: *Instance de l'app*. Issue 143 (last child of 136, links to an instance in the app).

The app used to know one instance (`Config.server`, one `looty` preferences file, one offline store, one set of widgets, one UnifiedPush registration). A person who uses two instances (an association and a school) had to sign out to switch.

## Decisions

- **One record per instance** (`Instance(id, origin)`), a list and an *active* one, kept in the shared `looty` preferences (`instances`, `activeInstance`). `InstanceStore` holds the logic over a small key/value interface so it is unit tested on the JVM. One account per instance: the session is the WebView cookie of the instance's host.
- **Everything stored on the device is scoped by the instance id.** `InstanceStore.scoped(name, id)` returns `name` unchanged for the first instance and `name.<id>` for the others. It names: the per-instance preferences (`looty`, workspace, views, mail account, refused changes), the notification preferences (`looty.push`), the widget cache (`looty.widget`), the outbox file and its attachment folder, and the WorkManager unique works (`looty-poll`, `looty-outbox`). The offline JSON cache and the web cache were already folders named after a hash of the instance address, so they are per instance without a move.
- **Migration with no move.** The old single `server` becomes the instance `default` the first time the list is read. Because `default` keeps the historical names above, its preferences, caches, outbox, notification settings, scheduled works and widgets stay exactly where they are: an update loses nothing. The `server` key is kept as a mirror of the active instance so a downgrade still works.
- **A session is bound to one instance.** `AppState` and `Api` take the instance id at creation. Switching (from the *Instances* screen, a link, a widget) activates the instance and restarts the main activity with a new task, so no state of the previous instance survives in memory. `Api` for an id that no longer exists fails (`no_server`) and never falls back to another instance.
- **Sign-out and removal are per instance.** Sign-out clears the cookies of that host only (the others stay signed in), its push subscription, offline copy, queued changes and caches. Removing an instance also deletes its preferences, notification settings, scheduled works and channels.
- **Notifications**: UnifiedPush is registered with the instance id as the library instance (`default` for the first, which matches the library default, so the existing registration keeps working) and each instance gets its own endpoint and subscription. The periodic check is one work per instance. Notification channels are per instance and kind (historic ids for the first instance, `<kind>.<id>` for the others, named after the host when there are several), so each instance can have its own sound and importance. A tap opens a link that carries the instance address, so it switches if needed.
- **Widgets choose their instance when added** (the configuration screen lists the instances when there are several). The choice is stored per widget; a widget placed before this change has none and means the first instance. Taps and the check-off action carry the instance. A widget whose instance was removed shows "signed out" and never data of another instance.
- **Links**: `InstanceLinks.resolve` gets the connected instances. A link of the active instance opens; a link of another connected instance switches to it; a link of an unknown instance offers *Add this instance* (the setup screen with the address filled in, then the page opens) or the browser, which stays ADR 0004's behaviour. Links produced by the app for another instance (notifications, widgets) carry `server=`.
- **DAVx5**: the phone-calendar connection is set up from the instance's *CalDAV* page by the person, per instance, with an app password issued by that instance; the app does not create phone accounts itself, so nothing needs a separate account per instance in the app.

## Known limits

- Cookies are shared by host name, not by port: two instances on the same host with different ports (a local test setup) share a session. Real instances have different hosts.
- Moodle files and attachments in the cache directory are named by id, which is unique per server; they are not purged on removal (Android clears that cache itself).
- Not verified on a device in this change: the multi-instance flows were checked by unit tests (storage, migration, links) and a build, not on a phone or emulator.
