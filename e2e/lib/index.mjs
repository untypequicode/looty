// Aides communes des tests e2e. Variables : E2E_BASE (interface, défaut http://localhost:3000),
// CHROMIUM (navigateur, défaut : celui installé par `npx playwright-core install chromium`), WS (espace, fourni par run.mjs).
import { chromium } from 'playwright-core'
import { mkdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

export const B = process.env.E2E_BASE || 'http://localhost:3000'
export const WS = process.env.WS
export const OUT = fileURLToPath(new URL('../out/', import.meta.url))
mkdirSync(OUT, { recursive: true })

let failures = 0
export const ok = (name, cond, extra = '') => {
  if (!cond) failures++
  console.log(`${cond ? 'PASS' : 'FAIL'} ${name} ${extra}`)
}
export const failed = () => failures

export const launch = (width = 1400, height = 1000) => chromium.launch({ executablePath: process.env.CHROMIUM || undefined, args: ['--no-sandbox'] })
  .then(async (browser) => {
    const ctx = await browser.newContext({ viewport: { width, height } })
    const page = await ctx.newPage()
    const errors = []
    page.on('pageerror', (e) => errors.push(e.message))
    return { browser, page, errors }
  })

export const login = async (page, email = 'alice@x.io') => {
  await page.goto(`${B}/api/auth/dev-login?email=${email}&redirect=/`)
  await page.waitForTimeout(800)
}

export const api = (page, path, method = 'GET', body) => page.evaluate(async ([path, method, body]) => {
  const r = await fetch(path, { method, headers: { 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
  return r.ok ? r.json().catch(() => null) : { status: r.status }
}, [path, method, body])
