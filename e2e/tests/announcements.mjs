// Annonces d'espace et sondages : publier avec un sondage, voter, changer son vote, clore ; widget d'accueil.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch(1300, 950)
await login(p)
const api = (path, method, body) => call(p, path, method, body)

await p.goto(`${B}/w/${WS}/announcements`, { waitUntil: 'networkidle' }); await p.waitForTimeout(800)
ok('sidebar has the announcements link', await p.locator('a[href$="/announcements"]').count() >= 1)
await p.locator('button', { hasText: /Nouvelle annonce|New announcement/ }).first().click(); await p.waitForTimeout(400)
const dlg = p.locator('[role=dialog]')
await dlg.locator('input').first().fill('Sortie de promo')
await dlg.locator('.ProseMirror').click(); await p.keyboard.type('Rendez-vous devant la fac.')
await dlg.locator('label.af-row', { hasText: /Ajouter un sondage|Add a poll/ }).locator('button, [role=switch]').first().click(); await p.waitForTimeout(200)
await dlg.locator('.af-poll input').first().fill('Quel soir ?')
const opts = dlg.locator('.af-opt input')
await opts.nth(0).fill('Jeudi'); await opts.nth(1).fill('Vendredi')
await dlg.locator('button', { hasText: /Ajouter une option|Add an option/ }).click()
await dlg.locator('.af-opt input').nth(2).fill('Samedi')
await dlg.locator('button', { hasText: /^\s*(Publier|Publish)\s*$/ }).click(); await p.waitForTimeout(1200)

const list = await api(`/api/workspaces/${WS}/announcements`)
const a = list.announcements.find((x) => x.title === 'Sortie de promo')
ok('announcement published with its poll', a && a.poll?.options.length === 3 && /Rendez-vous/.test(a.body), JSON.stringify(a?.poll ?? null).slice(0, 160))
const card = p.locator('article.an', { hasText: 'Sortie de promo' })
ok('poll shown in the announcement', await card.locator('.poll-opt').count() === 3)

await card.locator('.poll-opt', { hasText: 'Vendredi' }).click()
await card.locator('button', { hasText: /^\s*(Voter|Vote)\s*$/ }).click(); await p.waitForTimeout(1000)
ok('vote counted', /1 · 100 %/.test(await card.locator('.poll-opt', { hasText: 'Vendredi' }).innerText()))
await card.locator('.poll-opt', { hasText: 'Jeudi' }).click()
await card.locator('button', { hasText: /Changer mon vote|Change my vote/ }).click(); await p.waitForTimeout(1000)
let poll = (await api(`/api/workspaces/${WS}/announcements`)).announcements.find((x) => x.id === a.id).poll
ok('vote changed', poll.my_votes.length === 1 && poll.options.find((o) => o.label === 'Jeudi').votes === 1 && poll.voter_count === 1)
await p.screenshot({ path: OUT + 'announcements.png' })

await card.locator('button', { hasText: /Clore le sondage|Close the poll/ }).click(); await p.waitForTimeout(1000)
poll = (await api(`/api/workspaces/${WS}/announcements`)).announcements.find((x) => x.id === a.id).poll
ok('poll closed', poll.closed && await card.locator('.poll-opt:disabled').count() === 3)

// Accueil : le widget montre l'annonce.
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { dashboard: null })
await p.goto(`${B}/w/${WS}`, { waitUntil: 'networkidle' }); await p.waitForTimeout(1200)
ok('home widget lists the announcement', /Sortie de promo/.test(await p.locator('.gp', { hasText: /Annonces|Announcements/ }).first().innerText()))

await api(`/api/workspaces/${WS}/announcements/${a.id}`, 'DELETE')
ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
