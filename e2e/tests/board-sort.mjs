// Tri des tâches d'un tableau : choisi dans la barre d'outils, gardé dans l'URL, appliqué dans
// chaque colonne avec les filtres ; les valeurs absentes restent en bas ; « Ordre manuel » revient
// à l'ordre du glisser-déposer.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch()
await login(p)
const api = (path, method, body) => call(p, path, method, body)
const board = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Tri', with_default_columns: true })
const col = (await api(`/api/boards/${board.id}`)).columns[0]
const make = (body) => api(`/api/boards/columns/${col.id}/tasks`, 'POST', body)
// Ordre manuel : Alpha, Bravo, Charlie, Delta.
const a = await make({ title: 'Alpha', due_date: '2030-01-05T00:00:00Z', priority: 3 })
const b = await make({ title: 'Bravo', priority: 1 })
const c = await make({ title: 'Charlie', due_date: '2030-01-01T00:00:00Z', priority: 4 })
const d = await make({ title: 'Delta', due_date: '2030-01-03T00:00:00Z' })
const id = { [a.id]: 'A', [b.id]: 'B', [c.id]: 'C', [d.id]: 'D' }
const order = async (sel = `.col[data-column-id="${col.id}"] [data-task-id]`) =>
  (await p.locator(sel).evaluateAll((els) => els.map((e) => e.dataset.taskId))).map((x) => id[x] ?? '?').join('')
const settle = () => p.waitForTimeout(400)

await p.goto(`${B}/w/${WS}/board/${board.id}`, { waitUntil: 'networkidle' })
ok('ordre manuel par défaut', (await order()) === 'ABCD', await order())

// Choix dans la barre d'outils : échéance, plus tôt d'abord ; Bravo (sans échéance) en bas.
const sortBtn = p.locator('.tb-sort')
await sortBtn.click()
await p.locator('[data-sort="due"]').click()
await settle()
ok('tri par échéance', (await order()) === 'CDAB', await order())
ok('tri gardé dans l’URL', new URL(p.url()).searchParams.get('sort') === 'due')
ok('tri visible dans la barre', /Échéance|Due date/.test(await sortBtn.innerText()))

// Sens inverse : les valeurs absentes restent en bas.
await p.locator('.sort-foot .seg-btn').nth(1).click()
await settle()
ok('échéance, plus tard d’abord', (await order()) === 'ADCB', await order())
ok('sens gardé dans l’URL', new URL(p.url()).searchParams.get('dir') === 'desc')
await p.screenshot({ path: OUT + 'board-sort.png' })
await p.keyboard.press('Escape')

// Lien partagé : priorité (urgente d'abord), Delta sans priorité en bas, avec un filtre de recherche.
await p.goto(`${B}/w/${WS}/board/${board.id}?sort=priority&q=a`, { waitUntil: 'networkidle' })
await settle()
ok('lien partagé : tri et recherche ensemble', (await order()) === 'BACD', await order())
await p.goto(`${B}/w/${WS}/board/${board.id}?sort=priority&q=ar`, { waitUntil: 'networkidle' })
await settle()
ok('le filtre s’applique au tri', (await order()) === 'C', await order())

// Vue liste : même tri, tâches dans leur colonne.
await p.goto(`${B}/w/${WS}/board/${board.id}?sort=title&dir=desc&view=list`, { waitUntil: 'networkidle' })
await settle()
ok('vue liste triée', (await order('.lv-row')) === 'DCBA', await order('.lv-row'))

// Retour à l'ordre manuel.
await p.goto(`${B}/w/${WS}/board/${board.id}?sort=due`, { waitUntil: 'networkidle' })
await p.locator('.tb-sort').click()
await p.locator('[data-sort="manual"]').click()
await settle()
ok('ordre manuel rétabli', (await order()) === 'ABCD' && !new URL(p.url()).searchParams.get('sort'), p.url())
ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
