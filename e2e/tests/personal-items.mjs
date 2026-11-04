// Éléments personnels : un document personnel rangé dans l'espace, invisible des autres membres ;
// une tâche rendue personnelle depuis son panneau.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'write', boards: 'write', calendar: 'write' }, modesChosen: true })

// Bob rejoint l'espace dans son propre navigateur.
const ctx = await browser.newContext()
const bob = await ctx.newPage()
await login(bob, 'bob@x.io')
const inv = await api(`/api/workspaces/${WS}/invites`, 'POST', { role: 'member' })
await call(bob, `/api/invites/${inv.token ?? inv.url.split('/').pop()}/accept`, 'POST')

// Document personnel depuis « Nouveau ».
await p.goto(`${B}/w/${WS}/docs`, { waitUntil: 'networkidle' }); await p.waitForTimeout(800)
await p.locator('button', { hasText: /Nouveau|New/ }).first().click(); await p.waitForTimeout(300)
await p.locator('.menu-item, [role=menuitem]', { hasText: /Document personnel|Personal document/ }).first().click()
await p.waitForURL(/\/doc\//); await p.waitForTimeout(1200)
const docId = p.url().split('/doc/')[1].split(/[?#]/)[0]
await p.locator('textarea.doc-title').fill('Mon corrigé perso'); await p.waitForTimeout(1200)
const mine = await api(`/api/workspaces/${WS}/pages/${docId}`)
ok('personal document created', mine.private_to && mine.title === 'Mon corrigé perso', JSON.stringify(mine).slice(0, 160))
ok('badge shown in the document', await p.locator('.doc-personal').count() === 1)
ok('lock shown in the sidebar tree', await p.locator('.tree-label', { hasText: 'Mon corrigé perso' }).locator('xpath=..').locator('.lucide-lock-keyhole').count() >= 1)
await p.screenshot({ path: OUT + 'personal-doc.png' })
const bobPages = await call(bob, `/api/workspaces/${WS}/pages`)
ok('other member does not see it', !(bobPages.pages ?? bobPages).some((x) => x.id === docId))
ok('other member gets 404', (await call(bob, `/api/workspaces/${WS}/pages/${docId}`)).status === 404)

// Tâche : la rendre personnelle depuis le menu du panneau.
const board = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Perso e2e', with_default_columns: true })
const col = (await api(`/api/boards/${board.id}`)).columns[0]
const task = await api(`/api/boards/columns/${col.id}/tasks`, 'POST', { title: 'Relire mes notes' })
await p.goto(`${B}/w/${WS}/board/${board.id}?task=${task.id}`, { waitUntil: 'networkidle' }); await p.waitForTimeout(800)
await p.locator('aside:has(textarea) button:has(.lucide-ellipsis)').first().click()
await p.waitForTimeout(300)
await p.locator('.menu-item, [role=menuitem]', { hasText: /Rendre personnel|Make personal/ }).first().click(); await p.waitForTimeout(300)
await p.locator('[role=dialog] button', { hasText: /Rendre personnel|Make personal/ }).last().click(); await p.waitForTimeout(1200)
const t2 = (await api(`/api/boards/tasks/${task.id}`)).task
ok('task made personal from the panel', !!t2?.private_to, JSON.stringify(t2).slice(0, 160))
const bobBoard = await call(bob, `/api/boards/${board.id}`)
ok('panel stays open for the owner', (await p.locator('aside textarea').first().inputValue().catch(() => '')) === 'Relire mes notes')
ok('hidden from the other member', !bobBoard.columns.flatMap((c) => c.tasks).some((x) => x.id === task.id))
await p.screenshot({ path: OUT + 'personal-task.png' })

await api(`/api/boards/${board.id}`, 'DELETE')
await api(`/api/workspaces/${WS}/pages/${docId}`, 'DELETE')
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'read', boards: 'read', calendar: 'read' }, modesChosen: true })
ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
