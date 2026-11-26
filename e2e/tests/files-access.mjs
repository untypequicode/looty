// Fichiers envoyés (ADR 0022, phase 5) : une image n'est lisible que par qui lit la page qui la porte,
// retirer l'accès la coupe tout de suite, un avatar reste public.
import { B, WS, ok, launch, login, api as call } from '../lib/index.mjs'

const alice = await launch()
await login(alice.page)
const carl = await launch()
await login(carl.page, 'carl-files@x.io')
const a = (path, method, body) => call(alice.page, path, method, body)
const c = (path, method, body) => call(carl.page, path, method, body)
const carlId = (await c('/api/auth/me')).user.id

const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='
const page = await a(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Files access', content: '<p>x</p>' })
const upload = (p, query) => p.evaluate(async ({ b64, query }) => {
  const u8 = Uint8Array.from(atob(b64), (ch) => ch.charCodeAt(0))
  const f = new FormData()
  f.append('file', new File([u8], 'p.png', { type: 'image/png' }))
  const r = await fetch(`/api/upload${query}`, { method: 'POST', body: f, credentials: 'include' })
  return { status: r.status, body: await r.json().catch(() => ({})) }
}, { b64: PNG, query })
const up = await upload(alice.page, `?page=${page.id}`)
ok('envoi rattaché à la page', up.status === 200 && up.body.url?.startsWith('/uploads/'), JSON.stringify(up))
const url = up.body.url

const fetchStatus = (p, u, credentials = 'include') => p.evaluate(async ({ u, credentials }) => (await fetch(u, { credentials })).status, { u, credentials })
const imageLoads = (p, u) => p.evaluate((u) => new Promise((resolve) => {
  const img = new Image()
  img.onload = () => resolve(true)
  img.onerror = () => resolve(false)
  img.src = `${u}?t=${Date.now()}`
}), url)

await carl.page.goto(`${B}/`, { waitUntil: 'domcontentloaded' })
ok('sans partage: 404', (await fetchStatus(carl.page, url)) === 404)
ok('sans connexion: 401', (await fetchStatus(carl.page, url, 'omit')) === 401)
ok('propriétaire: 200', (await fetchStatus(alice.page, url)) === 200)

await a(`/api/workspaces/${WS}/pages/${page.id}/permissions`, 'PUT', { user_id: carlId, role: 'viewer' })
ok('invité en lecture: 200', (await fetchStatus(carl.page, url)) === 200)
ok('invité en lecture: l\'image se charge', await imageLoads(carl.page, url))
const head = await carl.page.evaluate(async (u) => { const r = await fetch(u); return r.headers.get('cache-control') }, url)
ok('cache privé', /private/.test(head || '') && !/immutable/.test(head || ''), String(head))

// Retrait pendant que la personne regarde la page : l'image ne se charge plus.
await carl.page.goto(`${B}/shared/${page.id}`, { waitUntil: 'networkidle' })
await a(`/api/workspaces/${WS}/pages/${page.id}/permissions`, 'PUT', { user_id: carlId, role: 'blocked' })
await carl.page.waitForTimeout(1500)
ok('accès retiré: 404 tout de suite', (await fetchStatus(carl.page, url)) === 404)
ok('accès retiré: l\'image ne se charge plus', !(await imageLoads(carl.page, url)))

// Un avatar reste public.
const av = await upload(alice.page, '')
await a('/api/user/profile', 'PUT', { name: 'Alice', avatar_url: av.body.url })
ok('avatar: public', (await fetchStatus(carl.page, av.body.url, 'omit')) === 200)

await alice.browser?.close?.()
await carl.browser?.close?.()
