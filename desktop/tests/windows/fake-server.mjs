// Faux serveur Looty pour le test de l'application Windows (smoke.ps1) : il répond aux
// appels que l'application fait d'elle-même et note chaque requête dans un journal, que
// le test lit pour savoir ce que l'application a fait (page chargée, session, lien…).
//   node fake-server.mjs <port> <journal>
import http from 'node:http'
import fs from 'node:fs'

const port = Number(process.argv[2] ?? 18080)
const logFile = process.argv[3] ?? 'fake-server.log'
const SESSION = 'fake-session-7f3a'
const log = (line) => fs.appendFileSync(logFile, line + '\n')

const page = (path) => `<!doctype html><html><head><meta charset="utf-8"><title>Looty test ${path}</title></head>
<body style="font: 16px system-ui; padding: 32px">
<h1>Looty test server</h1><p id="path">${path}</p><p id="state">…</p>
<script>
  // Le JavaScript de la page passe lui aussi par le proxy de l'application.
  fetch('/api/hit?from=' + encodeURIComponent(location.pathname))
    .then(() => fetch('/api/auth/me')).then((r) => r.json())
    .then((me) => me.authenticated ? me : fetch('/api/test-login', { method: 'POST' }).then(() => ({ authenticated: 'new' })))
    .then((me) => { document.getElementById('state').textContent = 'signed in: ' + me.authenticated; fetch('/api/hit?from=signed-in') })
</script></body></html>`

const json = (res, v, headers = {}) => { res.writeHead(200, { 'Content-Type': 'application/json', ...headers }); res.end(JSON.stringify(v)) }

http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x')
  const signedIn = (req.headers.cookie ?? '').split(';').some((c) => c.trim() === `looty_session=${SESSION}`)
  log(`${req.method} ${url.pathname}${url.search} session=${signedIn ? 'yes' : 'no'}`)
  switch (url.pathname) {
    case '/api/health': return json(res, { status: 'ok', desktop_sso: false })
    case '/api/test-login': return json(res, { ok: true }, { 'Set-Cookie': `looty_session=${SESSION}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400` })
    case '/api/auth/me': return json(res, signedIn ? { authenticated: true, user: { id: 'u1', name: 'Windows Test', email: 'win@test' } } : { authenticated: false })
    case '/api/hit': return json(res, { ok: true })
    case '/api/workspaces': return json(res, signedIn ? { workspaces: [{ id: 'w1', name: 'Test space' }] } : { workspaces: [] })
    case '/api/user/notifications': return json(res, { events: [] })
    case '/api/notifications': return json(res, [{ id: 'n1', read_at: null }])
    case '/api/workspaces/w1/my_tasks': return json(res, { tasks: [{ id: 't1', title: 'Overdue test task', board_id: 'b1', board_name: 'Board', due_date: '2020-01-01T10:00:00Z' }] })
    // Version plus récente : l'application affiche une notification Windows (toast).
    case '/api/releases': return json(res, { latest: { desktop: { version: '99.0.0' } } })
  }
  if (url.pathname.startsWith('/api/')) { res.writeHead(404); return res.end() }
  res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' })
  res.end(page(url.pathname))
}).listen(port, '127.0.0.1', () => log(`listening ${port}`))
