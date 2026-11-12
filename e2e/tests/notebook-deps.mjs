// Notebook Pyodide modulaire : bouton « Installer » sur ModuleNotFoundError, requirements.txt (écrit et réinstallé
// au démarrage), import des fichiers .py du projet (relus avant chaque exécution). Demande l'accès à PyPI et au CDN Pyodide.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)

const nb = await api(`/api/workspaces/${WS}/projects`, 'POST', { kind: 'notebook', name: 'Dépendances', template: 'blank' })
const open = async () => {
  await p.goto(`${B}/w/${WS}/projects/${nb.id}`, { waitUntil: 'networkidle' })
  await p.locator('.nb .cell').first().waitFor({ timeout: 15000 })
}
await open()
const cell = () => p.locator('.nb .cell').first()
/** Remplace le code de la première cellule et l'exécute (Ctrl+Entrée : on reste dans la cellule). */
const runCode = async (code) => {
  await cell().locator('.cm-content').click()
  await p.keyboard.press('Control+a')
  await p.keyboard.type(code)
  await p.keyboard.press('Control+Enter')
}
const out = () => cell().locator('.cell-out')
const reqText = async () => {
  const proj = await api(`/api/projects/${nb.id}`)
  const f = proj.files.find((x) => x.path === 'requirements.txt')
  return f ? p.evaluate(async (u) => (await fetch(u)).text(), `/api/projects/${nb.id}/files/${f.id}/raw`) : null
}

// (a) import chess → erreur → bouton → installation → la cellule se relance.
await runCode('import chess\nprint("chess", chess.Board().fen()[:8])')
await out().getByText('ModuleNotFoundError').first().waitFor({ timeout: 120000 })
const btn = cell().locator('[data-testid=nb-install]')
await btn.waitFor({ timeout: 5000 })
ok('install button shown under the error', /chess/.test(await btn.textContent()), await btn.textContent())
await btn.click()
await out().getByText('chess rnbqkbnr').waitFor({ timeout: 180000 })
ok('cell re-runs after the install', /chess rnbqkbnr/.test(await out().innerText()))
ok('button gone once it works', (await cell().locator('[data-testid=nb-install]').count()) === 0)
await p.waitForTimeout(500)
const req = await reqText()
ok('requirements.txt contains chess', !!req && /^chess\s*$/m.test(req), JSON.stringify(req))

// (b) rechargement : le noyau repart de zéro, requirements.txt est réinstallé, sans ligne pip.
await open()
await runCode('import chess\nprint("again", chess.__name__)')
await out().getByText('again chess').waitFor({ timeout: 180000 })
ok('import works after reload without pip', /again chess/.test(await out().innerText()))

// (c) fichier du projet : import utils, puis modification relue sans redémarrer.
const create = await api(`/api/projects/${nb.id}/files`, 'POST', { path: 'utils.py', is_dir: false, content: 'def f():\n    return "v1"\n' })
await runCode('import utils\nprint("utils", utils.f())')
await out().getByText('utils v1').waitFor({ timeout: 60000 })
ok('import utils from the project root', /utils v1/.test(await out().innerText()))
await api(`/api/projects/${nb.id}/files/${create.id}`, 'PUT', { content: 'def f():\n    return "v2"\n' })
await cell().locator('.cell-run').click()
await out().getByText('utils v2').waitFor({ timeout: 60000 })
ok('modified utils.py is picked up without restart', /utils v2/.test(await out().innerText()))
await api(`/api/projects/${nb.id}/files`, 'POST', { path: 'pkg/mod.py', is_dir: false, content: 'VALUE = 7\n' })
await runCode('from pkg.mod import VALUE\nprint("pkg", VALUE)')
await out().getByText('pkg 7').waitFor({ timeout: 60000 })
ok('from pkg.mod import … (folder without __init__.py)', /pkg 7/.test(await out().innerText()))
// (d) %pip install avec guillemets et contrainte : importable ensuite, requirements.txt complété sans doublon de chess.
await runCode('%pip install chess "pyyaml>=6"\nimport yaml\nprint("yaml", yaml.safe_load("a: 1")["a"])')
await out().getByText('yaml 1').waitFor({ timeout: 180000 })
ok('import yaml works after %pip install', /yaml 1/.test(await out().innerText()))
let req2 = null
for (let i = 0; i < 20; i++) { req2 = await reqText(); if (req2 && /^pyyaml\s*>=\s*6\s*$/im.test(req2)) break; await p.waitForTimeout(500) }
ok('requirements.txt contains pyyaml>=6', !!req2 && /^pyyaml\s*>=\s*6\s*$/im.test(req2), JSON.stringify(req2))
ok('chess not duplicated in requirements.txt', !!req2 && (req2.match(/^chess\b/gim) ?? []).length === 1, JSON.stringify(req2))

// (e) bibliothèque sans version WebAssembly (roues natives seulement, absente de Pyodide) : note localisée.
await runCode('%pip install psycopg2-binary')
const note = cell().locator('[data-testid=nb-wasm-note]')
await note.waitFor({ timeout: 180000 })
const noteText = (await note.locator('p').innerText()).trim()
ok('no-WASM library shows the localized note', noteText.length > 20 && !/^notebook\./.test(noteText) && /navigateur|browser/i.test(noteText), noteText)
await p.screenshot({ path: OUT + 'notebook-deps.png' })

ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
