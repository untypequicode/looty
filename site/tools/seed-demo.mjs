// Espace de démonstration « Acme Studio » pour les captures d'écran du site (pile de développement locale).
//   node site/tools/seed-demo.mjs   (LOOTY=http://localhost:13095, DB=docker exec looty-dev-pg …)
// Crée l'équipe, un tableau de lancement, des documents et une semaine d'événements ; affiche l'identifiant de l'espace.
import { execFileSync } from 'node:child_process'
const B = process.env.LOOTY || 'http://localhost:13095'
const team = [
  ['maya@acme.dev', 'Maya Chen'], ['leo@acme.dev', 'Leo Martin'], ['sara@acme.dev', 'Sara Okafor'],
  ['noah@acme.dev', 'Noah Fischer'], ['ines@acme.dev', 'Ines Moreau'],
]
async function login(email, name) {
  const r = await fetch(`${B}/api/auth/dev-login?email=${email}&name=${encodeURIComponent(name)}&redirect=/`, { redirect: 'manual' })
  return (r.headers.getSetCookie?.() ?? []).map((c) => c.split(';')[0]).join('; ')
}
const cookies = {}
for (const [e, n] of team) cookies[e] = await login(e, n)
const me = cookies['maya@acme.dev']
const api = async (method, path, body) => {
  const r = await fetch(B + path, { method, headers: { cookie: me, 'content-type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
  const t = await r.text()
  if (!r.ok) throw new Error(`${method} ${path} → ${r.status} ${t.slice(0, 200)}`)
  return t ? JSON.parse(t) : null
}
const psql = (sql) => execFileSync('docker', ['exec', 'looty-dev-pg', 'psql', '-U', 'postgres', '-d', 'looty_db', '-tAc', sql]).toString().trim()

const ws = await api('POST', '/api/workspaces', { name: 'Acme Studio' })
const W = ws.id ?? ws.workspace?.id
const ids = {}
for (const [e] of team) ids[e] = psql(`select id from users where email='${e}'`)
for (const [e] of team.slice(1)) psql(`insert into workspace_members (workspace_id, user_id, role) values ('${W}', '${ids[e]}', 'member') on conflict do nothing`)

// --- Tableau de lancement
const board = await api('POST', `/api/workspaces/${W}/boards`, { name: 'Product launch', with_default_columns: false })
const cols = {}
for (const c of ['Backlog', 'In progress', 'Review', 'Done']) cols[c] = (await api('POST', `/api/boards/${board.id}/columns`, { name: c })).id
const labels = {}
for (const [n, c] of [['Design', '#b18cff'], ['Engineering', '#4ea7fc'], ['Marketing', '#f2994a'], ['Bug', '#eb5757']]) labels[n] = (await api('POST', `/api/boards/${board.id}/labels`, { name: n, color: c })).id
const day = (n) => new Date(Date.now() + n * 86400000).toISOString()
const u = (e) => ids[e]
const tasks = [
  ['Backlog', 'Plan the launch week', 'Marketing', 'sara@acme.dev', 1, 9],
  ['Backlog', 'Write the release notes', 'Marketing', 'maya@acme.dev', 0, 7],
  ['Backlog', 'Record the demo video', 'Design', 'ines@acme.dev', 2, 10],
  ['In progress', 'Redesign the onboarding flow', 'Design', 'maya@acme.dev', 3, 3],
  ['In progress', 'Offline mode for the mobile app', 'Engineering', 'leo@acme.dev', 4, 5],
  ['In progress', 'Pricing page copy', 'Marketing', 'sara@acme.dev', 2, 4],
  ['Review', 'Fix calendar sync on Android', 'Bug', 'noah@acme.dev', 4, 1],
  ['Review', 'Accessibility audit', 'Design', 'ines@acme.dev', 3, 2],
  ['Done', 'Set up the CI pipeline', 'Engineering', 'noah@acme.dev', 2, -2],
  ['Done', 'Choose the new icon', 'Design', 'maya@acme.dev', 1, -4],
  ['Done', 'Summarize the beta feedback', 'Marketing', 'sara@acme.dev', 1, -3],
]
const made = {}
for (const [col, title, label, who, prio, due] of tasks) {
  const t = await api('POST', `/api/boards/columns/${cols[col]}/tasks`, { title, priority: prio, due_date: day(due), labels: [labels[label]], assignees: [u(who)] })
  made[title] = t.id ?? t.task?.id
}
// quelques tâches terminées et sous-tâches
for (const t of ['Set up the CI pipeline', 'Choose the new icon', 'Summarize the beta feedback']) await api('PUT', `/api/boards/tasks/${made[t]}/toggle_validation`).catch(() => {})
for (const s of ['Wireframes', 'User testing', 'Final screens']) await api('POST', `/api/boards/tasks/${made['Redesign the onboarding flow']}/subtasks`, { title: s }).catch(() => {})

// --- Documents
const page = (title, icon, content, parent_id) => api('POST', `/api/workspaces/${W}/pages`, { title, icon, content, parent_id })
const launch = await page('Launch plan', '🚀', `<p>Everything we need to ship <strong>Looty 2.0</strong> on <strong>November 12</strong>. This page is edited live by the whole team.</p>
<h2>Goals</h2>
<ul><li><p>Ship the new onboarding and offline mode on every platform</p></li><li><p>Keep the launch week calm: no big changes after <em>freeze day</em></p></li><li><p>Reach <strong>500 teams</strong> in the first month</p></li></ul>
<blockquote data-notice-type="info" data-type="notice" class="callout callout-info"><p><strong>Freeze day is November 5.</strong> Only fixes after that, reviewed by two people.</p></blockquote>
<h2>Release flow</h2>
<div code="graph LR; A[Design] --> B[Build]; B --> C[Review]; C --> D[Ship]; C --> B" data-type="mermaid"><pre>graph LR; A[Design] --> B[Build]; B --> C[Review]; C --> D[Ship]; C --> B</pre></div>
<h2>Timeline</h2>
<table><tbody><tr><th><p>Week</p></th><th><p>Focus</p></th><th><p>Owner</p></th></tr><tr><td><p>Oct 19</p></td><td><p>Onboarding and offline mode</p></td><td><p>Maya, Leo</p></td></tr><tr><td><p>Oct 26</p></td><td><p>Mobile polish and accessibility</p></td><td><p>Ines, Noah</p></td></tr><tr><td><p>Nov 2</p></td><td><p>Release candidate, freeze day</p></td><td><p>Noah</p></td></tr><tr><td><p>Nov 9</p></td><td><p>Launch week</p></td><td><p>Sara</p></td></tr></tbody></table>
<h2>Checklist</h2>
<ul data-type="taskList"><li data-checked="true" data-type="taskItem"><label><input type="checkbox" checked="checked"><span></span></label><div><p>Pick the launch date</p></div></li><li data-checked="true" data-type="taskItem"><label><input type="checkbox" checked="checked"><span></span></label><div><p>Choose the new icon</p></div></li><li data-checked="false" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>Write the release notes</p></div></li><li data-checked="false" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>Book the demo with the first customers</p></div></li></ul>`)
await page('Meeting notes · Monday', '📝', '<p>Standup and planning.</p>', launch.id)
await page('Brand guidelines', '🎨', '<p>Colors, typography and tone of voice.</p>')
await page('Engineering handbook', '📚', '<p>How we build, review and ship.</p>')

// --- Semaine d'événements (lundi de la semaine courante)
const now = new Date(); const monday = new Date(now); monday.setDate(now.getDate() - ((now.getDay() + 6) % 7)); monday.setHours(0, 0, 0, 0)
const at = (d, h, m = 0) => { const x = new Date(monday); x.setDate(monday.getDate() + d); x.setHours(h, m, 0, 0); return x.toISOString() }
const tz = 'Europe/Paris'
for (const [d, h, m, len, title, repeat] of [
  [0, 9, 30, 15, 'Daily standup', { freq: 'daily', interval: 1 }], [0, 10, 30, 60, 'Sprint planning'], [1, 14, 0, 60, 'Design review'],
  [2, 11, 0, 45, 'Offline mode sync'], [3, 16, 0, 60, 'Customer demo'], [4, 12, 30, 90, 'Team lunch'], [4, 15, 0, 45, 'Retrospective'],
]) {
  await api('POST', `/api/workspaces/${W}/events`, { title, all_day: false, start: at(d, h, m), end: at(d, h + Math.floor((m + len) / 60), (m + len) % 60), tz, ...(repeat ? { repeat } : {}), keep_recurrence: false })
}
console.log(JSON.stringify({ workspace: W, board: board.id, launch: launch.id }))
