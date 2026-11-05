// Captures d'écran de l'application pour le site (WebP, clair et sombre, français et anglais), sur l'espace
// « Looty Démo » régénéré à chaque lancement par la commande backend `seed-demo` (dates du jour).
//   CHROMIUM=<chemin> node site/tools/screenshots.mjs [dossier-de-sortie] [fr|en] [dark|light] [écran…]
// LOOTY : adresse de Looty (défaut http://localhost:13095), avec LOOTY_DEV_LOGIN=true côté backend.
// SEED : commande qui génère le jeu de données (défaut : backend/target/debug/backend seed-demo, lancé depuis
// backend/ avec DATABASE_URL), par exemple « docker exec looty-backend /app/backend seed-demo ».
import { chromium } from 'playwright-core'
import { execSync } from 'node:child_process'
import fs from 'node:fs'
import { fileURLToPath } from 'node:url'
const { CHROMIUM } = process.env
const B = process.env.LOOTY || 'http://localhost:13095'
const out = process.argv[2] || 'site/web/public/shots'
const langs = (process.argv[3] || 'en,fr').split(','), themes = (process.argv[4] || 'dark,light').split(',')
const only = process.argv.slice(5)
const backendDir = fileURLToPath(new URL('../../backend/', import.meta.url))
const seed = process.env.SEED || `${backendDir}target/debug/backend seed-demo`
const demo = JSON.parse(execSync(`${seed} ${langs.join(' ')}`, { cwd: backendDir, env: process.env }).toString())
const screens = ({ workspace, board, launch }) => ({
  home: `/w/${workspace}`,
  board: `/w/${workspace}/board/${board}`,
  doc: `/w/${workspace}/doc/${launch}`,
  calendar: `/w/${workspace}/calendar`,
})
fs.mkdirSync(out, { recursive: true })
const browser = await chromium.launch({ executablePath: CHROMIUM, args: ['--no-sandbox'] })
async function toWebp(page, png, width, quality) {
  return page.evaluate(async ({ b64, width, quality }) => {
    const img = new Image(); img.src = 'data:image/png;base64,' + b64; await img.decode()
    const w = Math.min(width, img.width), h = Math.round(img.height * (w / img.width))
    const c = document.createElement('canvas'); c.width = w; c.height = h
    const g = c.getContext('2d'); g.imageSmoothingQuality = 'high'; g.drawImage(img, 0, 0, w, h)
    return c.toDataURL('image/webp', quality).split(',')[1]
  }, { b64: png.toString('base64'), width, quality })
}
for (const lang of langs) for (const theme of themes) {
  const admin = demo[lang].characters.find((c) => c.role === 'admin')
  const ctx = await browser.newContext({ viewport: { width: 1560, height: 960 }, deviceScaleFactor: 1.5, locale: lang === 'fr' ? 'fr-FR' : 'en-US', timezoneId: 'Europe/Paris' })
  await ctx.addInitScript(([lang, theme]) => { localStorage.setItem('looty.locale', lang); localStorage.setItem('looty.theme', theme) }, [lang, theme])
  const page = await ctx.newPage()
  // Profil, langue, nouveautés vues et modes d'affichage : déjà réglés par seed-demo.
  await page.goto(`${B}/api/auth/dev-login?email=${encodeURIComponent(admin.email)}&redirect=/`); await page.waitForTimeout(800)
  for (const [name, path] of Object.entries(screens(demo[lang]))) {
    if (only.length && !only.includes(name)) continue
    await page.goto(B + path, { waitUntil: 'networkidle' }); await page.waitForTimeout(name === 'doc' ? 3500 : 1800)
    // Première visite de l’accueil : fenêtre « Personnaliser » à valider.
    const keep = page.getByRole('button', { name: /Save my home|Enregistrer mon accueil|Enregistrer/ })
    if (name === 'home' && await keep.count()) { await keep.first().click(); await page.waitForTimeout(1200) }
    const png = await page.screenshot({ type: 'png' })
    const webp = await toWebp(page, png, 1800, 0.82)
    const file = `${out}/${name}-${theme}-${lang}.webp`
    fs.writeFileSync(file, Buffer.from(webp, 'base64'))
    console.log(file, Math.round(fs.statSync(file).size / 1024) + ' KB')
  }
  await ctx.close()
}
await browser.close()
