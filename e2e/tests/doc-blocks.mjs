// Blocs reliés dans un document : un vrai tableau (kanban / liste, filtres) et un agenda des
// prochains jours ; le tableau est relié à la page dans le graphe.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const { browser, page: p, errors } = await launch(1440, 1000)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
const me = (await api('/api/auth/me')).user.id

const board = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Révisions', with_default_columns: true })
const cols = (await api(`/api/boards/${board.id}`)).columns
const due = new Date(Date.now() + 2 * 86400_000).toISOString()
await api(`/api/boards/columns/${cols[1].id}/tasks`, 'POST', { title: 'Fiche chapitre 3', assignees: [me], due_date: due })
await api(`/api/boards/columns/${cols[1].id}/tasks`, 'POST', { title: 'Exercices autre personne' })
const html = `<p>Plan de la semaine</p><div data-type="board-block" data-board="${board.id}" data-view="kanban" data-filter="all"></div><div data-type="agenda-block" data-days="7" data-scope="all"></div>`
const pg = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Semaine', content: html })

await p.goto(`${B}/w/${WS}/doc/${pg.id}`, { waitUntil: 'networkidle' })
await p.waitForSelector('.bb-head', { timeout: 15000 })
ok('bloc tableau : nom', (await p.locator('.bb-name').textContent()).includes('Révisions'))
ok('bloc tableau : kanban avec les tâches', (await p.locator('.bb-body').textContent()).includes('Fiche chapitre 3'))
await p.locator('.ag-row').first().waitFor({ timeout: 10000 }).catch(() => {})
ok('bloc agenda : échéance', (await p.locator('.ag-body').textContent()).includes('Fiche chapitre 3'))
await p.screenshot({ path: OUT + 'doc-blocks.png' })

// Filtre « Les miennes » (en modification) : la tâche d'une autre personne disparaît.
await p.locator('header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click()
await p.waitForTimeout(500)
await p.locator('.bb-head').getByText(/^(Mine|Les miennes)$/).click()
await p.waitForTimeout(500)
const body = await p.locator('.bb-body').textContent()
ok('filtre : seulement mes tâches', body.includes('Fiche chapitre 3') && !body.includes('autre personne'))
await p.locator('.bb-head button[title="List"], .bb-head button[title="Liste"]').first().click()
await p.waitForTimeout(500)
ok('vue liste', !(await p.locator('.bb-body.kanban').count()))

// Enregistrement de la page puis lien dans le graphe.
await p.waitForTimeout(2500)
const graph = await api(`/api/workspaces/${WS}/graph`)
ok('graphe : page → tableau', graph.edges.some((e) => e.source === pg.id && e.target === board.id))
ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
