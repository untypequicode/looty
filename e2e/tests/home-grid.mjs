import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
const dash = async () => (await api(`/api/workspaces/${WS}/prefs`)).dashboard
// Ancienne disposition (liste + tailles) : convertie à la lecture.
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { dashboard: { saved: true, knowsClasses: true, widgets: [{ id: 'greeting', size: 'full' }, { id: 'agenda', size: 'half' }, { id: 'myTasks', size: 'half' }, { id: 'boards', size: 'full' }] } })
await p.goto(`${B}/w/${WS}`, { waitUntil: 'networkidle' }); await p.waitForTimeout(1500)
ok('legacy: 3 panels + greeting', (await p.locator('.gp-head h3').count()) === 3 && (await p.locator('.hero').count()) === 1)
await p.screenshot({ path: OUT + 'k4-legacy.png' })
// Défaut
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { dashboard: null })
await p.goto(`${B}/w/${WS}`, { waitUntil: 'networkidle' }); await p.waitForTimeout(1500)
ok('default: no forced dialog', (await p.locator('[role=dialog]').count()) === 0)
await p.screenshot({ path: OUT + 'k4-default.png' })
// Déplacer « Documents » par son en-tête
const head = p.locator('.gp-head', { hasText: /Documents|Docs/ }).first()
let b = await head.boundingBox()
await p.mouse.move(b.x + 80, b.y + 12); await p.mouse.down()
for (let i = 1; i <= 20; i++) await p.mouse.move(b.x + 80 - i * 20, b.y + 12 + i * 4)
await p.mouse.up(); await p.waitForTimeout(1200)
const d = await dash()
ok('drag saved', d?.grid && d.saved, JSON.stringify(d?.widgets?.find((w) => w.i === 'docs')))
// Retirer puis rajouter via Personnaliser
const n0 = await p.locator('.gp-head h3').count()
await p.locator('.gp', { hasText: /Documents|Docs/ }).first().locator('.gp-head button').last().click(); await p.waitForTimeout(300)
await p.getByText(/Retirer le panneau|Remove the panel/).click(); await p.waitForTimeout(1200)
ok('remove', (await p.locator('.gp-head h3').count()) === n0 - 1)
await p.locator('.page-header, header').getByRole('button', { name: /Personnaliser|Customize/ }).first().click(); await p.waitForTimeout(500)
await p.screenshot({ path: OUT + 'k4-customize.png' })
await p.locator('.hc-row', { hasText: /Documents/ }).first().locator('.hc-toggle').click(); await p.waitForTimeout(1200)
await p.keyboard.press('Escape'); await p.waitForTimeout(400)
ok('re-add', (await p.locator('.gp-head h3').count()) === n0)
// Réinitialiser
await p.locator('.page-header, header').getByRole('button', { name: /Personnaliser|Customize/ }).first().click(); await p.waitForTimeout(400)
await p.getByRole('button', { name: /Réinitialiser|Reset/ }).click(); await p.waitForTimeout(1200)
ok('reset', !(await dash()))
// Étroit
await p.keyboard.press('Escape')
await p.setViewportSize({ width: 600, height: 900 }); await p.waitForTimeout(800)
await p.screenshot({ path: OUT + 'k4-narrow.png' })
ok('no page errors', errors.length === 0, errors.join('|'))
await browser.close()
