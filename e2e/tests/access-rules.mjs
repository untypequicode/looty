// Droits fins appliqués partout : un rôle sans accès aux tableaux ne voit aucune tâche (API,
// recherche, graphe, activité, temps réel), une page bloquée ne fuit pas par le rejeu hors
// ligne, un membre retiré perd sa connexion temps réel. Et une tâche passe d'un tableau à
// l'autre avec ses étiquettes.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const alice = await launch()
await login(alice.page)
const bob = await launch()
await login(bob.page, 'bob-access@x.io')
const a = (path, method, body) => call(alice.page, path, method, body)
const b = (path, method, body) => call(bob.page, path, method, body)
const bobId = (await b('/api/auth/me')).user.id

// Rôle « documents seulement » donné à Bob.
const role = await a(`/api/workspaces/${WS}/access/roles`, 'POST', { name: 'Docs only', permissions: { docs: 'edit', boards: 'none', calendar: 'none' } })
await a(`/api/workspaces/${WS}/access/members/add`, 'POST', { user_id: bobId, role_id: role.id })

const b1 = await a(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Accès 1', with_default_columns: true })
const b2 = await a(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Accès 2', with_default_columns: true })
const full1 = await a(`/api/boards/${b1.id}`)
const label = await a(`/api/boards/${b1.id}/labels`, 'POST', { name: 'Urgent', color: '#e5484d' })
const task = await a(`/api/boards/columns/${full1.columns[0].id}/tasks`, 'POST', { title: 'Tâche secrète zorglub', labels: [label.id] })
const sub = await a(`/api/boards/tasks/${task.id}/subtasks`, 'POST', { title: 'Sous-tâche zorglub' })
await a(`/api/boards/tasks/${task.id}/comments`, 'POST', { content: '<p>commentaire</p>' })

ok('tableaux: liste vide', (await b(`/api/workspaces/${WS}/boards`)).length === 0)
ok('tâche: refusée', (await b(`/api/boards/tasks/${task.id}`)).status === 403)
ok('commentaires: refusés', (await b(`/api/boards/tasks/${task.id}/comments`)).status === 403)
ok('historique: refusé', (await b(`/api/boards/tasks/${task.id}/activities`)).status === 403)
ok('pièces jointes: refusées', (await b(`/api/boards/tasks/${task.id}/attachments`)).status === 403)
ok('mes tâches (tout l’espace): vide', (await b(`/api/workspaces/${WS}/my_tasks?scope=all`)).tasks.length === 0)
ok('recherche: aucune tâche', !(await b(`/api/workspaces/${WS}/search?q=zorglub`)).results.some((r) => r.type === 'task'))
const graph = await b(`/api/workspaces/${WS}/graph`)
ok('graphe: ni tableau ni tâche', !graph.nodes.some((n) => n.kind === 'task' || n.kind === 'board'))
ok('activité: aucune tâche', !(await b(`/api/workspaces/${WS}/activity`)).items.some((i) => i.source !== 'page'))
const overview = await b(`/api/workspaces/${WS}/overview`)
ok('accueil: ni tableaux ni activité', overview.boards.length === 0 && overview.activity.length === 0)

// Page bloquée : le rejeu d'une création hors ligne avec son identifiant ne renvoie rien.
const page = await a(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Page privée', content: '<p>secret</p>' })
await a(`/api/workspaces/${WS}/pages/${page.id}/permissions`, 'PUT', { user_id: bobId, role: 'blocked' })
const replay = await b(`/api/workspaces/${WS}/pages`, 'POST', { id: page.id, title: 'x' })
ok('page bloquée: rejeu refusé', replay.status === 409 && !JSON.stringify(replay).includes('secret'), JSON.stringify(replay))

// Document ouvert pendant que les droits changent : le serveur de collaboration revérifie.
const { readFileSync } = await import('node:fs')
const webVersion = JSON.parse(readFileSync(new URL('../../backend/releases.json', import.meta.url))).releases.find((r) => r.platform === 'web').version
await b('/api/user/releases/seen', 'PUT', { version: webVersion })
await b(`/api/workspaces/${WS}/prefs`, 'PATCH', { modesChosen: true, modes: { docs: 'edit', boards: 'read', calendar: 'read' } })
const shared = await a(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Page partagée' })
await bob.page.goto(`${B}/w/${WS}/doc/${shared.id}`, { waitUntil: 'networkidle' })
await bob.page.waitForTimeout(1500)
// Mode lecture par défaut : passage en modification.
await bob.page.getByRole('button', { name: /^(Edit|Modifier)$/ }).click()
await bob.page.waitForTimeout(800)
const editableBefore = await bob.page.evaluate(() => document.querySelector('.ProseMirror')?.getAttribute('contenteditable'))
await a(`/api/workspaces/${WS}/pages/${shared.id}/permissions`, 'PUT', { user_id: bobId, role: 'blocked' })
await bob.page.waitForTimeout(3000)
const editableAfter = await bob.page.evaluate(() => document.querySelector('.ProseMirror')?.getAttribute('contenteditable') ?? 'none')
ok('collaboration: éditable avant', editableBefore === 'true', String(editableBefore))
ok('collaboration: plus éditable une fois bloqué', editableAfter !== 'true', editableAfter)

// Temps réel : Bob ne reçoit rien des tableaux ni de la page bloquée ; retiré, sa connexion se ferme.
await bob.page.goto(`${B}/`, { waitUntil: 'domcontentloaded' })
await bob.page.evaluate((ws) => new Promise((resolve) => {
  const w = window
  w.__msgs = []
  w.__closed = false
  const s = new WebSocket(`${location.origin.replace(/^http/, 'ws')}/api/ws?workspace_id=${ws}`)
  s.onmessage = (m) => w.__msgs.push(JSON.parse(m.data).type)
  s.onclose = () => { w.__closed = true }
  s.onopen = () => resolve(null)
}), WS)
await a(`/api/boards/tasks/${task.id}`, 'PUT', { title: 'Tâche secrète renommée' })
await a(`/api/workspaces/${WS}/pages/${page.id}`, 'PUT', { title: 'Page privée renommée' })
await a(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Page visible' })
await bob.page.waitForTimeout(800)
const got = await bob.page.evaluate(() => window.__msgs)
ok('temps réel: aucune tâche reçue', !got.some((t) => t.startsWith('TASK_')), got.join(','))
ok('temps réel: page visible reçue', got.includes('PAGE_CREATED'), got.join(','))
ok('temps réel: page bloquée non reçue', !got.includes('PAGE_UPDATED'), got.join(','))
await a(`/api/workspaces/${WS}/members/${bobId}`, 'DELETE')
await bob.page.waitForTimeout(800)
ok('retiré: connexion temps réel fermée', await bob.page.evaluate(() => window.__closed))
ok('retiré: plus de permission par page', (await a(`/api/workspaces/${WS}/guests`)).guests.every((g) => g.user_id !== bobId))

// Déplacement vers un autre tableau : l'étiquette est recréée et la sous-tâche suit.
const full2 = await a(`/api/boards/${b2.id}`)
const moved = await a(`/api/boards/tasks/${task.id}/move`, 'PUT', { column_id: full2.columns[0].id, position: 65536 })
const after = await a(`/api/boards/${b2.id}`)
const inB2 = after.columns.flatMap((c) => c.tasks)
ok('autre tableau: tâche déplacée', inB2.some((t) => t.id === task.id))
ok('autre tableau: sous-tâche suivie', inB2.some((t) => t.id === sub.id))
ok('autre tableau: étiquette reprise', moved.labels.length === 1 && moved.labels[0].board_id === b2.id && moved.labels[0].name === 'Urgent', JSON.stringify(moved.labels))
ok('autre tableau: étiquette créée dans le tableau', after.labels.some((l) => l.name === 'Urgent'))
ok('ancien tableau: vide', (await a(`/api/boards/${b1.id}`)).columns.every((c) => c.tasks.length === 0))

ok('aucune erreur page', !alice.errors.length && !bob.errors.length, [...alice.errors, ...bob.errors].join(' | '))
await alice.browser.close()
await bob.browser.close()
