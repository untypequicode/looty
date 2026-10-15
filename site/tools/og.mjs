// Images d'aperçu pour les réseaux sociaux (1200×630, PNG) : site/web/public/og-<langue>.png
//   CHROMIUM=<chemin> node site/tools/og.mjs
import { chromium } from 'playwright-core'
import fs from 'node:fs'
const pub = new URL('../web/public/', import.meta.url).pathname
const shot = (lang) => 'data:image/webp;base64,' + fs.readFileSync(`${pub}shots/home-dark-${lang}.webp`).toString('base64')
// Police Inter (paquet du site) : même rendu que les pages.
const inter = 'data:font/woff2;base64,' + fs.readFileSync(new URL('../web/node_modules/@fontsource-variable/inter/files/inter-latin-wght-normal.woff2', import.meta.url)).toString('base64')
const logo = 'data:image/svg+xml;base64,' + fs.readFileSync(`${pub}favicon.svg`).toString('base64')
const text = {
  en: ['Tasks, documents and calendar', 'for your team, in one calm place.', 'Free and self-hosted, with apps for Android, Linux and Windows'],
  fr: ['Tâches, documents et agenda', 'pour votre équipe, au même endroit.', 'Libre et auto-hébergé, avec des applications Android, Linux et Windows'],
}
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM, args: ['--no-sandbox'] })
const page = await browser.newPage({ viewport: { width: 1200, height: 630 } })
for (const lang of ['en', 'fr']) {
  const [l1, l2, foot] = text[lang]
  await page.setContent(`<!doctype html><html><head><style>
    @font-face { font-family: 'Inter'; src: url(${inter}) format('woff2'); font-weight: 100 900; }
    body { margin: 0; width: 1200px; height: 630px; overflow: hidden; background: #0b0b0d; color: #ededef; font-family: 'Inter', system-ui, sans-serif; position: relative; }
    .glow { position: absolute; inset: -200px -100px auto; height: 700px; background: radial-gradient(50% 50% at 30% 40%, rgba(113,112,255,.35), transparent 70%); }
    .copy { position: absolute; left: 72px; top: 78px; width: 560px; }
    .brand { display: flex; align-items: center; gap: 14px; font-size: 30px; font-weight: 700; } .brand img { width: 44px; height: 44px; border-radius: 10px; }
    h1 { font-size: 50px; line-height: 1.08; letter-spacing: -.03em; margin: 44px 0 0; font-weight: 750; }
    h1 em { font-style: normal; background: linear-gradient(120deg, #a5a4ff, #c9a6ff 60%, #ffb27a); -webkit-background-clip: text; color: transparent; }
    p { position: absolute; left: 72px; bottom: 64px; width: 520px; margin: 0; color: #a0a0ab; font-size: 21px; line-height: 1.45; }
    .shot { position: absolute; left: 640px; top: 96px; width: 820px; border-radius: 14px; box-shadow: 0 30px 80px -20px rgba(0,0,0,.8), 0 0 0 1px rgba(255,255,255,.12); }
  </style></head><body><div class="glow"></div>
    <div class="copy"><div class="brand"><img src="${logo}">Looty</div><h1>${l1}<br><em>${l2}</em></h1></div>
    <p>${foot}</p><img class="shot" src="${shot(lang)}"></body></html>`)
  await page.evaluate(() => document.fonts.ready)
  await page.screenshot({ path: `${pub}og-${lang}.png` })
  console.log(`${pub}og-${lang}.png`)
}
await browser.close()
