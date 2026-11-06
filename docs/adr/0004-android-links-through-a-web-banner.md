# Instance links open the Android app through a web banner, not App Links

Every Looty instance is self-hosted on its own domain (GLOSSARY: *Instance*, *Instance de l'app*). Android App Links (verified `https` links) need each domain listed in the manifest at build time, plus an `assetlinks.json` signed for our key on that domain, so one published APK cannot claim the links of arbitrary instances. On Android 12+, unverified `https` intent filters no longer open the app by default, so a `https://*/w/…` filter is not a reliable fallback either.

Decision: **the web interface hands over to the app**. On an Android browser, Looty web shows a banner (and a menu entry) "Open in the Looty app" pointing to `intent://…#Intent;scheme=looty;package=…;S.browser_fallback_url=…;end`, which opens `looty://w/…` when the app is installed and the download page otherwise. "Share → Looty" on a link of the app's instance offers "Open in Looty" first, next to creating a task or a document. The app only opens links of *its* instance; a link from another instance is offered in the browser.

Considered options: App Links for a fixed list of domains (only helps the domains we know, and there is no official hosted instance); a build per self-hoster (impractical, breaks in-app updates and F-Droid).

Consequences: tapping a Looty link in another app (chat, mail) first opens the browser, then one tap switches to the app. Revisit if the app ever supports several instances and a reliable way to claim arbitrary domains appears.
