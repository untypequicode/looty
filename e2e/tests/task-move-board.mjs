// Panneau de tâche : « Déplacer vers un tableau » envoie la tâche dans la première colonne
// d'un autre tableau, avec ses étiquettes.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch()
await login(p)
const api = (path, method, body) => call(p, path, method, body)
const src = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Départ', with_default_columns: true })
const dst = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Arrivée', with_default_columns: true })
const col = (await api(`/api/boards/${src.id}`)).columns[1]
const label = await api(`/api/boards/${src.id}/labels`, 'POST', { name: 'Maths', color: '#3b82f6' })
const task = await api(`/api/boards/columns/${col.id}/tasks`, 'POST', { title: 'À déménager', labels: [label.id] })

await p.goto(`${B}/w/${WS}/board/${src.id}`, { waitUntil: 'networkidle' })
// Tableau ouvert en lecture (préférence de l'espace) : passage en modification.
await p.locator('.page-header, header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click()
await p.waitForTimeout(400)
await p.locator(`[data-task-id="${task.id}"]`).first().click()
await p.waitForTimeout(600)
await p.locator('.panel-head button').filter({ has: p.locator('svg.lucide-ellipsis, svg.lucide-more-horizontal') }).first().click()
const item = p.locator('.menu-item', { hasText: 'Arrivée' })
ok('menu: autre tableau proposé', await item.isVisible())
ok('menu: tableau courant absent', !(await p.locator('.menu-item', { hasText: 'Départ' }).count()))
await p.screenshot({ path: OUT + 'move-board-menu.png' })
await item.click()
await p.waitForTimeout(1200)
const after = await api(`/api/boards/${dst.id}`)
const moved = after.columns[0].tasks.find((t) => t.id === task.id)
ok('tâche dans la 1re colonne du tableau d’arrivée', !!moved)
ok('étiquette reprise', moved?.labels?.[0]?.name === 'Maths' && moved.labels[0].board_id === dst.id)
ok('panneau fermé, tâche retirée du tableau de départ', !(await p.locator(`[data-task-id="${task.id}"]`).count()))
ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
