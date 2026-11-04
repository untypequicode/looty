// Mots-clés : en poser sur une tâche depuis son panneau, sur un document ; les retrouver avec la palette (#mot-clé).
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'write', boards: 'write', calendar: 'write' }, modesChosen: true })

const board = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Mots-clés', with_default_columns: true })
const col = (await api(`/api/boards/${board.id}`)).columns[0]
const task = await api(`/api/boards/columns/${col.id}/tasks`, 'POST', { title: 'Rendre le TP de réseaux' })
const doc = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Corrigé TD3' })

await p.goto(`${B}/w/${WS}/board/${board.id}?task=${task.id}`, { waitUntil: 'networkidle' }); await p.waitForTimeout(800)
const prop = p.locator('.prop', { hasText: /Mots-clés|Keywords/ })
await prop.locator('.kw-add').click(); await p.waitForTimeout(200)
await p.keyboard.type('INF301'); await p.keyboard.press('Enter'); await p.waitForTimeout(600)
await p.keyboard.type('partiel'); await p.keyboard.press('Enter'); await p.waitForTimeout(600)
await p.keyboard.press('Escape')
let kw = await api(`/api/keywords/task/${task.id}`)
ok('keywords added from the task panel', JSON.stringify(kw.keywords) === '["INF301","partiel"]', JSON.stringify(kw))
ok('chips shown in the panel', await prop.locator('.kw-chip').count() === 2)
await p.screenshot({ path: OUT + 'keywords-task.png' })

await api(`/api/keywords/page/${doc.id}`, 'PUT', { keywords: ['inf301'] })
await p.goto(`${B}/w/${WS}/doc/${doc.id}`, { waitUntil: 'networkidle' }); await p.waitForTimeout(1000)
ok('document shows its keyword', await p.locator('.doc-kw .kw-chip', { hasText: 'inf301' }).count() === 1)

// Clic sur un mot-clé : palette ouverte sur « #inf301 », qui trouve la tâche et le document.
await p.locator('.doc-kw .kw-name').click(); await p.waitForTimeout(1200)
const input = p.locator('.cmd-search input')
ok('palette opened on the keyword', (await input.inputValue()) === '#inf301')
const items = await p.locator('.cmd-item').allInnerTexts()
ok('palette finds the task and the document', items.some((x) => /Rendre le TP/.test(x)) && items.some((x) => /Corrigé TD3/.test(x)), JSON.stringify(items))
ok('matched keyword shown', await p.locator('.cmd-item .cmd-kw').count() >= 2)
await p.screenshot({ path: OUT + 'keywords-palette.png' })
await p.keyboard.press('Escape')

await api(`/api/boards/${board.id}`, 'DELETE')
await api(`/api/workspaces/${WS}/pages/${doc.id}`, 'DELETE')
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'read', boards: 'read', calendar: 'read' }, modesChosen: true })
ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
