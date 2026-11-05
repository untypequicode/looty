// Notebook exécuté sur un serveur Jupyter externe (relayé par le backend, jeton jamais exposé).
// Demande un serveur Jupyter joignable par le backend : E2E_JUPYTER_URL (+ E2E_JUPYTER_TOKEN),
// backend lancé avec JUPYTER_ALLOW_PRIVATE=true. Sans E2E_JUPYTER_URL, le test est ignoré.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const JUPYTER = process.env.E2E_JUPYTER_URL
const TOKEN = process.env.E2E_JUPYTER_TOKEN || ''
if (!JUPYTER) {
  console.log('SKIP notebook-jupyter (E2E_JUPYTER_URL not set)')
  process.exit(0)
}

const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
const raw = (path, method = 'GET', body) => p.evaluate(async ([path, method, body]) => {
  const r = await fetch(path, { method, headers: { 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
  return { status: r.status, text: await r.text() }
}, [path, method, body])

const nb = await api(`/api/workspaces/${WS}/projects`, 'POST', { kind: 'notebook', name: 'Calcul distant', template: 'blank' })
const base = `/api/projects/${nb.id}/jupyter`

// --- Configuration (API) ---
const none = await api(base)
ok('no Jupyter server by default', none.configured === false && none.can_edit === true, JSON.stringify(none))
const inUrl = await raw(base, 'PUT', { url: `${JUPYTER}/lab?token=${TOKEN || 'x'}` })
ok('token in the address refused', inUrl.status === 400 && inUrl.text.includes('jupyter_token_in_url'), inUrl.text)
const badTest = await raw(`${base}/test`, 'POST', { url: JUPYTER, token: 'wrong-token' })
ok('wrong token: explicit error', badTest.status === 400 && badTest.text.includes('jupyter_auth_failed'), badTest.text)
const goodTest = await api(`${base}/test`, 'POST', { url: `${JUPYTER}/lab`, token: TOKEN })
ok('test lists kernels', Array.isArray(goodTest.kernels) && goodTest.kernels.some((k) => k.name === 'python3'), JSON.stringify(goodTest))
const saved = await raw(base, 'PUT', { url: `${JUPYTER}/lab`, token: TOKEN, kernel: 'python3' })
ok('server saved (address normalized)', saved.status === 200 && JSON.parse(saved.text).url === `${JUPYTER.replace(/\/$/, '')}/`, saved.text)
ok('token never sent back', !TOKEN || !saved.text.includes(TOKEN), saved.text)
const got = await raw(base)
ok('config: token stored, hidden', JSON.parse(got.text).has_token === !!TOKEN && (!TOKEN || !got.text.includes(TOKEN)), got.text)

// --- Notebook : Pyodide par défaut, puis Jupyter ---
await p.goto(`${B}/w/${WS}/projects/${nb.id}`, { waitUntil: 'networkidle' })
await p.locator('.nb .cell').first().waitFor({ timeout: 15000 })
const engineBtn = p.locator('[data-testid=nb-engine]')
ok('Pyodide is the default engine', (await engineBtn.textContent()).includes('Pyodide'))
await engineBtn.click()
await p.locator('.dialog').getByText(/^(Jupyter server|Serveur Jupyter)$/).first().click()
await p.locator('.dialog select').first().waitFor({ timeout: 10000 })
await p.locator('.dialog').getByRole('button', { name: /^(Use|Utiliser)$/ }).click()
await p.waitForTimeout(500)
ok('engine switched to Jupyter', (await engineBtn.textContent()).includes('Jupyter'))

const cell = p.locator('.nb .cell').first()
await cell.locator('.cm-content').click()
await p.keyboard.type('import platform; print(platform.system())\n6 * 7')
await p.keyboard.press('Control+Enter')
await cell.locator('.cell-out').getByText('42').waitFor({ timeout: 30000 }).catch(async (e) => {
  await p.screenshot({ path: OUT + 'notebook-jupyter-fail.png' })
  console.log('kernel:', await p.locator('[data-testid=nb-kernel]').textContent(), '| cell:', await cell.textContent())
  throw e
})
const out = await cell.locator('.cell-out').textContent()
ok('cell runs on the remote kernel', out.includes('Linux') && !out.includes('Emscripten'), JSON.stringify(out))
ok('kernel status shown', /Jupyter/.test(await p.locator('[data-testid=nb-kernel]').textContent()))
ok('execution count from the kernel', (await cell.locator('.cell-prompt').textContent()).includes('1'))
await p.screenshot({ path: OUT + 'notebook-jupyter.png' })

// Variables gardées entre deux cellules, interruption.
await p.locator('.nb-end').getByRole('button').first().click()
await p.keyboard.type('import time\nfor i in range(60): time.sleep(0.5)')
await p.keyboard.press('Control+Enter')
await p.waitForTimeout(1500)
await p.locator('.nb-bar').getByRole('button', { name: /^(Interrupt|Interrompre)$/ }).click()
const second = p.locator('.nb .cell').nth(1)
await second.locator('.cell-out').getByText('KeyboardInterrupt').first().waitFor({ timeout: 15000 })
ok('interrupt stops the remote cell', true)

// Sorties partagées : une autre session voit le résultat, le fichier ne contient pas le jeton.
await p.waitForTimeout(2500)
const other = await browser.newPage()
await login(other)
await other.goto(`${B}/w/${WS}/projects/${nb.id}`, { waitUntil: 'networkidle' })
await other.locator('.nb .cell-out').first().waitFor({ timeout: 15000 })
ok('outputs visible to others', (await other.locator('.nb .cell').first().locator('.cell-out').textContent()).includes('Linux'))
await other.close()
const proj = await api(`/api/projects/${nb.id}`)
const file = proj.files.find((f) => f.path.endsWith('.ipynb'))
const ipynb = await raw(`/api/projects/${nb.id}/files/${file.id}/raw`)
const meta = JSON.parse(ipynb.text).metadata
ok('engine saved in the notebook', meta?.looty?.engine === 'jupyter' && meta?.kernelspec?.name === 'python3', JSON.stringify(meta))
ok('notebook file has no token', !TOKEN || !ipynb.text.includes(TOKEN))

// Jeton refusé : erreur explicite dans la cellule, pas de repli sur Pyodide.
if (TOKEN) {
  await api(base, 'PUT', { url: JUPYTER, token: 'wrong-token', kernel: 'python3' })
  await p.reload({ waitUntil: 'networkidle' })
  await p.locator('.nb .cell').first().waitFor({ timeout: 15000 })
  await p.locator('.nb .cell').first().locator('.cell-run').click()
  await p.locator('.nb .cell').first().locator('.cell-out .error').waitFor({ timeout: 20000 })
  const err = await p.locator('.nb .cell').first().locator('.cell-out').textContent()
  ok('auth error is explicit', /Token rejected|Jeton refusé/.test(err), JSON.stringify(err))
  ok('kernel status shows the error', /Token rejected|Jeton refusé/.test(await p.locator('[data-testid=nb-kernel]').textContent()))
  await api(base, 'PUT', { url: JUPYTER, token: TOKEN, kernel: 'python3' })
}

// Retour à Pyodide.
await engineBtn.click()
await p.locator('.dialog').getByText(/^(Browser \(Pyodide\)|Navigateur \(Pyodide\))$/).first().click()
await p.locator('.dialog').getByRole('button', { name: /^(Use|Utiliser)$/ }).click()
await p.waitForTimeout(500)
ok('back to Pyodide', (await engineBtn.textContent()).includes('Pyodide'))
ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
