// Faux Moodle (services web REST + export de calendrier) pour les tests e2e. Port fixe : 18300 (lib/mock-moodle.mjs).
import http from 'node:http'
import { MOODLE_TOKEN as TOKEN, PRIVATE_TOKEN } from './tokens.mjs'
const now = () => Math.floor(Date.now() / 1000)
const day = 86400
const courses = [
  { id: 1, fullname: 'Algèbre linéaire', displayname: 'Algèbre linéaire', shortname: 'ALG', categoryname: 'L3', progress: 40, isfavourite: true, lastaccess: now() - day, hidden: 0, startdate: now() - 60 * day, enddate: now() + 60 * day, overviewfiles: [{ fileurl: 'http://127.0.0.1:18300/pluginfile.php/9/course/overviewfiles/algebre.png' }] },
  { id: 2, fullname: 'Analyse', displayname: 'Analyse', shortname: 'ANA', categoryname: 'L3', progress: 75, isfavourite: false, lastaccess: now() - 3600, hidden: 0, startdate: now() - 60 * day, enddate: 0 },
  { id: 3, fullname: 'Cours caché', shortname: 'X', hidden: 1, categoryname: 'L3', startdate: 0, enddate: 0 },
  { id: 4, fullname: 'Probabilités', displayname: 'Probabilités', shortname: 'PRO', categoryname: 'L2', progress: 100, isfavourite: false, lastaccess: now() - 200 * day, hidden: 0, startdate: now() - 300 * day, enddate: now() - 100 * day },
  { id: 5, fullname: 'Géométrie', displayname: 'Géométrie', shortname: 'GEO', categoryname: 'L3', progress: 5, isfavourite: false, lastaccess: 0, hidden: 0, startdate: now() + 30 * day, enddate: now() + 120 * day },
  { id: 6, fullname: 'Informatique', displayname: 'Informatique', shortname: 'INF', progress: 20, isfavourite: false, lastaccess: now() - 5 * day, hidden: 0, startdate: now() - 10 * day, enddate: 0 },
]
const state = {
  notifications: [{ id: 10, subject: 'Note publiée : Algèbre', smallmessage: 'Votre note est disponible', contexturl: 'http://127.0.0.1:18300/grade', read: false, timecreated: now() - 3600 }],
  conversations: [{ id: 7, name: '', type: 1, unreadcount: 1, members: [{ id: 5, fullname: 'Alice Martin' }, { id: 9, fullname: 'Prof Durand' }],
    messages: [{ id: 100, useridfrom: 9, text: '<p>Bonjour, le TP est reporté.</p>', timecreated: now() - 7200 }, { id: 99, useridfrom: 5, text: 'Merci', timecreated: now() - 9000 }] }],
}
const ics = () => {
  const d = (t) => new Date(t * 1000).toISOString().replace(/[-:]/g, '').replace(/\.\d+/, '')
  return ['BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//Moodle//NONSGML//EN',
    'BEGIN:VEVENT', 'UID:1@moodle', `DTSTAMP:${d(now())}`, `DTSTART:${d(now() + 2 * day)}`, `DTEND:${d(now() + 2 * day + 3600)}`, 'SUMMARY:Rendu TP Algèbre', 'END:VEVENT',
    'BEGIN:VEVENT', 'UID:2@moodle', `DTSTAMP:${d(now())}`, `DTSTART:${d(now() + 5 * day)}`, `DTEND:${d(now() + 5 * day + 3600)}`, 'SUMMARY:Test Analyse', 'END:VEVENT',
    'END:VCALENDAR'].join('\r\n')
}
function ws(fn, a) {
  switch (fn) {
    case 'core_webservice_get_site_info': return { userid: 5, fullname: 'Alice Martin', sitename: 'Moodle Mock UB', username: 'alice' }
    case 'core_enrol_get_users_courses': return courses
    case 'core_course_set_favourite_courses': { for (const [k, v] of Object.entries(a)) { const m = k.match(/^courses\[(\d+)\]\[id\]$/); if (m) { const c = courses.find((x) => x.id == v); if (c) c.isfavourite = a[`courses[${m[1]}][favourite]`] == 1 } } return { warnings: [] } }
    case 'core_user_set_user_preferences': { const m = String(a['preferences[0][name]']).match(/hidden_course_(\d+)$/); const c = m && courses.find((x) => x.id == m[1]); if (c) c.hidden = a['preferences[0][value]'] ? 1 : 0; return { saved: [], warnings: [] } }
    case 'core_calendar_get_action_events_by_timesort': return { events: [
      { id: 501, name: 'Rendu TP Algèbre est à remettre', activityname: 'Rendu TP Algèbre', modulename: 'assign', timesort: now() + 2 * day, url: 'http://127.0.0.1:18300/mod/assign/view.php?id=1', course: { id: 1, fullname: 'Algèbre linéaire' }, action: { name: 'Remettre un travail', actionable: true } },
      { id: 502, name: 'Test Analyse', activityname: 'Test Analyse', modulename: 'quiz', timesort: now() + 5 * day, url: 'http://127.0.0.1:18300/mod/quiz/view.php?id=2', course: { id: 2, fullname: 'Analyse' }, action: { name: 'Faire le test', actionable: true } },
      { id: 503, name: 'Devoir en retard', activityname: 'Devoir en retard', modulename: 'assign', timesort: now() - 2 * day, url: 'http://127.0.0.1:18300/x', course: { id: 1, fullname: 'Algèbre linéaire' }, action: { name: 'Remettre', actionable: true } }] }
    case 'message_popup_get_popup_notifications': return { notifications: [...state.notifications].sort((x, y) => y.id - x.id), unreadcount: state.notifications.filter((n) => !n.read).length }
    case 'core_message_get_conversations': return { conversations: state.conversations }
    case 'core_message_get_conversation_messages': {
      const c = state.conversations.find((x) => x.id == a.convid)
      return c ? { id: c.id, members: c.members, messages: c.messages } : { exception: 'x', errorcode: 'invalidrecord', message: 'Conversation introuvable' }
    }
    case 'core_message_mark_all_conversation_messages_as_read': { const c = state.conversations.find((x) => x.id == a.conversationid); if (c) c.unreadcount = 0; return null }
    case 'core_message_send_messages_to_conversation': {
      const c = state.conversations.find((x) => x.id == a.conversationid)
      const m = { id: Math.max(...c.messages.map((x) => x.id)) + 1, useridfrom: 5, text: a['messages[0][text]'], timecreated: now() }
      c.messages.unshift(m)
      return [m]
    }
    case 'core_calendar_get_calendar_export_token': return { token: 'exp123', warnings: [] }
    case 'tool_mobile_get_autologin_key': return a.privatetoken === PRIVATE_TOKEN
      ? { key: 'autokey' + now(), autologinurl: 'http://127.0.0.1:18300/admin/tool/mobile/autologin.php', warnings: [] }
      : { exception: 'x', errorcode: 'invalidprivatetoken', message: 'Invalid private token' }
    case 'core_course_get_contents': return a.courseid != 1 ? [] : [
      { id: 11, name: 'Généralités', section: 0, uservisible: true, summary: '<p>Bienvenue <img src="http://127.0.0.1:18300/webservice/pluginfile.php/9/course/section/11/logo.png"><script>alert(1)</script></p>', modules: [
        { id: 101, name: 'Annonces', modname: 'forum', modplural: 'Forums', url: 'http://127.0.0.1:18300/mod/forum/view.php?id=101', visible: 1, uservisible: true },
        { id: 102, name: 'Plan du cours', modname: 'resource', modplural: 'Fichiers', url: 'http://127.0.0.1:18300/mod/resource/view.php?id=102', visible: 1, uservisible: true,
          contents: [{ type: 'file', filename: 'plan.pdf', filepath: '/', filesize: 400, mimetype: 'application/pdf', timemodified: now() - day, fileurl: 'http://127.0.0.1:18300/webservice/pluginfile.php/20/mod_resource/content/1/plan.pdf?forcedownload=1' }] },
        { id: 103, name: 'Consignes', modname: 'label', modplural: 'Étiquettes', url: '', visible: 1, uservisible: true, description: '<p><b>Important</b> : lire le plan.</p>' },
      ] },
      { id: 12, name: 'Chapitre 1 : Vecteurs', section: 1, uservisible: true, summary: '', modules: [
        { id: 104, name: 'Cours vecteurs', modname: 'page', modplural: 'Pages', url: 'http://127.0.0.1:18300/mod/page/view.php?id=104', visible: 1, uservisible: true,
          contents: [{ type: 'file', filename: 'index.html', filepath: '/', filesize: 100, mimetype: 'text/html', timemodified: now(), fileurl: 'http://127.0.0.1:18300/webservice/pluginfile.php/21/mod_page/content/3/index.html?forcedownload=1' },
            { type: 'file', filename: 'schema.png', filepath: '/', filesize: 70, mimetype: 'image/png', timemodified: now(), fileurl: 'http://127.0.0.1:18300/webservice/pluginfile.php/21/mod_page/content/3/schema.png?forcedownload=1' }] },
        { id: 105, name: 'Rendu TP Algèbre', modname: 'assign', modplural: 'Devoirs', instance: 55, url: 'http://127.0.0.1:18300/mod/assign/view.php?id=105', visible: 1, uservisible: true, dates: [{ label: 'À rendre :', timestamp: now() + 2 * day }] },
        { id: 106, name: 'Wikipédia', modname: 'url', modplural: 'URL', url: 'http://127.0.0.1:18300/mod/url/view.php?id=106', visible: 1, uservisible: true, contents: [{ type: 'url', filename: 'Wikipédia', fileurl: 'https://fr.wikipedia.org/wiki/Vecteur' }] },
        { id: 107, name: 'Quiz 1', modname: 'quiz', modplural: 'Tests', url: 'http://127.0.0.1:18300/mod/quiz/view.php?id=107', visible: 1, uservisible: true },
        { id: 108, name: 'Caché', modname: 'page', url: 'http://127.0.0.1:18300/x', visible: 0, uservisible: false },
      ] },
    ]
    case 'mod_assign_get_assignments': return { courses: [{ id: 1, assignments: [{ id: 55, cmid: 105, name: 'Rendu TP Algèbre', intro: '<p>Rendre le <a href="http://127.0.0.1:18300/pluginfile.php/22/mod_assign/intro/sujet.pdf">sujet</a> complété.</p>', duedate: now() + 2 * day, cutoffdate: 0, allowsubmissionsfromdate: now() - 5 * day,
      introattachments: [{ filename: 'sujet.pdf', filesize: 400, mimetype: 'application/pdf', fileurl: 'http://127.0.0.1:18300/webservice/pluginfile.php/22/mod_assign/introattachment/0/sujet.pdf' }] }] }] }
    case 'mod_assign_get_submission_status': return { lastattempt: { submission: { status: 'draft' }, gradingstatus: 'notgraded' }, feedback: null }
    default: return { exception: 'x', errorcode: 'invalidfunction', message: fn }
  }
}
const body = (req) => new Promise((r) => { let b = ''; req.on('data', (d) => (b += d)); req.on('end', () => r(b)) })
// Panne simulée (tests du mode hors ligne) : /__mock/down?on=1 puis tout répond 503.
let down = false
export const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, 'http://x')
  if (url.pathname === '/__mock/down') { down = url.searchParams.get('on') === '1'; res.writeHead(204); return res.end() }
  if (down) { res.writeHead(503, { 'Content-Type': 'text/html' }); return res.end('<h1>Maintenance</h1>') }
  const send = (v, type = 'application/json') => { res.writeHead(200, { 'Content-Type': type }); res.end(typeof v === 'string' ? v : JSON.stringify(v)) }
  const raw = await body(req)
  const form = Object.fromEntries(new URLSearchParams(raw))
  if (url.pathname === '/lib/ajax/service-nologin.php') return send([{ error: false, data: { sitename: 'Moodle Mock UB', typeoflogin: 2, logourl: '' } }])
  if (url.pathname === '/login/token.php') return send(form.username === 'alice' && form.password === 'secret' ? { token: TOKEN, privatetoken: PRIVATE_TOKEN } : { error: 'Invalid login', errorcode: 'invalidlogin' })
  if (url.pathname === '/webservice/rest/server.php') {
    if (form.wstoken !== TOKEN) return send({ exception: 'moodle_exception', errorcode: 'invalidtoken', message: 'Invalid token' })
    return send(ws(form.wsfunction, form))
  }
  if (url.pathname === '/calendar/export_execute.php') return url.searchParams.get('authtoken') === 'exp123' ? send(ics(), 'text/calendar') : (res.writeHead(403), res.end())
  // Portail officiel : page de connexion (CAS simulé), puis redirection vers le schéma demandé.
  if (url.pathname === '/admin/tool/mobile/launch.php') {
    const scheme = url.searchParams.get('urlscheme') || 'moodlemobile'
    const forced = process.env.FORCED_SCHEME || ''
    if ((req.method === 'POST' && form.password === 'secret') || url.searchParams.get('auto') === '1') {
      const key = Buffer.from(`sig:::${TOKEN}:::${PRIVATE_TOKEN}`).toString('base64')
      res.writeHead(303, { Location: `${forced || scheme}://token=${key}` }); return res.end()
    }
    return send(`<!doctype html><html><body><h1>CAS Université (mock)</h1><form method="post"><input name="username" value="alice"><input name="password" type="password" id="pw"><button id="go">Connexion</button></form></body></html>`, 'text/html')
  }
  if (url.pathname === '/admin/tool/mobile/autologin.php') return send(`<!doctype html><title>autologin</title><p id="auto">logged in as ${url.searchParams.get('userid')} → ${url.searchParams.get('urltogo')}</p>`, 'text/html')
  if (url.pathname.startsWith('/mod/') || url.pathname.startsWith('/course/')) return send('<!doctype html><title>Moodle page</title><p>' + url.pathname + '</p>', 'text/html')
  if (url.pathname === '/my/' || url.pathname === '/') return send('<!doctype html><title>Moodle Mock</title><h1>Tableau de bord</h1>', 'text/html')
  if (url.pathname === '/user/managetoken.php') return send(`<!doctype html><title>Clés de sécurité</title><table><tr><th>Clé</th><th>Service</th></tr><tr><td id="key">${TOKEN}</td><td>Moodle mobile web service</td></tr></table>`, 'text/html')
  if (url.pathname.startsWith('/webservice/pluginfile.php/')) {
    if (url.searchParams.get('token') !== TOKEN) { res.writeHead(403); return res.end('forbidden') }
    const name = url.pathname.split('/').pop()
    if (name.endsWith('.pdf')) return send('%PDF-1.1\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj 2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj 3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 144]>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF', 'application/pdf')
    if (name.endsWith('.png')) { res.writeHead(200, { 'Content-Type': 'image/png' }); return res.end(Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64')) }
    if (name === 'index.html') return send('<html><body><h2>Les vecteurs</h2><p>Un vecteur a une direction.</p><img src="schema.png" alt="schéma"><a href="http://127.0.0.1:18300/mod/quiz/view.php?id=107">Faire le quiz</a><script>alert(1)</script></body></html>', 'text/html')
    res.writeHead(404); return res.end()
  }
  if (url.pathname === '/mock/new') {
    state.notifications.push({ id: 11 + state.notifications.length, subject: 'Nouveau devoir : Projet', smallmessage: 'À rendre vendredi', contexturl: '', read: false, timecreated: now() })
    const c = state.conversations[0]; c.messages.unshift({ id: Math.max(...c.messages.map((x) => x.id)) + 1, useridfrom: 9, text: 'Pensez au projet', timecreated: now() }); c.unreadcount++
    return send({ ok: true })
  }
  res.writeHead(404); res.end()
})

// Lancé seul : `node lib/mock-moodle.mjs`.
if (import.meta.url === `file://${process.argv[1]}`) server.listen(18300, '127.0.0.1', () => console.log('mock moodle on 18300'))
