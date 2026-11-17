// Multi-user load test for the Looty backend, with RSS / Postgres sampling.
//
// Needs a backend started with LOOTY_DEV_LOGIN=true and a Postgres running in a docker container.
// Every simulated user is a separate dev-login session of the same account (alice@x.io): it exercises
// the HTTP/WS code paths and the database, not the per-user permission matrix.
//
//   node scripts/loadtest.mjs --base http://127.0.0.1:18080 --pid <backend pid> --pg <container> \
//        [--users 60] [--duration 120] [--settle 30] [--out result.json] [--db looty_db --dbuser postgres]
//
// Seeding: a workspace with ~3000 pages (some with a few KB of yjs_state), ~3000 tasks, and 3 projects
// holding ~100 MB of binary files (downloaded as zip / bundle during the run). Seeding SQL goes through
// `docker exec <container> psql`. Output: JSON summary on stdout (peak RSS, settled RSS, p95, errors).
import { execFileSync, spawnSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { randomBytes } from 'node:crypto'

const arg = (n, d) => { const i = process.argv.indexOf(`--${n}`); return i > 0 ? process.argv[i + 1] : d }
const BASE = arg('base', 'http://127.0.0.1:18080')
const PID = arg('pid')
const PG = arg('pg')
const USERS = +arg('users', 60)
const DURATION = +arg('duration', 120) * 1000
const SETTLE = +arg('settle', 30) * 1000
const DB = arg('db', 'looty_db')
const DBUSER = arg('dbuser', 'postgres')
const OUT = arg('out')
if (!PID || !PG) { console.error('--pid and --pg are required'); process.exit(2) }

const psql = (sql) => execFileSync('docker', ['exec', '-i', PG, 'psql', '-q', '-tA', '-U', DBUSER, '-d', DB, '-v', 'ON_ERROR_STOP=1'], { input: sql, encoding: 'utf8' })
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const pick = (a) => a[Math.floor(Math.random() * a.length)]

// ---------- stats ----------
const lat = {} // label -> [ms]
const errs = {} // label -> count
const errSamples = {}
let total = 0
const rec = (label, ms, ok, info) => {
  total++
  ;(lat[label] ||= []).push(ms)
  if (!ok) { errs[label] = (errs[label] || 0) + 1; errSamples[label] ||= info }
}
const p = (arr, q) => { if (!arr?.length) return 0; const s = [...arr].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor(q * s.length))] }

// ---------- http ----------
async function login(i) {
  const r = await fetch(`${BASE}/api/auth/dev-login?email=alice@x.io&redirect=/`, { redirect: 'manual' })
  const cookie = r.headers.getSetCookie().map((c) => c.split(';')[0]).join('; ')
  if (!cookie) throw new Error(`dev-login failed (${r.status})`)
  return cookie
}
async function req(cookie, label, method, path, body, { raw = false, form = null } = {}) {
  const t = performance.now()
  try {
    const r = await fetch(`${BASE}${path}`, {
      method,
      headers: form ? { cookie } : { cookie, 'Content-Type': 'application/json' },
      body: form || (body ? JSON.stringify(body) : undefined),
    })
    let data = null
    if (raw) await r.arrayBuffer()
    else data = await r.json().catch(() => null)
    rec(label, performance.now() - t, r.ok, `${r.status}`)
    return r.ok ? data : null
  } catch (e) {
    rec(label, performance.now() - t, false, String(e.message || e))
    return null
  }
}
const mustJson = async (cookie, method, path, body) => {
  const r = await fetch(`${BASE}${path}`, { method, headers: { cookie, 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
  if (!r.ok) throw new Error(`${method} ${path} -> ${r.status} ${await r.text()}`)
  return r.json().catch(() => null)
}
const upload = (cookie, label, projectId, name, bytes) => {
  const form = new FormData()
  form.append('file', new Blob([bytes]), name)
  return req(cookie, label, 'POST', `/api/projects/${projectId}/upload`, null, { form })
}

// ---------- seed ----------
const admin = await login(0)
await mustJson(admin, 'PUT', '/api/user/releases/seen', { version: JSON.parse(readFileSync(new URL('../backend/releases.json', import.meta.url))).releases.find((r) => r.platform === 'web').version }).catch(() => {})
const { workspace } = await mustJson(admin, 'POST', '/api/workspaces', { name: `load ${Date.now()}` })
const WS = workspace.id
const bres = await mustJson(admin, 'POST', `/api/workspaces/${WS}/boards`, { name: 'Load', with_default_columns: true })
const BOARD = (bres.board || bres).id
const board = await mustJson(admin, 'GET', `/api/boards/${BOARD}`)
const columns = (board.columns || board.board?.columns || []).map((c) => c.id)
if (!columns.length) throw new Error('no columns in board: ' + JSON.stringify(board).slice(0, 300))
const COL = columns[0]

console.error('seeding SQL...')
const wsRow = null
const uid = wsRow || psql(`SELECT id FROM users WHERE email='alice@x.io';`).match(/[0-9a-f-]{36}/)[0]
// Pages: ~3000 with ~2 KB of text and, for one in three, ~6 KB of random yjs_state.
psql(`
INSERT INTO pages (workspace_id, title, content, created_by, yjs_state)
SELECT '${WS}', 'Seed page ' || g, repeat('lorem ipsum dolor sit amet ', 80),
       '${uid}', CASE WHEN g % 3 = 0 THEN decode(repeat(md5(g::text), 190), 'hex') ELSE NULL END
FROM generate_series(1, 3000) g;
`)
// Tasks: 3000 in the first column.
const taskCols = psql(`SELECT column_name FROM information_schema.columns WHERE table_name='tasks' AND is_nullable='NO' AND column_default IS NULL;`)
console.error('tasks required columns:', taskCols.replace(/\s+/g, ' ').trim())
for (let i = 0; i < 30; i++) {
  await Promise.all(Array.from({ length: 100 }, (_, j) => mustJson(admin, 'POST', `/api/boards/columns/${COL}/tasks`, { title: `Seed task ${i * 100 + j}`, description: 'seed '.repeat(40) })))
}
const pages = psql(`SELECT id FROM pages WHERE workspace_id='${WS}' LIMIT 200;`).match(/[0-9a-f-]{36}/g)
const tasks = (await mustJson(admin, 'GET', `/api/boards/${BOARD}`).then((b) => (b.columns || []).flatMap((c) => c.tasks || []))).map((t) => t.id)

const projects = []
for (let i = 0; i < 3; i++) {
  const pr = await mustJson(admin, 'POST', `/api/workspaces/${WS}/projects`, { kind: 'site', name: `proj${i}`, template: 'blank' })
  const id = (pr.project || pr).id
  for (const mb of [5, 15, 15]) await upload(admin, 'seed-upload', id, `blob${mb}-${i}.bin`, randomBytes(mb * 1024 * 1024))
  projects.push(id)
}
console.error(`seeded: ws=${WS} pages=3000 tasks~3000 projects=${projects.length}`)
for (const k of Object.keys(lat)) delete lat[k]
for (const k of Object.keys(errs)) delete errs[k]
total = 0

// ---------- sampler ----------
const samples = []
const rss = () => { try { return +readFileSync(`/proc/${PID}/status`, 'utf8').match(/VmRSS:\s+(\d+)/)[1] / 1024 } catch { return NaN } }
const t0 = Date.now()
let sampling = true
const sampler = (async () => {
  while (sampling) {
    const s = { t: (Date.now() - t0) / 1000, rssMB: rss() }
    const r = spawnSync('docker', ['stats', '--no-stream', '--format', '{{.MemUsage}}|{{.CPUPerc}}', PG], { encoding: 'utf8' })
    const m = r.stdout.trim().match(/([\d.]+)(MiB|GiB|KiB)[^|]*\|([\d.]+)%/)
    if (m) { s.pgMB = +m[1] * (m[2] === 'GiB' ? 1024 : m[2] === 'KiB' ? 1 / 1024 : 1); s.pgCpu = +m[3] }
    samples.push(s)
    await sleep(1500)
  }
})()

// ---------- users ----------
let stop = false
const cookies = await Promise.all(Array.from({ length: USERS }, (_, i) => login(i)))
const wsConns = []
for (const c of cookies.slice(0, Math.min(USERS, 60))) {
  try {
    const w = new WebSocket(`${BASE.replace('http', 'ws')}/api/ws?workspace_id=${WS}`, { headers: { cookie: c } })
    w.onopen = () => w.send(JSON.stringify({ type: 'USER_LOCATION', location: `page:${pick(pages)}`, status: 'active' }))
    w.onmessage = () => {}
    wsConns.push(w)
  } catch (e) { rec('ws-open', 0, false, String(e)) }
}

const actions = [
  [20, (c) => req(c, 'pages-list', 'GET', `/api/workspaces/${WS}/pages`)],
  [15, (c) => req(c, 'board-get', 'GET', `/api/boards/${BOARD}`)],
  [10, (c) => req(c, 'task-create', 'POST', `/api/boards/columns/${COL}/tasks`, { title: `T ${Math.random()}`, description: 'x'.repeat(500) })],
  [10, (c) => req(c, 'page-create', 'POST', `/api/workspaces/${WS}/pages`, { title: `P ${Math.random()}`, content: '<p>' + 'hello '.repeat(300) + '</p>' })],
  [8, (c) => req(c, 'page-get', 'GET', `/api/workspaces/${WS}/pages/${pick(pages)}`)],
  [8, (c) => req(c, 'task-comment', 'POST', `/api/boards/tasks/${pick(tasks)}/comments`, { content: 'comment ' + Math.random() })],
  [6, (c) => req(c, 'page-comment', 'POST', `/api/workspaces/${WS}/comments`, { kind: 'page', id: pick(pages), content: 'hi' })],
  [6, (c) => req(c, 'notifications', 'GET', `/api/notifications?workspace_id=${WS}`)],
  [5, (c) => req(c, 'my-tasks', 'GET', `/api/workspaces/${WS}/my_tasks`)],
  [4, (c) => req(c, 'graph', 'GET', `/api/workspaces/${WS}/graph`)],
  [3, (c) => req(c, 'export-csv', 'GET', `/api/boards/${BOARD}/export.csv`, null, { raw: true })],
  [3, (c) => req(c, 'search', 'GET', `/api/workspaces/${WS}/search?q=seed`)],
  [2.5, (c) => req(c, 'zip-download', 'GET', `/api/projects/${pick(projects)}/download`, null, { raw: true })],
  [1.5, (c) => req(c, 'bundle', 'GET', `/api/projects/${pick(projects)}/bundle`, null, { raw: true })],
  [1.2, (c) => upload(c, 'upload', pick(projects), `u${Math.random()}.bin`, randomBytes((1 + Math.floor(Math.random() * 40)) * 1024 * 1024))],
]
const sum = actions.reduce((a, x) => a + x[0], 0)
const choose = () => { let r = Math.random() * sum; for (const a of actions) if ((r -= a[0]) < 0) return a[1]; return actions[0][1] }

console.error(`load: ${USERS} users, ${wsConns.length} websockets, ${DURATION / 1000}s`)
const loop = async (c) => { while (!stop) { await choose()(c); await sleep(200 + Math.random() * 800) } }
const running = cookies.map(loop)
await sleep(DURATION)
stop = true
await Promise.all(running)
const loadEnd = (Date.now() - t0) / 1000
for (const w of wsConns) try { w.close() } catch {}
console.error(`settling ${SETTLE / 1000}s...`)
await sleep(SETTLE)
sampling = false
await sampler

// ---------- report ----------
const all = Object.values(lat).flat()
const during = samples.filter((s) => s.t <= loadEnd)
const after = samples.filter((s) => s.t > loadEnd + SETTLE / 1000 - 6)
const max = (a, k) => Math.max(...a.map((s) => s[k]).filter(Number.isFinite))
const avg = (a, k) => { const v = a.map((s) => s[k]).filter(Number.isFinite); return v.reduce((x, y) => x + y, 0) / (v.length || 1) }
const summary = {
  users: USERS, websockets: wsConns.length, requests: total, errors: Object.values(errs).reduce((a, b) => a + b, 0),
  peakRssMB: +max(during, 'rssMB').toFixed(0), settledRssMB: +avg(after, 'rssMB').toFixed(0),
  peakPgMemMB: +max(samples, 'pgMB').toFixed(0), settledPgMemMB: +avg(after, 'pgMB').toFixed(0),
  avgPgCpuPct: +avg(during, 'pgCpu').toFixed(0), peakPgCpuPct: +max(during, 'pgCpu').toFixed(0),
  p95ms: +p(all, 0.95).toFixed(0), p50ms: +p(all, 0.5).toFixed(0),
  perEndpoint: Object.fromEntries(Object.entries(lat).map(([k, v]) => [k, { n: v.length, p95: +p(v, 0.95).toFixed(0), errors: errs[k] || 0, firstError: errSamples[k] }])),
}
const text = JSON.stringify({ summary, samples }, null, 1)
if (OUT) writeFileSync(OUT, text)
console.log(JSON.stringify(summary, null, 1))
process.exit(0)
