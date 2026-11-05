// Éditeur de notebook : mode commande (comme Jupyter), presse-papiers des cellules, Z,
// glisser pour réordonner, repli, sommaire, scinder / fusionner, durée d'exécution.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const { browser, page: p, errors } = await launch(1400, 950)
await p.context().grantPermissions(['clipboard-read', 'clipboard-write'], { origin: B })
await login(p)
const api = (path, method, body) => call(p, path, method, body)

const nb = await api(`/api/workspaces/${WS}/projects`, 'POST', { kind: 'notebook', name: 'Raccourcis', template: 'blank' })
await p.goto(`${B}/w/${WS}/projects/${nb.id}`, { waitUntil: 'networkidle' })
const cells = p.locator('.nb .cell')
await cells.first().waitFor({ timeout: 15000 })
const sources = () => p.evaluate(() => [...document.querySelectorAll('.nb .cell')].map((c) => c.querySelector('.cm-content')?.textContent ?? c.querySelector('.cell-md')?.textContent ?? ''))
const types = () => p.evaluate(() => [...document.querySelectorAll('.nb .cell')].map((c) => (c.classList.contains('markdown') ? 'md' : 'code')))
// Le collage lit le presse-papiers de façon asynchrone : on attend le nombre de cellules.
const count = async (n) => { for (let i = 0; i < 40 && (await cells.count()) !== n; i++) await p.waitForTimeout(100); return cells.count() }
const mode = () => p.locator('[data-testid=nb-mode]').textContent()

// Première cellule : du code, puis Échap → mode commande.
await cells.first().locator('.cm-content').click()
await p.keyboard.type('a = 1')
ok('edit mode while typing', /Edit|Édition/.test(await mode()))
await p.keyboard.press('Escape')
ok('Escape: command mode', /Command|Commande/.test(await mode()))

// B ajoute en dessous, A au-dessus, sans entrer en édition.
await p.keyboard.press('b')
await p.keyboard.press('a')
ok('A / B add cells', (await cells.count()) === 3, String(await cells.count()))
ok('still in command mode', /Command|Commande/.test(await mode()))
// Navigation, Entrée pour éditer.
await p.keyboard.press('ArrowUp')
await p.keyboard.press('k')
await p.keyboard.press('j')
await p.keyboard.press('Enter')
await p.keyboard.type('# Intro')
await p.keyboard.press('Escape')
await p.keyboard.press('m')
ok('M turns the cell into Markdown', (await types())[1] === 'md', JSON.stringify(await types()))
await p.keyboard.press('y')
ok('Y turns it back into code', (await types())[1] === 'code')
await p.keyboard.press('m')

// DD supprime, Z rétablit à sa place.
await p.keyboard.press('ArrowDown')
await p.keyboard.press('d')
await p.keyboard.press('d')
ok('DD deletes', (await cells.count()) === 2)
await p.keyboard.press('z')
ok('Z restores the deleted cell', (await cells.count()) === 3)
ok('restored at its place', (await types()).join() === 'code,md,code', JSON.stringify(await types()))

// C / V : copie dans le presse-papiers (JSON nbformat), collée en dessous.
await cells.first().locator('.cell-prompt').click()
await p.keyboard.press('c')
const clip = await p.evaluate(() => navigator.clipboard.readText())
let parsed = null
try { parsed = JSON.parse(clip) } catch {}
ok('clipboard holds nbformat cells', Array.isArray(parsed) && parsed[0]?.cell_type === 'code' && parsed[0].source.join('') === 'a = 1', clip.slice(0, 200))
await p.keyboard.press('v')
ok('V pastes below', (await count(4)) === 4 && (await sources())[1] === 'a = 1', JSON.stringify(await sources()))
// Collage d'une cellule venant de JupyterLab (liste de cellules nbformat).
await p.evaluate(() => navigator.clipboard.writeText(JSON.stringify([{ cell_type: 'markdown', metadata: {}, source: ['## Depuis Jupyter'] }])))
await p.keyboard.press('v')
await count(5)
ok('paste nbformat JSON from elsewhere', (await types())[2] === 'md' && (await sources())[2].includes('Depuis Jupyter'), JSON.stringify(await sources()))
// X coupe.
await p.keyboard.press('x')
ok('X cuts', (await cells.count()) === 4)

// Dupliquer (outils de la cellule).
await cells.first().hover()
await cells.first().locator('.cell-tools').getByRole('button', { name: /^(Duplicate|Dupliquer)$/ }).click()
ok('duplicate from the cell tools', (await cells.count()) === 5 && (await sources())[1] === 'a = 1')

// Sommaire.
await p.locator('[data-testid=nb-toc-toggle]').click()
await p.locator('[data-testid=nb-toc]').getByText('Intro').waitFor({ timeout: 5000 })
ok('table of contents lists headings', true)

// Scinder (Ctrl+Maj+-) puis fusionner (Maj+M).
const last = cells.last()
await last.locator('.cm-content').click()
await p.keyboard.type('x = 1\ny = 2')
await p.keyboard.press('ArrowUp')
await p.keyboard.press('End')
await p.keyboard.press('Control+Shift+Minus')
ok('split at the cursor', (await cells.count()) === 6 && (await sources()).slice(-2).join('|') === 'x = 1|y = 2', JSON.stringify(await sources()))
await p.keyboard.press('Escape')
await p.keyboard.press('k')
await p.keyboard.press('Shift+M')
const merged = await cells.last().locator('.cm-line').allTextContents()
ok('Shift+M merges with the next cell', (await cells.count()) === 5 && merged.join('|') === 'x = 1|y = 2', JSON.stringify(merged))

// Glisser-déposer : la dernière cellule en tête.
const before = await sources()
await cells.last().locator('.cell-grip').dragTo(cells.first(), { targetPosition: { x: 200, y: 4 } })
const afterDrag = await sources()
ok('drag and drop reorders', afterDrag[0] === before.at(-1) && afterDrag.length === before.length, JSON.stringify(afterDrag))

// Exécution : durée affichée et enregistrée, repli de la sortie.
const first = cells.first()
await first.locator('.cell-run').click()
await first.locator('[data-testid=cell-time]').waitFor({ timeout: 90000 })
ok('duration shown under the cell', /Ran in|Exécutée en/.test(await first.locator('[data-testid=cell-time]').textContent()))
await first.locator('.cell-gutter .cell-fold').click()
ok('input folded', (await first.locator('.cell-hidden').count()) === 1)
await p.waitForTimeout(3500)
const proj = await api(`/api/projects/${nb.id}`)
const file = proj.files.find((f) => f.path === 'notebook.ipynb')
const ipynb = await p.evaluate(async (u) => (await fetch(u)).json(), `/api/projects/${nb.id}/files/${file.id}/raw`)
const meta = ipynb.cells[0].metadata
ok('timing saved like JupyterLab', !!meta.execution?.['iopub.execute_input'] && !!meta.execution?.['shell.execute_reply'], JSON.stringify(meta))
ok('folding saved in metadata.jupyter', meta.jupyter?.source_hidden === true, JSON.stringify(meta))
await p.screenshot({ path: OUT + 'notebook-editor.png' })

ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
