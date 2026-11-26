// Aides des tests de partage de pages (ADR 0022) : chaque test crée SON espace de travail et ses comptes, pour ne
// dépendre ni de l'ordre d'exécution ni de l'état laissé par un autre test (la CI répartit les fichiers sur 3 instances).
import { readFileSync } from 'node:fs'
import { B, ok, launch, login, api } from './index.mjs'

export const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='
const webVersion = JSON.parse(readFileSync(new URL('../../backend/releases.json', import.meta.url))).releases.find((r) => r.platform === 'web').version

/** Attend qu'une condition (asynchrone) devienne vraie. */
export const until = async (fn, tries = 60, ms = 250) => {
  for (let i = 0; i < tries; i++) {
    try { if (await fn()) return true } catch { /* la page navigue : on réessaie */ }
    await new Promise((r) => setTimeout(r, ms))
  }
  return false
}

/** Alice (administratrice de l'instance, propriétaire d'un espace neuf) ; `person()` ouvre d'autres comptes. */
export async function start(label, width = 1400, height = 950) {
  const { browser, page: alice, errors } = await launch(width, height)
  await login(alice)
  const a = (path, method, body) => api(alice, path, method, body)
  await a('/api/user/releases/seen', 'PUT', { version: webVersion })
  const { workspace } = await a('/api/workspaces', 'POST', { name: `${label} ${Date.now()}` })
  const ws = workspace.id
  await a(`/api/workspaces/${ws}/prefs`, 'PATCH', { modes: { docs: 'write', boards: 'write', calendar: 'write' }, modesChosen: true })
  const jsErrors = [errors]

  /** Compte hors de l'espace (invité) avec son propre navigateur isolé. */
  const person = async (email) => {
    const ctx = await browser.newContext({ viewport: { width: 1300, height: 900 } })
    const page = await ctx.newPage()
    const errs = []
    page.on('pageerror', (e) => errs.push(e.message))
    jsErrors.push(errs)
    await login(page, email)
    const call = (path, method, body) => api(page, path, method, body)
    await call('/api/user/releases/seen', 'PUT', { version: webVersion })
    const me = (await call('/api/auth/me')).user
    return {
      page, id: me.id, email, api: call,
      /** Statut HTTP d'une URL (cookies de la personne). */
      status: (url) => page.evaluate(async (u) => (await fetch(u, { credentials: 'include' })).status, url),
      /** L'image se charge-t-elle dans le navigateur ? */
      imageLoads: (url) => page.evaluate((u) => new Promise((resolve) => {
        const img = new Image()
        img.onload = () => resolve(true)
        img.onerror = () => resolve(false)
        img.src = `${u}${u.includes('?') ? '&' : '?'}t=${Date.now()}`
      }), url),
      read: (id) => call(`/api/workspaces/${ws}/pages/${id}`),
      level: async (id) => { const r = await call(`/api/pages/${id}/access`); return r?.status ? null : r.level },
    }
  }
  /** Image PNG envoyée par Alice et rattachée à une page. */
  const upload = (pageId) => alice.evaluate(async ({ b64, pageId }) => {
    const u8 = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
    const f = new FormData()
    f.append('file', new File([u8], 'p.png', { type: 'image/png' }))
    const r = await fetch(`/api/upload?page=${pageId}`, { method: 'POST', body: f, credentials: 'include' })
    return (await r.json()).url
  }, { b64: PNG, pageId })

  return {
    browser, alice, a, ws, person, upload,
    mk: (title, parent_id = null, extra = {}) => a(`/api/workspaces/${ws}/pages`, 'POST', { title, parent_id, content: '<p>x</p>', ...extra }),
    share: (pageId, body) => a(`/api/workspaces/${ws}/pages/${pageId}/permissions`, 'PUT', body),
    open: async (who, pageId, path = 'doc') => {
      await who.goto(`${B}/w/${ws}/${path}/${pageId}`, { waitUntil: 'networkidle' })
    },
    done: async () => {
      const all = jsErrors.flat()
      ok('aucune erreur JS', all.length === 0, all.join(' | '))
      await browser.close()
    },
  }
}
