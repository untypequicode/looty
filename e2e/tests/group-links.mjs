// Annonce réservée à un groupe (et suivie) ; groupe relié à une tâche depuis son panneau.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'write', boards: 'write', calendar: 'write' }, modesChosen: true })

// Bob et Carol rejoignent l'espace (chacun dans son navigateur).
const people = {}
for (const who of ['bob', 'carol']) {
  const ctx = await browser.newContext()
  const page = await ctx.newPage()
  await login(page, `${who}@x.io`)
  const inv = await api(`/api/workspaces/${WS}/invites`, 'POST', { role: 'member' })
  await call(page, `/api/invites/${inv.token ?? inv.url.split('/').pop()}/accept`, 'POST')
  people[who] = { page, id: (await call(page, '/api/auth/me')).user.id }
}
const group = await api(`/api/workspaces/${WS}/groups`, 'POST', { name: 'TD2 e2e', members: [people.bob.id] })

// Publier pour @TD2 depuis la page Annonces.
await p.goto(`${B}/w/${WS}/announcements`, { waitUntil: 'networkidle' }); await p.waitForTimeout(800)
await p.locator('button', { hasText: /Nouvelle annonce|New announcement/ }).first().click(); await p.waitForTimeout(400)
const dlg = p.locator('[role=dialog]')
await dlg.locator('input').first().fill('Salle du TD2')
await dlg.locator('.af-audience').click(); await p.waitForTimeout(300)
await p.locator('.combo-item', { hasText: '@TD2 e2e' }).click(); await p.keyboard.press('Escape'); await p.waitForTimeout(200)
ok('audience shown in the form', /@TD2 e2e/.test(await dlg.locator('.af-audience').innerText()))
await dlg.locator('button', { hasText: /^\s*(Publier|Publish)\s*$/ }).click(); await p.waitForTimeout(1200)
const a = (await api(`/api/workspaces/${WS}/announcements`)).announcements.find((x) => x.title === 'Salle du TD2')
ok('published for the group', a?.targeted && a.audience.groups[0] === group.id, JSON.stringify(a ?? null).slice(0, 200))
ok('"For:" line shown to the author', /TD2 e2e/.test(await p.locator('article.an', { hasText: 'Salle du TD2' }).locator('.an-for').innerText()))
await p.screenshot({ path: OUT + 'announcement-audience.png' })
const sees = async (who) => (await call(people[who].page, `/api/workspaces/${WS}/announcements`)).announcements.some((x) => x.id === a.id)
ok('group member sees it, other member does not', await sees('bob') && !(await sees('carol')))
await api(`/api/workspaces/${WS}/groups/${group.id}`, 'PUT', { members: [people.bob.id, people.carol.id] })
ok('added to the group: now sees it', await sees('carol'))

// Tâche : relier le groupe depuis le panneau.
const board = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Groupes reliés', with_default_columns: true })
const col = (await api(`/api/boards/${board.id}`)).columns[0]
const task = await api(`/api/boards/columns/${col.id}/tasks`, 'POST', { title: 'Compte rendu TD2' })
await p.goto(`${B}/w/${WS}/board/${board.id}?task=${task.id}`, { waitUntil: 'networkidle' }); await p.waitForTimeout(800)
await p.locator('.prop', { hasText: /Personnes assignées|Assignees/ }).locator('button.prop-val').click(); await p.waitForTimeout(300)
await p.locator('.combo-item', { hasText: '@TD2 e2e' }).click(); await p.waitForTimeout(800); await p.keyboard.press('Escape')
let tk = (await api(`/api/boards/tasks/${task.id}`)).task
ok('group linked from the panel', tk.groups?.[0] === group.id && tk.assignees.includes(people.bob.id) && tk.assignees.includes(people.carol.id), JSON.stringify(tk).slice(0, 200))
await p.waitForTimeout(500)
ok('linked group shown in the panel', /@TD2 e2e/.test(await p.locator('.prop', { hasText: /Personnes assignées|Assignees/ }).innerText()))
await p.screenshot({ path: OUT + 'task-linked-group.png' })
await api(`/api/workspaces/${WS}/groups/${group.id}`, 'PUT', { members: [people.bob.id] })
tk = (await api(`/api/boards/tasks/${task.id}`)).task
ok('task follows the group', tk.assignees.includes(people.bob.id) && !tk.assignees.includes(people.carol.id))

await api(`/api/workspaces/${WS}/announcements/${a.id}`, 'DELETE')
await api(`/api/boards/${board.id}`, 'DELETE')
await api(`/api/workspaces/${WS}/groups/${group.id}`, 'DELETE')
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'read', boards: 'read', calendar: 'read' }, modesChosen: true })
ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
