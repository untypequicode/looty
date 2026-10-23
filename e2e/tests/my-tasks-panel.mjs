import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
const me = await api('/api/auth/me')
const uid = me.user?.id ?? me.id
const board = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Panneau', with_default_columns: true })
const col = (await api(`/api/boards/${board.id}`)).columns[0]
const mk = async (title) => {
  const t = await api(`/api/boards/columns/${col.id}/tasks`, 'POST', { title, assignees: [uid] })
  return t.task?.id ?? t.id
}
const t1 = await mk('Première tâche'), t2 = await mk('Seconde tâche')

// Mes tâches : un clic ouvre le panneau sans quitter la page.
await p.goto(`${B}/w/${WS}/my-tasks`, { waitUntil: 'networkidle' })
await p.locator('.mt-row', { hasText: 'Première tâche' }).click(); await p.waitForTimeout(1200)
ok('clic: reste sur Mes tâches', new URL(p.url()).pathname === `/w/${WS}/my-tasks`, p.url())
ok('clic: ?task= dans l’adresse', new URL(p.url()).searchParams.get('task') === t1)
ok('panneau visible, liste visible', (await p.locator('.panel').count()) === 1 && (await p.locator('.mt-row').count()) === 2)
await p.screenshot({ path: OUT + 'my-tasks-panel.png' })
// Autre ligne : le panneau passe à l’autre tâche.
await p.locator('.mt-row', { hasText: 'Seconde tâche' }).click(); await p.waitForTimeout(800)
ok('autre ligne: panneau sur la 2e tâche', new URL(p.url()).searchParams.get('task') === t2 && (await p.locator('.panel').count()) === 1)
// Rechargement avec ?task=
await p.reload({ waitUntil: 'networkidle' }); await p.waitForTimeout(1200)
ok('rechargement: panneau rouvert', (await p.locator('.panel').count()) === 1)
// Fermeture
await p.keyboard.press('Escape'); await p.waitForTimeout(800)
ok('Échap: panneau fermé, adresse nettoyée', (await p.locator('.panel').count()) === 0 && !new URL(p.url()).searchParams.has('task'))

// Onglets : chaque onglet garde sa page après des allers-retours rapides.
await p.goto(`${B}/w/${WS}/my-tasks`, { waitUntil: 'networkidle' })
const tabs = () => p.evaluate((ws) => JSON.parse(localStorage.getItem(`looty.tabs.${ws}`) || '{}'), WS)
await p.keyboard.press('Alt+t'); await p.waitForTimeout(500)
await p.goto(`${B}/w/${WS}/calendar`, { waitUntil: 'networkidle' }); await p.waitForTimeout(500)
let s = await tabs()
ok('2 onglets', s.tabs?.length >= 2, JSON.stringify(s.tabs?.map((t) => t.path)))
for (let i = 0; i < 4; i++) { await p.keyboard.press('Alt+1'); await p.keyboard.press('Alt+2') }
await p.waitForTimeout(1500)
s = await tabs()
const paths = s.tabs.map((t) => new URL(t.path, B).pathname)
ok('onglets: chemins conservés après bascules rapides', paths[0] === `/w/${WS}/my-tasks` && paths[1] === `/w/${WS}/calendar`, paths.join(' | '))
ok('onglet actif = page affichée', new URL(s.tabs.find((t) => t.id === s.active).path, B).pathname === new URL(p.url()).pathname)
ok('aucune erreur page', errors.length === 0, errors.join(' / '))
await browser.close()
