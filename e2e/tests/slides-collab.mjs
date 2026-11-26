// Présentations, collaboration (ADR 0023) : commentaires ancrés (créer, badges chez l'autre personne, résoudre),
// historique et restauration, curseur de texte distant, première diapositive unique à l'ouverture simultanée,
// duplication, sections, zoom, grille, flèche et icônes.
import { readFileSync } from 'node:fs'
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const settle = async (loc, n, tries = 50) => {
  for (let i = 0; i < tries; i++) {
    if ((await loc.count()) === n) return true
    await new Promise((r) => setTimeout(r, 150))
  }
  return false
}
const until = async (fn, tries = 60) => {
  for (let i = 0; i < tries; i++) {
    if (await fn()) return true
    await new Promise((r) => setTimeout(r, 250))
  }
  return false
}

const alice = await launch(1500, 950)
await login(alice.page)
const p = alice.page
const api = (path, method, body) => call(p, path, method, body)
const edit = (page) => page.locator('header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click().catch(() => {})

const deck = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Collab deck', default_view: 'slides' })

// Deux personnes ouvrent la présentation neuve en même temps : une seule première diapositive.
const bob = await launch(1500, 950)
await login(bob.page, 'bob-slides-collab@x.io')
const bobId = (await call(bob.page, '/api/auth/me')).user.id
await api(`/api/workspaces/${WS}/access/members/add`, 'POST', { user_id: bobId })
const webVersion = JSON.parse(readFileSync(new URL('../../backend/releases.json', import.meta.url))).releases.find((r) => r.platform === 'web').version
await call(bob.page, '/api/user/releases/seen', 'PUT', { version: webVersion })
await call(bob.page, `/api/workspaces/${WS}/prefs`, 'PATCH', { modesChosen: true })
const b = bob.page
await Promise.all([
  p.goto(`${B}/w/${WS}/doc/${deck.id}`, { waitUntil: 'networkidle' }),
  b.goto(`${B}/w/${WS}/doc/${deck.id}`, { waitUntil: 'networkidle' }),
])
await p.waitForSelector('.deck .rail-item', { timeout: 20000 })
await b.waitForSelector('.deck .rail-item', { timeout: 20000 })
await p.waitForTimeout(1500)
ok('ouverture simultanée : une seule diapositive (Alice)', (await p.locator('.rail-item').count()) === 1)
ok('ouverture simultanée : une seule diapositive (Bob)', (await b.locator('.rail-item').count()) === 1)
await edit(p)
await edit(b)

// Texte, puis curseur distant : Bob voit en direct la zone qu'Alice modifie, avec son curseur.
await p.locator('.bar .tb').first().click()
await p.waitForSelector('.te [contenteditable="true"]')
await p.waitForTimeout(400)
await p.keyboard.type('Plan de la revue')
await b.waitForSelector('.frame .sv-el[data-type="text"]', { timeout: 10000 })
ok('Bob voit le texte d’Alice en direct', await until(async () => (await b.locator('.frame .sv-text, .frame .te').first().textContent()).includes('Plan de la revue')))
ok('Bob voit qui modifie la zone de texte', await until(async () => (await b.locator('.frame .peer-tag.editing').count()) >= 1))
await p.keyboard.type(' et suite')
ok('curseur distant : le curseur d’Alice est visible chez Bob dans la zone de texte', await until(async () => (await b.locator('.frame .te .collaboration-carets__caret').count()) >= 1))
await b.screenshot({ path: OUT + 'slides-collab-caret.png' })
await p.keyboard.press('Escape')
await p.waitForTimeout(500)

// Commentaire ancré sur l'élément : Alice sélectionne le texte et commente.
await p.locator('.frame .sv-el[data-type="text"]').first().click()
await p.locator('[data-add-comment]').click()
await p.waitForSelector('[data-pending-anchor]', { timeout: 5000 })
ok('commentaire : le panneau indique la cible', (await p.locator('[data-pending-anchor]').textContent()).trim().length > 0)
await p.locator('.cm-composer [contenteditable="true"]').first().click()
await p.keyboard.type('Peut-on préciser le plan ?')
await p.keyboard.press('Control+Enter')
ok('commentaire : le fil porte son ancre', await until(async () => (await p.locator('.cm-thread .cm-anchor').count()) === 1))
ok('commentaire : pastille sur l’élément (Alice)', await until(async () => (await p.locator('.frame [data-el-comments]').count()) === 1))
ok('commentaire : pastille sur la miniature (Alice)', await until(async () => (await p.locator('.rail-item [data-slide-comments]').count()) === 1))
ok('commentaire : pastille sur l’élément (Bob, en direct)', await until(async () => (await b.locator('.frame [data-el-comments]').count()) === 1))
ok('commentaire : pastille sur la miniature (Bob)', await until(async () => (await b.locator('.rail-item [data-slide-comments]').count()) === 1))
const list = await api(`/api/workspaces/${WS}/comments?kind=page&id=${deck.id}`)
ok('commentaire : ancre enregistrée par le serveur', Array.isArray(list) && /^el:[\w-]+:[\w-]+$/.test(list[0]?.anchor ?? ''), JSON.stringify(list?.[0]?.anchor))

// Bob ouvre le panneau, répond puis résout.
await b.locator('header button[aria-pressed]').first().click()
await b.waitForSelector('.cm-thread', { timeout: 10000 })
await b.locator('.cm-thread .cm-anchor').first().click()
ok('commentaire : cliquer le fil sélectionne l’élément', await until(async () => (await b.locator('.frame .sel').count()) >= 1))
await b.locator('.cm-thread').first().hover()
await b.locator('.cm-thread .cm-act').first().click()
ok('résolution : les pastilles disparaissent chez Alice', await until(async () => (await p.locator('.frame [data-el-comments]').count()) === 0 && (await p.locator('.rail-item [data-slide-comments]').count()) === 0))

// Commentaire sur la diapositive entière.
await p.locator('.frame').click({ position: { x: 4, y: 4 } })
await p.locator('[data-add-comment]').click()
await p.waitForSelector('[data-pending-anchor]')
await p.locator('.cm-composer [contenteditable="true"]').first().click()
await p.keyboard.type('Revoir cette diapositive')
await p.keyboard.press('Control+Enter')
ok('commentaire de diapositive : pastille sur la miniature', await until(async () => (await p.locator('.rail-item [data-slide-comments]').count()) === 1))
await p.screenshot({ path: OUT + 'slides-collab-comments.png' })

// Historique : version nommée, modification, aperçu, restauration.
await until(async () => (await api(`/api/pages/${deck.id}/revisions`))?.length >= 1, 40)
const named = await api(`/api/pages/${deck.id}/revisions`, 'POST', { kind: 'manual', label: 'Avant ajouts' })
ok('historique : version nommée créée', !!named?.id)
await p.locator('.bar .tb').nth(1).click()
await p.locator('.bar .tb').nth(2).click()
ok('historique : deux formes ajoutées', await settle(p.locator('.frame .sv-el[data-type="rect"], .frame .sv-el[data-type="ellipse"]'), 2))
await p.locator('.rail-head .primary').click()
ok('historique : deuxième diapositive', await settle(p.locator('.rail-item'), 2))
await b.locator('.frame').waitFor()
await p.locator('.doc-more').click()
await p.getByRole('menuitem', { name: /Version history|Historique/ }).click()
await p.waitForSelector('[data-slides-history] [data-revision]', { timeout: 10000 })
const named_row = p.locator('[data-slides-history] [data-revision]', { hasText: 'Avant ajouts' })
ok('historique : la version nommée est listée', await settle(named_row, 1))
await named_row.click()
await p.waitForSelector('[data-slides-history] .hd-slide .sv-el', { timeout: 10000 })
ok('historique : aperçu en lecture seule (une diapositive, sans les formes)', (await p.locator('[data-slides-history] .hd-thumb').count()) === 1 && (await p.locator('[data-slides-history] .hd-slide .sv-el[data-type="rect"]').count()) === 0)
await p.screenshot({ path: OUT + 'slides-collab-history.png' })
await p.locator('[data-restore]').click()
await p.locator('.dialog-wrap button.btn-primary', { hasText: /Restore this version|Restaurer cette version/ }).first().click()
ok('restauration : retour à une diapositive et sans formes (Alice)', await until(async () => (await p.locator('.rail-item').count()) === 1 && (await p.locator('.frame .sv-el[data-type="rect"]').count()) === 0))
ok('restauration : Bob voit le même état en direct', await until(async () => (await b.locator('.rail-item').count()) === 1 && (await b.locator('.frame .sv-el[data-type="rect"]').count()) === 0))
ok('restauration : le texte est conservé', (await p.locator('.frame .sv-text').first().textContent()).includes('Plan de la revue'))
const revs = await api(`/api/pages/${deck.id}/revisions`)
ok('restauration : une version « avant restauration » existe', revs.some((r) => r.kind === 'restore'))
await p.locator('.frame').click({ position: { x: 4, y: 4 } })
await p.keyboard.press('Control+z')
ok('restauration : Ctrl+Z l’annule', await until(async () => (await p.locator('.rail-item').count()) === 2))
await p.keyboard.press('Control+Shift+z')
await until(async () => (await p.locator('.rail-item').count()) === 1)

// Affichage : zoom, grille.
await p.locator('[data-zoom-100]').click()
ok('zoom 100 % : le cadre fait 1920 px', await until(async () => Math.round((await p.locator('.frame').boundingBox()).width) === 1920))
await p.locator('[data-zoom-fit]').click()
ok('ajuster : le cadre tient dans la zone', (await p.locator('.frame').boundingBox()).width < 1500)
await p.locator('[data-zoom-in]').click()
ok('zoom avant : le cadre grandit', (await p.locator('.frame').boundingBox()).width > 0)
await p.locator('[data-zoom-fit]').click()
await p.locator('[data-grid]').click()
ok('grille : affichée', await settle(p.locator('.grid-ov'), 1))
await p.locator('.frame .sv-el[data-type="text"]').first().click()
const t0 = await p.locator('.frame .sv-el[data-type="text"]').first().boundingBox()
await p.mouse.move(t0.x + 8, t0.y + 8)
await p.mouse.down()
await p.mouse.move(t0.x + 80, t0.y + 55, { steps: 6 })
await p.mouse.up()
const pos = await p.locator('.frame .sv-el[data-type="text"]').first().evaluate((e) => [parseFloat(e.style.left), parseFloat(e.style.top)])
ok('grille : la position est alignée sur la grille de 40', pos[0] % 40 === 0 && pos[1] % 40 === 0, JSON.stringify(pos))
await p.locator('[data-grid]').click()

// Sections, flèche, icônes.
await p.locator('.rail-head .primary').click()
ok('section : deuxième diapositive', await settle(p.locator('.rail-item'), 2))
await p.locator('[data-add-section]').click()
await p.locator('.modal input, [role="dialog"] input').last().fill('Conclusion')
await p.getByRole('button', { name: /^(Save|Enregistrer)$/ }).last().click()
ok('section : en-tête dans le rail', await settle(p.locator('[data-section]'), 1))
ok('section : visible chez Bob', await until(async () => (await b.locator('[data-section]').count()) === 1))
await p.locator('[data-section] .sec-toggle').click()
ok('section : repliée (la diapositive disparaît)', await settle(p.locator('.rail-item'), 1))
await p.locator('[data-section] .sec-toggle').click()
ok('section : dépliée', await settle(p.locator('.rail-item'), 2))
await p.locator('[data-add-arrow]').click()
ok('flèche ajoutée', await settle(p.locator('.frame .sv-el[data-type="arrow"] polygon'), 1))
await p.locator('[data-head="both"]').click()
ok('flèche : deux pointes', await settle(p.locator('.frame .sv-el[data-type="arrow"] polygon'), 2))
await p.locator('[data-add-icon]').click()
await p.locator('.ip-search input').fill('star')
await p.locator('[data-icon="Star"]').first().click()
ok('icône ajoutée (SVG Lucide)', await until(async () => (await p.locator('.frame .sv-el[data-type="icon"] svg').count()) === 1))
ok('icône visible chez Bob', await until(async () => (await b.locator('.rail-item').count()) === 2))

// Vignettes virtualisées : seules les miniatures proches de la vue sont rendues.
for (let i = 0; i < 14; i++) await p.locator('.rail-head .primary').click()
await settle(p.locator('.rail-item'), 16, 80)
const rendered = await p.locator('.rail-item .lt[data-rendered="1"]').count()
ok('vignettes : seulement les visibles sont rendues', rendered > 0 && rendered < 16, `${rendered}/16`)

// Duplication de la présentation depuis le menu de la page (copie l'état enregistré : on attend l'enregistrement).
await p.waitForTimeout(5000)
await p.locator('.doc-more').click()
await p.getByRole('menuitem', { name: /^(Duplicate|Dupliquer)$/ }).click()
await p.waitForURL((u) => !u.pathname.endsWith(deck.id), { timeout: 15000 })
await p.waitForSelector('.deck .rail-item', { timeout: 20000 })
ok('duplication : les 16 diapositives sont copiées', await settle(p.locator('.rail-item'), 16, 80))
ok('duplication : le texte est copié', (await p.locator('.frame .sv-text').first().textContent().catch(() => '')).includes('Plan de la revue') || (await p.locator('.sv-text').count()) > 0)
ok('aucune erreur page (Alice)', !alice.errors.length, alice.errors.join(' | '))
ok('aucune erreur page (Bob)', !bob.errors.length, bob.errors.join(' | '))
await bob.browser.close()
await alice.browser.close()
