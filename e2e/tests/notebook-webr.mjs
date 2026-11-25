// R dans le navigateur (WebR) : aucun serveur, aucun CDN. Choix du moteur « R (navigateur) », 1+1, data frame, graphique,
// et vérification qu'aucune requête ne part vers un autre domaine que l'application (le dépôt de paquets n'est contacté
// que si le code installe un paquet). Le moteur est servi par /webr/ : le test n'a besoin d'aucun accès Internet.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const { browser, page: p, errors } = await launch(1400, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
const external = []
p.on('request', (r) => {
  const u = r.url()
  if (!u.startsWith(B) && !/^(data|blob|about):/.test(u) && !u.startsWith('ws')) external.push(u)
})

const nb = await api(`/api/workspaces/${WS}/projects`, 'POST', { kind: 'notebook', name: 'R navigateur', template: 'blank' })
await p.goto(`${B}/w/${WS}/projects/${nb.id}`, { waitUntil: 'networkidle' })
await p.locator('.nb .cell').first().waitFor({ timeout: 15000 })
const cell = () => p.locator('.nb .cell').first()
const out = () => cell().locator('.cell-out')
const runCode = async (code) => {
  await cell().locator('.cm-content').click()
  await p.keyboard.press('Control+a')
  await p.keyboard.type(code)
  await p.keyboard.press('Control+Enter')
}

// Moteur : « R (navigateur) » dans la boîte de dialogue, enregistré dans le notebook.
await p.locator('[data-testid=nb-engine]').click()
await p.getByRole('dialog').getByText(/R \((navigateur|browser)\)/).first().click()
await p.locator('[data-testid=nb-engine-webr-hint]').waitFor({ timeout: 5000 })
await p.getByRole('dialog').getByRole('button', { name: /^(Utiliser|Use)$/ }).click()
await p.waitForTimeout(500)
ok('engine button shows R (WebR)', /WebR/.test(await p.locator('[data-testid=nb-engine]').textContent()))
const before = external.length

// (a) 1+1
await runCode('1+1')
await out().getByText('[1] 2').waitFor({ timeout: 180000 })
ok('1+1 returns 2 with WebR', /\[1\] 2/.test(await out().innerText()))

// (b) data frame en tableau HTML ; état conservé entre cellules
await runCode('df <- data.frame(a = 1:3, b = c("x", "y", "z"))\ndf')
await out().locator('table').waitFor({ timeout: 60000 })
ok('data frame rendered as an HTML table', (await out().locator('table tbody tr').count()) === 3)

// (c) graphique → image PNG
await runCode('plot(1:10, main = "WebR")')
await out().locator('img').waitFor({ timeout: 60000 })
ok('plot rendered as an image', (await out().locator('img').count()) === 1)

// (d) erreur R
await runCode('stop("boom")')
await out().getByText('boom').first().waitFor({ timeout: 30000 })
ok('R error shown', /boom/.test(await out().innerText()))

// (d2) panneaux R : l'environnement liste `df` (requête silencieuse vers WebR)
await p.locator('[data-testid=nb-r-panels-toggle]').click()
await p.getByText('data.frame', { exact: true }).first().waitFor({ timeout: 30000 })
ok('R panel lists the data frame from the WebR session', true)

// (e) rien n'est parti vers un tiers pendant l'exécution
const leaked = external.slice(before)
ok('no third-party request while running R', leaked.length === 0, leaked.join(' '))

// (f) rechargement : le moteur est mémorisé dans le notebook
await p.reload({ waitUntil: 'networkidle' })
await p.locator('.nb .cell').first().waitFor({ timeout: 15000 })
ok('engine persists after reload', /WebR/.test(await p.locator('[data-testid=nb-engine]').textContent()))
await p.screenshot({ path: `${OUT}notebook-webr.png` })
ok('no page error', errors.length === 0, errors.join(' | '))
await browser.close()
