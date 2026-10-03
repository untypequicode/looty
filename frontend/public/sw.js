// Service worker Looty : notifications push.
self.addEventListener('install', () => self.skipWaiting())
self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()))

self.addEventListener('push', (event) => {
  let data = {}
  try { data = event.data ? event.data.json() : {} } catch { data = { body: event.data && event.data.text() } }
  event.waitUntil((async () => {
    // Application ouverte et au premier plan : la notification apparaît déjà dans l'app.
    const wins = await self.clients.matchAll({ type: 'window', includeUncontrolled: true })
    if (wins.some((w) => w.focused && w.visibilityState === 'visible') && data.tag !== 'looty-test') return
    await self.registration.showNotification(data.title || 'Looty', {
      body: data.body || '',
      tag: data.tag || undefined,
      icon: '/favicon.ico',
      badge: '/favicon.ico',
      data: { url: data.url || '/' },
    })
  })())
})

self.addEventListener('notificationclick', (event) => {
  event.notification.close()
  const url = new URL((event.notification.data && event.notification.data.url) || '/', self.location.origin)
  if (url.origin !== self.location.origin) return
  event.waitUntil((async () => {
    const wins = await self.clients.matchAll({ type: 'window', includeUncontrolled: true })
    const win = wins.find((w) => new URL(w.url).origin === url.origin)
    if (win) {
      await win.focus()
      return win.navigate(url.href)
    }
    return self.clients.openWindow(url.href)
  })())
})
