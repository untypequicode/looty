// Mini-vidéos des fonctions phares pour le site (5 à 15 s, sans son, en boucle), sur l'espace « Looty Démo »
// régénéré à chaque lancement par la commande backend `seed-demo` (mêmes conventions que screenshots.mjs).
//   CHROMIUM=<chemin> node site/tools/videos.mjs [dossier-de-sortie] [fr|en] [dark|light] [scénario…]
// LOOTY : adresse de Looty (défaut http://localhost:13095), avec LOOTY_DEV_LOGIN=true côté backend.
// SEED : commande qui génère le jeu de données (voir screenshots.mjs).
// Sortie, par scénario S, langue L et thème T : S-T-L.webm (VP9), S-T-L.mp4 (H.264) et S-T-L.webp (affiche).
// Budget : chaque fichier vidéo reste sous BUDGET_KB (600 par défaut) : l'encodage est refait avec une qualité plus
// basse puis une taille réduite tant qu'il le dépasse.
// Prérequis : Chromium, Playwright (playwright-core) et ffmpeg dans le PATH (FFMPEG : chemin de ffmpeg).
// DRY_RUN=1 : liste les fichiers attendus sans rien lancer.
import { chromium } from 'playwright-core'
import { execFileSync, execSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const { CHROMIUM } = process.env
const B = process.env.LOOTY || 'http://localhost:13095'
const FFMPEG = process.env.FFMPEG || 'ffmpeg'
const BUDGET_KB = Number(process.env.BUDGET_KB || 600)
const out = process.argv[2] || 'site/web/public/clips'
const langs = (process.argv[3] || 'en,fr').split(','), themes = (process.argv[4] || 'dark').split(',')
const only = process.argv.slice(5)
const SIZE = { width: 1280, height: 800 }
const backendDir = fileURLToPath(new URL('../../backend/', import.meta.url))

// --- Souris visible et mouvements fluides -------------------------------------------------------------------------

const cursorScript = () => {
  const dot = document.createElement('div')
  dot.style.cssText = 'position:fixed;z-index:2147483647;pointer-events:none;width:22px;height:22px;margin:-4px 0 0 -4px;border-radius:50% 50% 50% 0;transform:rotate(-45deg);background:#fff;border:2px solid #111;box-shadow:0 2px 6px rgba(0,0,0,.45);left:-50px;top:-50px;transition:transform .08s'
  const mount = () => document.documentElement.append(dot)
  document.documentElement ? mount() : addEventListener('DOMContentLoaded', mount)
  addEventListener('mousemove', (e) => { dot.style.left = e.clientX + 'px'; dot.style.top = e.clientY + 'px' }, true)
  addEventListener('mousedown', () => { dot.style.transform = 'rotate(-45deg) scale(.8)' }, true)
  addEventListener('mouseup', () => { dot.style.transform = 'rotate(-45deg)' }, true)
}

const pause = (page, ms = 700) => page.waitForTimeout(ms)
async function center(target) {
  const box = await target.boundingBox()
  if (!box) throw new Error('élément sans boîte (invisible ?)')
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 }
}
/** Déplace la souris vers l'élément en une trajectoire douce (pas de saut), puis marque une pause. */
async function glide(page, target, { steps = 35, wait = 250 } = {}) {
  const { x, y } = await center(target)
  await page.mouse.move(x, y, { steps })
  await pause(page, wait)
}
async function click(page, target, opts) {
  await glide(page, target, opts)
  await page.mouse.down(); await pause(page, 120); await page.mouse.up(); await pause(page, 500)
}
async function drag(page, from, to) {
  await glide(page, from)
  await page.mouse.down(); await pause(page, 250)
  const { x, y } = await center(to)
  await page.mouse.move(x, y, { steps: 50 })
  await pause(page, 350); await page.mouse.up(); await pause(page, 900)
}
/** Saisie lente, lisible à l'écran. */
const type = (page, text, delay = 90) => page.keyboard.type(text, { delay })

// --- Scénarios : un par fonction (contexte, jeu de données, langue) --------------------------------------------------
// Chaque scénario reçoit { page, demo (identifiants du jeu de données), lang, api(path, method, body), url(path),
// ready() (marque le début utile de la vidéo : tout ce qui précède, chargement compris, est coupé),
// second(email) (ouvre une deuxième personne, dans un autre contexte non filmé, avec un faux micro) }.
// Un scénario qui échoue est signalé et passe son tour : les autres sont quand même produits.

const scenarios = {
  // Une tâche glissée d'une colonne à l'autre.
  board: async ({ page, url, demo, ready }) => {
    await page.goto(url(`/w/${demo.workspace}/board/${demo.board}`), { waitUntil: 'networkidle' }); await pause(page, 1200)
    await ready()
    const columns = page.locator('section.col')
    await drag(page, columns.nth(0).locator('[data-task-id]').first(), columns.nth(1).locator('[data-task-id]').nth(1))
    await pause(page, 1500)
  },
  // Recherche par mot-clé dans la palette.
  search: async ({ page, url, demo, lang, ready }) => {
    await page.goto(url(`/w/${demo.workspace}`), { waitUntil: 'networkidle' }); await pause(page, 2500); await ready()
    await page.keyboard.press('Control+k'); await pause(page, 600)
    await type(page, '#' + (lang === 'fr' ? 'bêta' : 'beta'), 140)
    await pause(page, 2500)
  },
  // Une formule LaTeX dans un document.
  latex: async ({ page, url, demo, api, lang, ready }) => {
    const doc = await api(`/api/workspaces/${demo.workspace}/pages`, 'POST', { title: 'Maths' })
    await page.goto(url(`/w/${demo.workspace}/doc/${doc.id}`), { waitUntil: 'networkidle' }); await pause(page, 1500)
    await ready()
    await click(page, page.locator('.ProseMirror').first())
    await type(page, lang === 'fr' ? 'Une intégrale simple :' : 'A simple integral:', 70); await page.keyboard.press('Enter')
    await type(page, '$$ ', 150)
    await type(page, '\\int_0^1 x^2\\,dx = \\frac13', 90)
    await pause(page, 1200); await page.keyboard.press('Control+Enter')
    await pause(page, 3000)
  },
  // Agenda : on ouvre un événement, puis la semaine suivante.
  calendar: async ({ page, url, demo, ready }) => {
    await page.goto(url(`/w/${demo.workspace}/calendar`), { waitUntil: 'networkidle' }); await pause(page, 1500); await ready()
    await click(page, page.locator('.wk-ev').first()); await pause(page, 2200)
    await page.keyboard.press('Escape'); await pause(page, 600)
    await click(page, page.getByRole('button', { name: /^(Next|Suivant)/ }).first()); await pause(page, 1500)
    await click(page, page.locator('.wk-ev').first()); await pause(page, 2500)
  },
  // Édition à deux : une seconde personne (autre contexte, non filmé) écrit dans le même document.
  collab: async ({ page, url, demo, api, second, lang, ready }) => {
    const doc = await api(`/api/workspaces/${demo.workspace}/pages`, 'POST', { title: lang === 'fr' ? 'Notes de réunion' : 'Meeting notes' })
    await page.goto(url(`/w/${demo.workspace}/doc/${doc.id}`), { waitUntil: 'networkidle' }); await pause(page, 1500)
    const other = await second(demo.characters.find((c) => c.role === 'member').email)
    await other.goto(url(`/w/${demo.workspace}/doc/${doc.id}`), { waitUntil: 'networkidle' }); await pause(page, 1500); await ready()
    const editor = page.locator('.ProseMirror').first()
    await click(page, editor)
    await type(page, lang === 'fr' ? 'Ordre du jour : le lancement.' : 'Agenda: the launch.', 70)
    await other.locator('.ProseMirror').first().click(); await other.keyboard.press('Control+End'); await other.keyboard.press('Enter')
    await other.keyboard.type(lang === 'fr' ? 'Je prends les notes de la bêta.' : 'I will take the beta notes.', { delay: 70 })
    await pause(page, 2500)
    await click(page, editor)
    await page.keyboard.press('Control+End'); await page.keyboard.press('Enter')
    await type(page, lang === 'fr' ? 'Parfait, merci !' : 'Great, thanks!', 70)
    await pause(page, 2500)
  },
  // Annonce épinglée et vote à son sondage.
  poll: async ({ page, url, demo, ready }) => {
    await page.goto(url(`/w/${demo.workspace}/announcements`), { waitUntil: 'networkidle' }); await pause(page, 1500); await ready()
    const card = page.locator('article.an').filter({ has: page.locator('.poll-opt') }).first()
    await click(page, card.locator('.poll-opt').nth(1))
    await click(page, card.locator('button', { hasText: /^\s*(Voter|Vote)\s*$/ }))
    await pause(page, 2500)
  },
  // Notebook Pyodide : les cellules s'exécutent dans le navigateur (Maj+Entrée), le graphique apparaît.
  notebook: async ({ page, url, demo, api, ready }) => {
    // Préchauffage (non filmé) : Pyodide et matplotlib se téléchargent au premier lancement ; l'autre onglet les met en cache.
    const warm = await page.context().newPage()
    const scratch = await api(`/api/workspaces/${demo.workspace}/projects`, 'POST', { kind: 'notebook', name: 'warm', template: 'blank' })
    await warm.goto(url(`/w/${demo.workspace}/projects/${scratch.id}`), { waitUntil: 'networkidle' })
    await warm.locator('.nb .cell').first().locator('.cm-content').click()
    await warm.keyboard.type('import matplotlib.pyplot as plt\nplt.plot([1, 2])\nplt.show()')
    await warm.keyboard.press('Shift+Enter')
    await warm.locator('.nb .cell .cell-out img').first().waitFor({ timeout: 300000 })
    await api(`/api/projects/${scratch.id}`, 'DELETE').catch(() => {})
    await warm.close()
    const list = await api(`/api/workspaces/${demo.workspace}/projects`)
    const nb = (Array.isArray(list) ? list : list.projects).find((p) => p.kind === 'notebook')
    await page.goto(url(`/w/${demo.workspace}/projects/${nb.id}`), { waitUntil: 'networkidle' })
    const cells = page.locator('.nb .cell')
    await cells.first().waitFor({ timeout: 15000 }); await pause(page, 1200); await ready()
    for (const i of [1, 2, 3]) {
      await click(page, cells.nth(i).locator('.cm-content'))
      await page.keyboard.press('Shift+Enter')
      await cells.nth(i).locator('.cell-prompt', { hasText: /\d/ }).waitFor({ timeout: 120000 })
      await pause(page, 900)
    }
    await cells.nth(3).locator('img').first().scrollIntoViewIfNeeded().catch(() => {})
    await pause(page, 3000)
  },
  // R dans le navigateur (WebR) : choix du moteur puis un calcul.
  webr: async ({ page, url, demo, api, ready }) => {
    const nb = await api(`/api/workspaces/${demo.workspace}/projects`, 'POST', { kind: 'notebook', name: 'R', template: 'blank' })
    await page.goto(url(`/w/${demo.workspace}/projects/${nb.id}`), { waitUntil: 'networkidle' })
    const cell = page.locator('.nb .cell').first()
    await cell.waitFor({ timeout: 15000 }); await pause(page, 1000); await ready()
    await click(page, page.locator('[data-testid=nb-engine]'))
    await click(page, page.getByRole('dialog').getByText(/R \((navigateur|browser)\)/).first())
    await click(page, page.getByRole('dialog').getByRole('button', { name: /^(Utiliser|Use)$/ }))
    await click(page, cell.locator('.cm-content'))
    await type(page, 'x <- c(2, 4, 6, 8)\nsummary(x)', 60)
    await page.keyboard.press('Control+Enter')
    await cell.locator('.cell-out').getByText(/Median/).waitFor({ timeout: 180000 })
    await pause(page, 2500)
  },
  // Appel vocal : la barre d'appel, une deuxième personne rejoint (faux micro).
  call: async ({ page, url, demo, second, ready }) => {
    await page.goto(url(`/w/${demo.workspace}/doc/${demo.launch}`), { waitUntil: 'networkidle' }); await pause(page, 1200); await ready()
    const start = (p) => p.getByRole('button', { name: /^(Start call|Lancer un appel|Join call|Rejoindre l’appel)$/ })
    await click(page, start(page).first())
    await page.getByRole('region', { name: /^(Voice call|Appel vocal)$/ }).waitFor({ timeout: 15000 }); await pause(page, 1800)
    const other = await second(demo.characters.find((c) => c.role === 'member').email)
    await other.goto(url(`/w/${demo.workspace}/doc/${demo.launch}`), { waitUntil: 'networkidle' })
    await start(other).first().click()
    await pause(page, 5000)
    await click(page, page.getByRole('button', { name: /^(Mute|Couper le micro)$/ }))
    await pause(page, 2000)
  },
}
// Non scripté : application Android hors ligne puis synchro (demande un émulateur, voir #156).

// --- Encodage ffmpeg : WebM (VP9) + MP4 (H.264) + affiche WebP, sous le budget (BUDGET_KB) par fichier ------------------

const kb = (f) => Math.round(fs.statSync(f).size / 1024)
function encode(src, base, cut = 0) {
  // Qualité puis largeur décroissantes jusqu'à tenir dans le budget.
  const attempts = [[38, 1024], [42, 1024], [46, 960], [50, 800]]
  const codecs = {
    webm: (crf) => ['-c:v', 'libvpx-vp9', '-b:v', '0', '-crf', String(crf), '-row-mt', '1'],
    mp4: (crf) => ['-c:v', 'libx264', '-crf', String(crf - 8), '-preset', 'slow', '-pix_fmt', 'yuv420p', '-movflags', '+faststart'],
  }
  for (const ext of ['webm', 'mp4']) {
    for (const [crf, width] of attempts) {
      execFileSync(FFMPEG, ['-y', '-loglevel', 'error', '-ss', String(cut), '-i', src, '-an', '-vf', `scale=${width}:-2,fps=20`, ...codecs[ext](crf), `${base}.${ext}`])
      if (kb(`${base}.${ext}`) <= BUDGET_KB) break
    }
    const size = kb(`${base}.${ext}`)
    console.log(`${base}.${ext}`, size + ' KB', size > BUDGET_KB ? `(AU-DESSUS du budget de ${BUDGET_KB} KB : raccourcir le scénario)` : '')
  }
  execFileSync(FFMPEG, ['-y', '-loglevel', 'error', '-ss', String(cut + Number(process.env.POSTER_AT || 3)), '-i', src, '-frames:v', '1', '-vf', 'scale=1024:-2', '-c:v', 'libwebp', '-quality', '72', `${base}.webp`])
  console.log(`${base}.webp`, kb(`${base}.webp`) + ' KB')
}

// --- Lancement ----------------------------------------------------------------------------------------------------

const names = Object.keys(scenarios).filter((n) => !only.length || only.includes(n))
if (process.env.DRY_RUN) {
  for (const lang of langs) for (const theme of themes) for (const n of names) console.log(`${n}-${theme}-${lang}`)
  process.exit(0)
}
const seed = process.env.SEED || `${backendDir}target/debug/backend seed-demo`
const demo = JSON.parse(execSync(`${seed} ${langs.join(' ')}`, { cwd: backendDir, env: process.env }).toString())
fs.mkdirSync(out, { recursive: true })
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'looty-clips-'))
// Faux micro et connexions WebRTC locales (appel vocal), sans fenêtre de permission.
const browser = await chromium.launch({
  executablePath: CHROMIUM,
  args: ['--no-sandbox', '--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--allow-loopback-in-peer-connection', '--disable-features=WebRtcHideLocalIpsWithMdns'],
})
let failed = 0

async function signIn(page, email) {
  await page.goto(`${B}/api/auth/dev-login?email=${encodeURIComponent(email)}&redirect=/`); await pause(page, 800)
  await page.evaluate(async () => {
    const { version } = await (await fetch('/api/health')).json()
    await fetch('/api/user/releases/seen', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ version }) })
  })
}

for (const lang of langs) for (const theme of themes) for (const name of names) {
  const admin = demo[lang].characters.find((c) => c.role === 'admin')
  const dir = path.join(tmp, `${name}-${theme}-${lang}`)
  const opts = { viewport: SIZE, locale: lang === 'fr' ? 'fr-FR' : 'en-US', timezoneId: 'Europe/Paris', permissions: ['microphone'] }
  const init = ([lang, theme]) => { localStorage.setItem('looty.locale', lang); localStorage.setItem('looty.theme', theme) }
  const ctx = await browser.newContext({ ...opts, recordVideo: { dir, size: SIZE } })
  await ctx.addInitScript(init, [lang, theme])
  await ctx.addInitScript(cursorScript)
  const extra = []
  const second = async (email) => {
    const c = await browser.newContext(opts)
    await c.addInitScript(init, [lang, theme])
    extra.push(c)
    const p = await c.newPage()
    await signIn(p, email)
    return p
  }
  const page = await ctx.newPage()
  const t0 = Date.now()
  let cut = 0 // début utile de la vidéo (après le chargement), en secondes
  const ready = async () => { cut = Math.max(0, (Date.now() - t0) / 1000 - 0.3) }
  try {
    await signIn(page, admin.email)
    const api = (p, method = 'GET', body) => page.evaluate(async ({ p, method, body }) => {
      const r = await fetch(p, { method, headers: { 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
      return r.json()
    }, { p, method, body })
    await scenarios[name]({ page, demo: demo[lang], lang, api, second, ready, url: (p) => B + p })
  } catch (e) {
    failed++
    console.error(`${name}-${theme}-${lang}: ${e.message.split('\n')[0]}`)
    for (const c of extra) await c.close()
    await ctx.close()
    continue
  }
  const video = page.video()
  for (const c of extra) await c.close()
  await ctx.close()
  encode(await video.path(), `${out}/${name}-${theme}-${lang}`, cut)
}
await browser.close()
fs.rmSync(tmp, { recursive: true, force: true })
if (failed) { console.error(`${failed} scénario(s) en échec`); process.exit(1) }
