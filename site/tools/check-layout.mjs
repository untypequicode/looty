// Layout check of the home page (regression: #host column squeezed by an unbreakable <pre>).
//   CHROMIUM=$(which chromium) node site/tools/check-layout.mjs [http://localhost:4321]   (needs playwright-core)
// Fails if a page overflows horizontally, if a 2-column section has a column under 25 % of its width
// or with no height, or if the console reports an error.
import { createRequire } from 'node:module'
const { chromium } = createRequire(process.cwd() + '/')('playwright-core')
const base = (process.argv[2] || 'http://localhost:4321').replace(/\/$/, '')
const b = await chromium.launch({ executablePath: process.env.CHROMIUM, args: ['--no-sandbox'] })
let bad = 0
for (const lang of ['', '/fr']) for (const w of [1560, 1280, 1024, 768, 390]) {
  const p = await b.newPage({ viewport: { width: w, height: 900 } })
  const errs = []
  p.on('console', (m) => m.type() === 'error' && !/status of 404/.test(m.text()) && errs.push(m.text()))
  p.on('response', (r) => r.status() >= 400 && !r.url().endsWith('/latest.json') && errs.push(r.status() + ' ' + r.url()))  // latest.json is served by the site server, absent from a static dist
  p.on('pageerror', (e) => errs.push(String(e)))
  await p.goto(`${base}${lang}/`)
  await p.waitForTimeout(300)
  const r = await p.evaluate(() => {
    const out = []
    if (document.documentElement.scrollWidth > innerWidth) out.push('horizontal overflow')
    for (const g of document.querySelectorAll('.host, .row')) {
      const gw = g.getBoundingClientRect().width
      const wide = getComputedStyle(g).gridTemplateColumns.split(' ').length > 1
      for (const c of g.children) {
        const r = c.getBoundingClientRect()
        if (r.height < 10) out.push(`${g.className}: empty column`)
        if (wide && r.width < gw * 0.25) out.push(`${g.className}: column ${r.width | 0}px of ${gw | 0}px`)
      }
    }
    return out
  })
  for (const m of [...r, ...errs]) { console.error(`${lang || '/'} @${w}: ${m}`); bad++ }
  await p.close()
}
await b.close()
if (bad) process.exit(1)
console.log('layout ok')
