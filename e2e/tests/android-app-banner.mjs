// Bannière « Ouvrir dans l'app Looty » : seulement dans un navigateur Android, lien intent:// vers la page
// (avec l'instance et le téléchargement en repli), masquable sur l'appareil, réactivable dans les réglages.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const ANDROID = 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36'
const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)

const board = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Bannière Android', with_default_columns: true })
const col = (await api(`/api/boards/${board.id}`)).columns[0]
const task = await api(`/api/boards/columns/${col.id}/tasks`, 'POST', { title: 'Réviser le partiel' })
const taskPath = `/w/${WS}/board/${board.id}?task=${task.id}`
const download = (await api('/api/releases')).latest.android?.url
ok('server gives an Android download page', typeof download === 'string' && download.startsWith('http'), String(download))

// Navigateur de bureau : pas de bannière.
await p.goto(B + taskPath, { waitUntil: 'networkidle' }); await p.waitForTimeout(800)
ok('no banner in a desktop browser', await p.locator('.android-banner').count() === 0)

const ctx = await browser.newContext({ userAgent: ANDROID, viewport: { width: 412, height: 900 }, isMobile: true, hasTouch: true })
const a = await ctx.newPage()
a.on('pageerror', (e) => errors.push(e.message))

// Page de connexion : pas de bannière.
await a.goto(`${B}/login`, { waitUntil: 'networkidle' }); await a.waitForTimeout(500)
ok('no banner on the login page', await a.locator('.android-banner').count() === 0)

await login(a)
await a.goto(B + taskPath, { waitUntil: 'networkidle' }); await a.waitForTimeout(1000)
const banner = a.locator('.android-banner')
ok('banner shown in an Android browser', await banner.count() === 1)
const expected = `intent://w/${WS}/board/${board.id}?task=${task.id}&server=${encodeURIComponent(new URL(B).origin)}`
  + `#Intent;scheme=looty;package=io.github.untypequicode.looty;S.browser_fallback_url=${encodeURIComponent(download)};end`
const href = await banner.locator('a.android-banner-open').getAttribute('href')
ok('Open points to the task in the app', href === expected, href)
await a.screenshot({ path: OUT + 'android-banner.png' })

// ✕ : masquée, et le reste après rechargement ; l'action reste dans le menu de la tâche.
await banner.locator('.android-banner-x').click(); await a.waitForTimeout(300)
ok('banner hidden after ✕', await a.locator('.android-banner').count() === 0)
await a.reload({ waitUntil: 'networkidle' }); await a.waitForTimeout(1000)
ok('banner stays hidden after a reload', await a.locator('.android-banner').count() === 0)
await a.locator('.panel-head button').filter({ has: a.locator('svg.lucide-ellipsis, svg.lucide-more-horizontal') }).first().click(); await a.waitForTimeout(300)
const items = await a.locator('.menu-item').allInnerTexts()
ok('task menu offers to open in the app', items.some((x) => /Looty app|app Looty/.test(x)), JSON.stringify(items))
ok('task menu does not offer the desktop app', !items.some((x) => /desktop app|application de bureau/.test(x)), JSON.stringify(items))
await a.keyboard.press('Escape')

// Réglages du compte : la bannière revient.
await a.goto(`${B}/w/${WS}/settings/preferences`, { waitUntil: 'networkidle' }); await a.waitForTimeout(800)
const sw = a.locator('.sr', { hasText: /Looty app|app Looty/ }).locator('[role=switch]')
ok('setting shown, off', await sw.getAttribute('aria-checked') === 'false')
await sw.click(); await a.waitForTimeout(300)
await a.goto(B + taskPath, { waitUntil: 'networkidle' }); await a.waitForTimeout(1000)
ok('banner back after turning the setting on', await a.locator('.android-banner').count() === 1)

// Autres pages /w/… : agenda, mode lecture.
await a.goto(`${B}/w/${WS}/calendar`, { waitUntil: 'networkidle' }); await a.waitForTimeout(800)
ok('banner on the calendar', await a.locator('.android-banner').count() === 1)
const doc = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Fiche de révision' })
await a.goto(`${B}/w/${WS}/read/${doc.id}`, { waitUntil: 'networkidle' }); await a.waitForTimeout(800)
const readHref = await a.locator('.android-banner a.android-banner-open').getAttribute('href')
ok('reader page banner opens the reader', readHref?.startsWith(`intent://w/${WS}/read/${doc.id}?server=`), readHref)
await api(`/api/workspaces/${WS}/pages/${doc.id}`, 'DELETE')

await api(`/api/boards/${board.id}`, 'DELETE')
ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
