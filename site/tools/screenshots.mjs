// Captures d'écran de l'application pour le site (WebP, clair et sombre, français et anglais).
//   WS=<espace> BOARD=<tableau> DOC=<document> CHROMIUM=<chemin> node site/tools/screenshots.mjs [dossier-de-sortie] [fr|en] [dark|light] [écran…]
import { chromium } from 'playwright-core'
import fs from 'node:fs'
const { WS, BOARD, DOC, CHROMIUM } = process.env
const B = process.env.LOOTY || 'http://localhost:13095'
const out = process.argv[2] || 'site/web/public/shots'
const langs = (process.argv[3] || 'en,fr').split(','), themes = (process.argv[4] || 'dark,light').split(',')
const only = process.argv.slice(5)
const screens = {
  home: `/w/${WS}`,
  board: `/w/${WS}/board/${BOARD}`,
  doc: `/w/${WS}/doc/${DOC}`,
  calendar: `/w/${WS}/calendar`,
}
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
  const ctx = await browser.newContext({ viewport: { width: 1560, height: 960 }, deviceScaleFactor: 1.5, locale: lang === 'fr' ? 'fr-FR' : 'en-US', timezoneId: 'Europe/Paris' })
  await ctx.addInitScript(([lang, theme]) => { localStorage.setItem('looty.locale', lang); localStorage.setItem('looty.theme', theme) }, [lang, theme])
  const page = await ctx.newPage()
  await page.goto(`${B}/api/auth/dev-login?email=maya@acme.dev&redirect=/`); await page.waitForTimeout(800)
  const J = (p, m = 'GET', b) => page.evaluate(async ([p, m, b]) => (await fetch(p, { method: m, headers: { 'Content-Type': 'application/json' }, body: b ? JSON.stringify(b) : undefined })).status, [p, m, b])
  await J('/api/user/releases/seen', 'PUT', { version: process.env.VERSION || '0.16.8' })
  await J(`/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'write', boards: 'write', calendar: 'write' }, modesChosen: true })
  for (const [name, path] of Object.entries(screens)) {
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
