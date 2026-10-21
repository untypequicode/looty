// Lance les tests e2e contre une instance de Looty déjà démarrée (E2E_BASE) : prépare un espace de travail relié au
// faux Moodle, exécute chaque test de tests/ (ou ceux dont le nom contient l'argument) et résume.
//   LOOTY_DEV_LOGIN=true côté backend ; Moodle simulé sur 127.0.0.1:18300 (lib/mock-moodle.mjs).
import { spawn } from 'node:child_process'
import { readFileSync, readdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { server } from './lib/mock-moodle.mjs'
import { MOODLE_TOKEN } from './lib/tokens.mjs'

const B = process.env.E2E_BASE || 'http://localhost:3000'

await new Promise((resolve) => server.listen(18300, '127.0.0.1', resolve))

const login = await fetch(`${B}/api/auth/dev-login?email=alice@x.io&redirect=/`, { redirect: 'manual' })
const cookie = login.headers.getSetCookie().map((c) => c.split(';')[0]).join('; ')
if (!cookie) throw new Error(`dev-login refused (${login.status}): is LOOTY_DEV_LOGIN=true set on the backend?`)
const call = async (path, method = 'GET', body) => {
  const r = await fetch(`${B}${path}`, { method, headers: { cookie, 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
  if (!r.ok) throw new Error(`${method} ${path} -> ${r.status} ${await r.text()}`)
  return r.json().catch(() => null)
}

const releases = JSON.parse(readFileSync(new URL('../backend/releases.json', import.meta.url))).releases
await call('/api/user/releases/seen', 'PUT', { version: releases.find((r) => r.platform === 'web').version })
const { workspace } = await call('/api/workspaces', 'POST', { name: `e2e ${Date.now()}` })
const WS = workspace.id
// Évite la fenêtre « Sur quoi travaillez-vous ? » du premier accès.
await call(`/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'read', boards: 'read', calendar: 'read' }, modesChosen: true })
await call(`/api/workspaces/${WS}/moodle/settings`, 'PUT', { url: 'http://127.0.0.1:18300' })
await call(`/api/workspaces/${WS}/moodle/connect`, 'POST', { token: MOODLE_TOKEN })

const dir = fileURLToPath(new URL('./tests/', import.meta.url))
const filter = process.argv[2]
const files = readdirSync(dir).filter((f) => f.endsWith('.mjs') && (!filter || f.includes(filter))).sort()
let failed = 0
for (const f of files) {
  console.log(`\n# ${f}`)
  // Asynchrone : le faux Moodle vit dans ce processus et doit répondre pendant le test.
  const child = spawn('node', [dir + f], { env: { ...process.env, WS }, stdio: ['ignore', 'pipe', 'inherit'] })
  let out = ''
  child.stdout.on('data', (d) => { out += d; process.stdout.write(d) })
  const timer = setTimeout(() => child.kill(), 300_000)
  const status = await new Promise((resolve) => child.on('close', resolve))
  clearTimeout(timer)
  const fails = (out.match(/^FAIL/gm) || []).length
  if (status !== 0 || fails) { failed++; console.log(`=> ${f}: ${fails} failed check(s), exit ${status}`) }
}
console.log(`\n${files.length - failed}/${files.length} test files passed`)
server.close()
process.exit(failed ? 1 : 0)
