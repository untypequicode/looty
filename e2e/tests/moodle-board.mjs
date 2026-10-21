// K1 : tableau de bord Moodle en grille (déplacer, redimensionner, ajouter, renommer, retirer, empiler, réinitialiser).
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { moodleBoard: null })
const board = async () => (await api(`/api/workspaces/${WS}/prefs`)).moodleBoard?.panels
await p.goto(`${B}/w/${WS}/moodle`, { waitUntil: 'networkidle' }); await p.waitForTimeout(1500)
const titles = async () => p.locator('.gp-head h3').allInnerTexts()
ok('default board: 4 panels', (await titles()).length === 4, JSON.stringify(await titles()))
ok('courses panel lists courses', await p.locator('.gp .ci').count() >= 2)
await p.screenshot({ path: OUT + 'k1-default.png' })

// Déplacer « Échéances » à droite en glissant son en-tête.
const head = p.locator('.gp-head', { hasText: /Échéances|Deadlines/ })
let b = await head.boundingBox()
await p.mouse.move(b.x + 60, b.y + 12); await p.mouse.down()
for (let i = 1; i <= 20; i++) await p.mouse.move(b.x + 60 + i * 40, b.y + 12 + i * 2)
await p.mouse.up(); await p.waitForTimeout(1200)
let saved = await board()
const dl = saved?.find((x) => x.kind === 'deadlines')
ok('drag saved automatically', dl && dl.x > 0, JSON.stringify(dl))

// Redimensionner « Mes cours » par le coin.
const item = p.locator('.vgl-item', { has: p.locator('.gp-head', { hasText: /Mes cours|My courses/ }) })
await item.hover()
const rz = item.locator('.vgl-item__resizer'); b = await rz.boundingBox()
await p.mouse.move(b.x + 5, b.y + 5); await p.mouse.down()
for (let i = 1; i <= 10; i++) await p.mouse.move(b.x + 5, b.y + 5 + i * 20)
await p.mouse.up(); await p.waitForTimeout(1200)
saved = await board()
const cr = saved?.find((x) => x.kind === 'courses')
ok('resize saved', cr && cr.h > 12, JSON.stringify(cr))

// Recharger : la disposition revient.
await p.reload({ waitUntil: 'networkidle' }); await p.waitForTimeout(1500)
const box = await p.locator('.vgl-item', { has: p.locator('.gp-head', { hasText: /Mes cours|My courses/ }) }).boundingBox()
ok('layout kept after reload', box && box.height > 12 * 28 + 11 * 14 + 20, JSON.stringify(box))

// Ajouter « Progression » et « Cours épinglé ».
const add = async (label) => { await p.locator('button', { hasText: /Ajouter un panneau|Add a panel/ }).click(); await p.waitForTimeout(200); await p.locator('[role=menuitem], .menu-item, button', { hasText: label }).last().click(); await p.waitForTimeout(1000) }
await add(/^Progression$|^Progress$/)
ok('progress panel added', (await titles()).some((x) => /Progression|Progress/.test(x)))
await add(/Cours épinglé|Pinned course/)
await p.locator('.mb-course').first().click(); await p.waitForTimeout(1800)
ok('pinned course shows its sections', await p.locator('.gp .mc.embedded .mc-sec').count() >= 1)
saved = await board()
ok('6 panels saved', saved?.length === 6, String(saved?.length))

// Renommer (double-clic sur le titre).
await p.locator('.gp-head h3', { hasText: /Progression|Progress/ }).dblclick()
await p.locator('.gp-input').fill('Mon avancement'); await p.keyboard.press('Enter'); await p.waitForTimeout(800)
ok('panel renamed', (await titles()).includes('Mon avancement') && (await board()).some((x) => x.title === 'Mon avancement'))

// Retirer par le menu.
const prog = p.locator('.gp', { has: p.locator('h3', { hasText: 'Mon avancement' }) })
await prog.locator('.gp-head button[aria-label]').last().click(); await p.waitForTimeout(200)
await p.locator('button, [role=menuitem]', { hasText: /Retirer le panneau|Remove the panel/ }).last().click(); await p.waitForTimeout(800)
ok('panel removed', !(await titles()).includes('Mon avancement') && (await board()).length === 5)
await p.screenshot({ path: OUT + 'k1-custom.png' })

// Messages récents : ouvre la conversation.
const conv = p.locator('.gp .cv').first()
if (await conv.count()) { await conv.click(); await p.waitForTimeout(1200); ok('conversation opens in Messages tab', await p.locator('.cv-thread .bubble').count() >= 1) }
else ok('conversation opens in Messages tab', false, 'no conversation in mock')
await p.locator('.md-tabs button', { hasText: /Tableau de bord|Dashboard/ }).click(); await p.waitForTimeout(800)

// Fenêtre étroite : panneaux empilés, sans changer la disposition.
const before = JSON.stringify(await board())
await p.setViewportSize({ width: 620, height: 900 }); await p.waitForTimeout(1000)
ok('narrow: stacked', await p.locator('.pg-stack .gp').count() === 5 && await p.locator('.vgl-layout').count() === 0)
await p.screenshot({ path: OUT + 'k1-narrow.png', fullPage: true })
await p.setViewportSize({ width: 1400, height: 950 }); await p.waitForTimeout(1000)
ok('layout unchanged by stacking', JSON.stringify(await board()) === before)

// Réinitialiser.
await p.getByRole('button', { name: /^(More actions|Plus d.actions)$/ }).first().click(); await p.waitForTimeout(200)
await p.locator('button, [role=menuitem]', { hasText: /Réinitialiser le tableau de bord|Reset the dashboard/ }).last().click(); await p.waitForTimeout(300)
await p.locator('.dialog button, [role=dialog] button', { hasText: /Réinitialiser|Reset/ }).last().click(); await p.waitForTimeout(1000)
ok('reset to default', (await titles()).length === 4 && !(await board()))
ok('no page errors', errors.length === 0, errors.slice(0, 3).join(' | '))
await browser.close()
