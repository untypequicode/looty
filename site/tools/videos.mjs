// Mini-vidéos des fonctions phares pour le site (5 à 15 s, sans son, en boucle), sur l'espace « Looty Démo »
// régénéré à chaque lancement par la commande backend `seed-demo` (mêmes conventions que screenshots.mjs).
//   CHROMIUM=<chemin> node site/tools/videos.mjs [dossier-de-sortie] [fr|en] [dark|light] [scénario…]
// LOOTY : adresse de Looty (défaut http://localhost:13095), avec LOOTY_DEV_LOGIN=true côté backend.
// SEED : commande qui génère le jeu de données (voir screenshots.mjs).
// Sortie, par scénario S, langue L et thème T : S-T-L.webm (VP9), S-T-L.mp4 (H.264) et S-T-L.webp (affiche).
// Les vidéos ne sont PAS versionnées (site/web/public/clips/ est ignoré par git) : elles sont refaites en CI (job
// manuel « Site clips ») ou à la main, puis copiées sur le serveur du site. Le composant <Clip> du site se replie
// proprement quand elles manquent. Prérequis : Chromium, Playwright (playwright-core) et ffmpeg dans le PATH.
// FFMPEG : chemin de ffmpeg (défaut « ffmpeg »). DRY_RUN=1 : liste les scénarios sans rien lancer.
import { chromium } from 'playwright-core'
import { execFileSync, execSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const { CHROMIUM } = process.env
const B = process.env.LOOTY || 'http://localhost:13095'
const FFMPEG = process.env.FFMPEG || 'ffmpeg'
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
// Chaque scénario reçoit { page, demo (identifiants du jeu de données), lang, api(path, method, body), url(path) }.
// Un scénario qui échoue est signalé et passe son tour : les autres sont quand même produits.

const scenarios = {
  // Une tâche glissée d'une colonne à l'autre.
  board: async ({ page, url, demo, api }) => {
    await page.goto(url(`/w/${demo.workspace}/board/${demo.board}`), { waitUntil: 'networkidle' }); await pause(page, 1200)
    const edit = page.locator('.page-header, header').getByRole('button', { name: /^(Edit|Modifier)$/ })
    if (await edit.count()) await click(page, edit.first())
    const columns = page.locator('[data-column-id]')
    const card = page.locator('[data-task-id]').first()
    await drag(page, card, columns.nth(1))
    await pause(page, 1500)
  },
  // Recherche par mot-clé dans la palette.
  search: async ({ page, url, demo, lang }) => {
    await page.goto(url(`/w/${demo.workspace}`), { waitUntil: 'networkidle' }); await pause(page, 1200)
    await page.keyboard.press('Control+k'); await pause(page, 600)
    const input = page.locator('.cmd-search input')
    await type(page, '#' + (lang === 'fr' ? 'partiel' : 'exam'), 140)
    await pause(page, 2500)
    void input
  },
  // Une formule LaTeX dans un document.
  latex: async ({ page, url, demo, api }) => {
    const doc = await api(`/api/workspaces/${demo.workspace}/pages`, 'POST', { title: 'Maths' })
    await page.goto(url(`/w/${demo.workspace}/doc/${doc.id}`), { waitUntil: 'networkidle' }); await pause(page, 1500)
    await click(page, page.locator('.ProseMirror').first())
    await type(page, '$$\\int_0^1 x^2\\,dx = \\frac13$$', 110)
    await pause(page, 3000)
  },
  // Agenda : un événement glissé vers un autre jour.
  calendar: async ({ page, url, demo }) => {
    await page.goto(url(`/w/${demo.workspace}/calendar`), { waitUntil: 'networkidle' }); await pause(page, 1500)
    const event = page.locator('[data-event-id], .fc-event, .cal-event').first()
    const day = page.locator('[data-date], .fc-daygrid-day, .cal-day').nth(10)
    await drag(page, event, day)
    await pause(page, 1500)
  },
}
// Non encore scriptés (voir #156) : édition collaborative à deux curseurs, annonce et vote à un sondage, notebook
// Pyodide qui s'exécute, application Android hors ligne puis synchro (émulateur ou maquette de téléphone).

// --- Encodage ffmpeg : WebM (VP9) + MP4 (H.264) + affiche WebP, moins de 1 Mo par vidéo visé ---------------------------

function encode(src, base) {
  const common = ['-y', '-loglevel', 'error', '-i', src, '-an', '-vf', 'scale=1024:-2,fps=24']
  execFileSync(FFMPEG, [...common, '-c:v', 'libvpx-vp9', '-b:v', '0', '-crf', '38', '-row-mt', '1', `${base}.webm`])
  execFileSync(FFMPEG, [...common, '-c:v', 'libx264', '-crf', '30', '-preset', 'slow', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', `${base}.mp4`])
  execFileSync(FFMPEG, ['-y', '-loglevel', 'error', '-ss', '2', '-i', src, '-frames:v', '1', '-vf', 'scale=1024:-2', '-c:v', 'libwebp', '-quality', '75', `${base}.webp`])
  for (const ext of ['webm', 'mp4']) {
    const kb = Math.round(fs.statSync(`${base}.${ext}`).size / 1024)
    console.log(`${base}.${ext}`, kb + ' KB', kb > 1024 ? '(au-dessus de 1 Mo : raccourcir le scénario)' : '')
  }
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
const browser = await chromium.launch({ executablePath: CHROMIUM, args: ['--no-sandbox'] })
let failed = 0

for (const lang of langs) for (const theme of themes) for (const name of names) {
  const admin = demo[lang].characters.find((c) => c.role === 'admin')
  const dir = path.join(tmp, `${name}-${theme}-${lang}`)
  const ctx = await browser.newContext({ viewport: SIZE, recordVideo: { dir, size: SIZE }, locale: lang === 'fr' ? 'fr-FR' : 'en-US', timezoneId: 'Europe/Paris' })
  await ctx.addInitScript(([lang, theme]) => { localStorage.setItem('looty.locale', lang); localStorage.setItem('looty.theme', theme) }, [lang, theme])
  await ctx.addInitScript(cursorScript)
  const page = await ctx.newPage()
  try {
    await page.goto(`${B}/api/auth/dev-login?email=${encodeURIComponent(admin.email)}&redirect=/`); await pause(page, 800)
    await page.evaluate(async () => {
      const { version } = await (await fetch('/api/health')).json()
      await fetch('/api/user/releases/seen', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ version }) })
    })
    const api = (p, method = 'GET', body) => page.evaluate(async ({ p, method, body }) => {
      const r = await fetch(p, { method, headers: { 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
      return r.json()
    }, { p, method, body })
    await scenarios[name]({ page, demo: demo[lang], lang, api, url: (p) => B + p })
  } catch (e) {
    failed++
    console.error(`${name}-${theme}-${lang}: ${e.message}`)
    await ctx.close()
    continue
  }
  const video = page.video()
  await ctx.close()
  encode(await video.path(), `${out}/${name}-${theme}-${lang}`)
}
await browser.close()
fs.rmSync(tmp, { recursive: true, force: true })
if (failed) { console.error(`${failed} scénario(s) en échec`); process.exit(1) }
