// Jeu de données « Looty Démo » (#145) : la commande backend `seed-demo` crée l'espace modèle en français et en
// anglais, on le vérifie par l'API en se connectant comme chaque personnage, puis on la relance (idempotence).
//   LOOTY_BACKEND_BIN : binaire du backend (défaut backend/target/debug/backend), lancé depuis backend/ avec DATABASE_URL.
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { B, ok } from '../lib/index.mjs'

const backendDir = fileURLToPath(new URL('../../backend/', import.meta.url))
const bin = process.env.LOOTY_BACKEND_BIN || backendDir + 'target/debug/backend'
const seed = () => JSON.parse(execFileSync(bin, ['seed-demo'], { cwd: backendDir, env: process.env, timeout: 240_000 }).toString())

const session = async (email) => {
  const r = await fetch(`${B}/api/auth/dev-login?email=${encodeURIComponent(email)}&redirect=/`, { redirect: 'manual' })
  const cookie = r.headers.getSetCookie().map((c) => c.split(';')[0]).join('; ')
  return async (path, method = 'GET', body) => {
    const res = await fetch(`${B}${path}`, { method, headers: { cookie, 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
    return res.ok ? res.json().catch(() => null) : { status: res.status }
  }
}

const NAMES = { fr: 'Looty Démo', en: 'Looty Demo' }
// Nombre de tâches dans l'espace perso de la personne connectée.
const personalTasks = async (as) => {
  const { id } = await as('/api/workspaces/personal')
  const boards = await as(`/api/workspaces/${id}/boards`)
  return (await Promise.all(boards.map((b) => as(`/api/boards/${b.id}`)))).flatMap((b) => b.columns.flatMap((c) => c.tasks)).length
}

const first = seed()
const DAY = 86_400_000
for (const lang of ['fr', 'en']) {
  const out = first[lang]
  const name = NAMES[lang]
  ok(`${lang}: command reports the workspace and five characters`, out?.workspace && out.characters?.length === 5, JSON.stringify(out))
  const admin = out.characters.find((c) => c.role === 'admin')
  const viewer = out.characters.find((c) => c.role === 'viewer')
  const members = out.characters.filter((c) => c.role === 'member')
  ok(`${lang}: one admin, three members, one read-only guest`, admin && viewer && members.length === 3)
  const as = await session(admin.email)
  const W = out.workspace

  const list = await as('/api/workspaces')
  ok(`${lang}: workspace named « ${name} »`, (list.workspaces ?? list).some((w) => w.id === W && w.name === name), JSON.stringify(list).slice(0, 300))
  const people = await as(`/api/workspaces/${W}/members`)
  const rows = people.members ?? people
  ok(`${lang}: five members, each with an avatar`, rows.length === 5 && rows.every((m) => m.avatar_url), JSON.stringify(rows.map((m) => [m.name, m.role, m.avatar_url])))
  ok(`${lang}: the guest is read-only`, rows.find((m) => m.email === viewer.email)?.role === 'viewer')

  // Tableau : étiquettes, priorités, sous-tâches, tâche récurrente, groupe relié.
  const boards = await as(`/api/workspaces/${W}/boards`)
  const shared = boards.filter((b) => !b.private_to)
  const board = await as(`/api/boards/${shared[0].id}`)
  const tasks = board.columns.flatMap((c) => c.tasks)
  ok(`${lang}: board has labels`, board.labels.length >= 3)
  ok(`${lang}: tasks carry labels and priorities`, tasks.some((t) => t.labels.length) && new Set(tasks.map((t) => t.priority)).size >= 3)
  ok(`${lang}: a task has subtasks`, tasks.some((t) => t.parent_id))
  ok(`${lang}: a recurring task`, tasks.some((t) => t.repeat))
  ok(`${lang}: a task with a linked group`, tasks.some((t) => t.groups.length))
  ok(`${lang}: task dates are around today`, tasks.filter((t) => t.due_date).every((t) => Math.abs(Date.parse(t.due_date) - Date.now()) < 30 * DAY))

  // Documents riches.
  const pages = await as(`/api/workspaces/${W}/pages`)
  const contents = (await Promise.all(pages.map((p) => as(`/api/workspaces/${W}/pages/${p.id}`)))).map((p) => p.content ?? '').join('\n')
  ok(`${lang}: documents with LaTeX`, /data-type="math-(block|inline)"/.test(contents))
  ok(`${lang}: documents with a diagram`, contents.includes('data-type="mermaid"'))
  ok(`${lang}: documents with a table`, contents.includes('<table'))
  ok(`${lang}: documents with code`, /<pre><code class="language-/.test(contents))

  // Agenda : une semaine glissante autour d'aujourd'hui.
  const from = new Date(Date.now() - 7 * DAY).toISOString(), to = new Date(Date.now() + 14 * DAY).toISOString()
  const events = (await as(`/api/workspaces/${W}/calendars/events?from=${from}&to=${to}`)).events.filter((e) => e.looty)
  const week = events.filter((e) => !e.recurring && Date.parse(e.starts_at) > Date.now() - DAY && Date.parse(e.starts_at) < Date.now() + 7 * DAY)
  const days = new Set(week.map((e) => e.starts_at.slice(0, 10)))
  ok(`${lang}: events spread over the coming week`, days.size >= 5, JSON.stringify([...days]))
  ok(`${lang}: a recurring event`, events.some((e) => e.recurring))

  // Annonce avec sondage, mots-clés, élément personnel.
  const ann = await as(`/api/workspaces/${W}/announcements`)
  ok(`${lang}: an announcement with a poll`, ann.announcements.some((a) => a.poll?.options?.length >= 2))
  const kws = await as(`/api/workspaces/${W}/keywords`)
  ok(`${lang}: keywords`, kws.length >= 2)
  const mine = pages.filter((p) => p.private_to === admin.id)
  ok(`${lang}: the admin has a personal document`, mine.length >= 1)
  const other = await session(members[0].email)
  const theirs = await other(`/api/workspaces/${W}/pages`)
  ok(`${lang}: nobody else sees it`, !theirs.some((p) => mine.some((m) => m.id === p.id)))

  // Projet : notebook (Pyodide) et petit site.
  const projects = await as(`/api/workspaces/${W}/projects`)
  ok(`${lang}: a notebook and a site`, projects.some((p) => p.kind === 'notebook') && projects.some((p) => p.kind === 'site'), JSON.stringify(projects.map((p) => p.kind)))

  // Espace perso : quelques tâches pour chaque personnage.
  for (const c of out.characters) {
    const n = await personalTasks(await session(c.email))
    ok(`${lang}: ${c.name} has personal tasks`, n >= 2, String(n))
  }
}
ok('English and French texts differ', first.fr.board_name !== first.en.board_name)

// Relancer : on remplace l'espace au lieu d'en ajouter un.
const again = seed()
for (const lang of ['fr', 'en']) {
  const admin = again[lang].characters.find((c) => c.role === 'admin')
  const as = await session(admin.email)
  const list = await as('/api/workspaces')
  const named = (list.workspaces ?? list).filter((w) => w.name === NAMES[lang])
  ok(`${lang}: still one demo workspace after a second run`, named.length === 1 && named[0].id === again[lang].workspace, JSON.stringify(named))
  const n = await personalTasks(as)
  const before = first[lang].characters.find((c) => c.role === 'admin')
  ok(`${lang}: personal tasks not duplicated`, n === before.personal_tasks && n === admin.personal_tasks, `${n} / ${before.personal_tasks}`)
}
