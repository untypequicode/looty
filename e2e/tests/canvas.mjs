// Canevas (façon AFFiNE) : chaque page a une vue Canevas collaborative ; les éléments de
// Looty y deviennent des cartes vivantes ; la section Canevas les liste ; les liens du
// canevas comptent dans la recherche et le graphe.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const alice = await launch(1440, 950)
await login(alice.page)
const p = alice.page
const api = (path, method, body) => call(p, path, method, body)

const target = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Cahier des charges', content: '<p>Objectifs du projet et contraintes.</p>' })
const board = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Sprint', with_default_columns: true })
const col = (await api(`/api/boards/${board.id}`)).columns[1]
const task = await api(`/api/boards/columns/${col.id}/tasks`, 'POST', { title: 'Maquette de l’accueil' })
const cv = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Atelier', default_view: 'canvas' })
ok('page créée en canevas', cv.default_view === 'canvas')

await p.goto(`${B}/w/${WS}/doc/${cv.id}`, { waitUntil: 'networkidle' })
await p.locator('header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click()
await p.waitForSelector('.excalidraw', { timeout: 20000 })
ok('canevas : Excalidraw affiché', await p.locator('.excalidraw').isVisible())
ok('canevas : sélecteur Page / Canevas', (await p.locator('.doc-views').textContent()).includes('Canva'))

// Insertion de cartes : le document et la tâche.
await p.locator('.cv-insert').click()
await p.locator('.ci-row', { hasText: 'Cahier des charges' }).click()
await p.locator('.ci').getByText(/^(Tâches|Tasks)$/).click()
await p.waitForTimeout(600)
await p.locator('.ci-row', { hasText: 'Maquette' }).click()
await p.locator('.dialog-footer').getByRole('button', { name: /(Insérer|Insert) 2/ }).click()
await p.waitForTimeout(1500)
ok('cartes vivantes affichées', (await p.locator('.lc').count()) === 2, String(await p.locator('.lc').count()))
ok('carte du document : titre et extrait', (await p.locator('.lc.page').textContent()).includes('Objectifs du projet'))
ok('carte de la tâche : colonne', (await p.locator('.lc.task').textContent()).includes(col.name))

// Un rectangle dessiné.
await p.mouse.click(1000, 820)
await p.keyboard.press('r')
await p.mouse.move(700, 650)
await p.mouse.down()
await p.mouse.move(900, 780, { steps: 6 })
await p.mouse.up()
await p.waitForTimeout(300)
await p.screenshot({ path: OUT + 'canvas.png' })

// Enregistrement par le serveur de collaboration (délai de quelques secondes).
let listed
for (let i = 0; i < 15; i++) {
  listed = (await api(`/api/workspaces/${WS}/canvases`)).find((c) => c.id === cv.id)
  if (listed?.canvas_count >= 3) break
  await p.waitForTimeout(1000)
}
ok('enregistré : 3 éléments', listed?.canvas_count === 3, JSON.stringify(listed && { count: listed.canvas_count }))
for (let i = 0; i < 8 && !listed?.canvas_preview; i++) {
  await p.waitForTimeout(1000)
  listed = (await api(`/api/workspaces/${WS}/canvases`)).find((c) => c.id === cv.id)
}
ok('aperçu SVG enregistré', !!listed?.canvas_preview?.startsWith('<svg'))

// Interconnexion : la carte crée un lien dans le graphe, le canevas est trouvé par la recherche.
const graph = await api(`/api/workspaces/${WS}/graph`)
ok('graphe : lien canevas → document', graph.edges.some((e) => e.source === cv.id && e.target === target.id && e.kind === 'link'))
ok('graphe : lien canevas → tâche', graph.edges.some((e) => e.source === cv.id && e.target === task.id))
const found = await api(`/api/workspaces/${WS}/search?q=${task.id}`)
ok('recherche : liens du canevas', found.results.some((r) => r.id === cv.id))

// La carte suit la tâche en direct.
await api(`/api/boards/tasks/${task.id}`, 'PUT', { title: 'Maquette de l’accueil v2' })
await p.waitForTimeout(1200)
ok('carte tenue à jour', (await p.locator('.lc.task').textContent()).includes('v2'))

// Une autre personne voit le même canevas.
const bob = await launch(1300, 900)
await login(bob.page, 'bob-canvas@x.io')
const bobId = (await call(bob.page, '/api/auth/me')).user.id
await api(`/api/workspaces/${WS}/access/members/add`, 'POST', { user_id: bobId })
const { readFileSync } = await import('node:fs')
const webVersion = JSON.parse(readFileSync(new URL('../../backend/releases.json', import.meta.url))).releases.find((r) => r.platform === 'web').version
await call(bob.page, '/api/user/releases/seen', 'PUT', { version: webVersion })
await call(bob.page, `/api/workspaces/${WS}/prefs`, 'PATCH', { modesChosen: true })
await bob.page.goto(`${B}/w/${WS}/doc/${cv.id}`, { waitUntil: 'networkidle' })
await bob.page.waitForSelector('.lc', { timeout: 20000 }).catch(() => {})
ok('autre personne : mêmes cartes', (await bob.page.locator('.lc').count()) === 2)
await bob.browser.close()

// Section Canevas.
await p.goto(`${B}/w/${WS}/canvas`, { waitUntil: 'networkidle' })
ok('section : carte du canevas', await p.locator('.cl-card', { hasText: 'Atelier' }).isVisible())
ok('section : aperçu', (await p.locator('.cl-card', { hasText: 'Atelier' }).locator('img').count()) === 1)
await p.screenshot({ path: OUT + 'canvas-section.png' })

// Vue Canevas de n'importe quelle page.
await p.goto(`${B}/w/${WS}/doc/${target.id}?view=canvas`, { waitUntil: 'networkidle' })
await p.waitForSelector('.excalidraw', { timeout: 20000 })
ok('page ordinaire : vue Canevas', await p.locator('.excalidraw').isVisible())

ok('aucune erreur page', !alice.errors.length, alice.errors.join(' | '))
await alice.browser.close()
