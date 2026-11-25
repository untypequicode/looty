// Présentations (ADR 0023) : créer une présentation, ajouter du texte et des formes, déplacer,
// annuler, voir la présence et les changements d'une autre personne en direct.
import { readFileSync } from 'node:fs'
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const alice = await launch(1440, 950)
await login(alice.page)
const p = alice.page
const api = (path, method, body) => call(p, path, method, body)

const deck = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Revue trimestrielle', default_view: 'slides' })
ok('page créée comme présentation', deck.default_view === 'slides')

await p.goto(`${B}/w/${WS}/doc/${deck.id}`, { waitUntil: 'networkidle' })
await p.locator('header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click().catch(() => {})
await p.waitForSelector('.deck .frame', { timeout: 20000 })
await p.waitForSelector('.deck .rail-item', { timeout: 20000 })
ok('une diapositive au départ', (await p.locator('.rail-item').count()) === 1)

// Texte : ajout, saisie, sortie de l'édition.
await p.locator('.bar .tb').first().click()
await p.waitForSelector('.te [contenteditable="true"]')
await p.waitForTimeout(300)
await p.keyboard.type('Bonjour les équipes')
await p.waitForTimeout(300)
await p.keyboard.press('Escape')
await p.waitForTimeout(500)
ok('zone de texte rendue', (await p.locator('.frame .sv-text').first().textContent()).includes('Bonjour les équipes'))

// Rectangle : ajout puis déplacement à la souris.
await p.locator('.bar .tb').nth(1).click()
const rect = p.locator('.frame .sv-el[data-type="rect"]')
ok('rectangle ajouté', (await rect.count()) === 1)
const left0 = await rect.evaluate((e) => parseFloat(e.style.left))
const b = await rect.boundingBox()
await p.mouse.move(b.x + b.width / 2, b.y + b.height / 2)
await p.mouse.down()
await p.mouse.move(b.x + b.width / 2 + 120, b.y + b.height / 2 + 40, { steps: 8 })
await p.mouse.up()
await p.waitForTimeout(300)
const left1 = await rect.evaluate((e) => parseFloat(e.style.left))
ok('rectangle déplacé', left1 > left0 + 100, `${left0} -> ${left1}`)

// Propriétés : couleur de remplissage.
await p.locator('.ins .cf', { hasText: /Fill|Remplissage/ }).locator('.sw').nth(3).click()
ok('remplissage modifié', (await rect.locator('rect').getAttribute('fill')) === 'var(--pal-orange)' || (await rect.locator('rect').getAttribute('fill')).startsWith('var(--pal-'))

// Annuler / rétablir (clavier).
await p.locator('.frame').click({ position: { x: 5, y: 5 } })
await p.keyboard.press('Control+z')
await p.waitForTimeout(300)
ok('annulation : fond de la couleur rétabli', (await rect.locator('rect').getAttribute('fill')) !== 'var(--pal-orange)')

// Diapositives : ajout, duplication, réordonnancement par les boutons.
await p.locator('.rail-head .primary').click()
ok('deux diapositives', (await p.locator('.rail-item').count()) === 2)
await p.locator('.rail-head .rb[title]').first().click()
await p.waitForTimeout(500)
ok('diapositive dupliquée', (await p.locator('.rail-item').count()) === 3)
await p.locator('.rail-item .thumb').first().click()
await p.locator('.bar .tb').nth(2).click()
ok('ellipse sur la première', (await p.locator('.frame .sv-el[data-type="ellipse"]').count()) === 1)

// Une autre personne voit les mêmes diapositives et ses changements arrivent en direct.
const bob = await launch(1300, 900)
await login(bob.page, 'bob-slides@x.io')
const bobId = (await call(bob.page, '/api/auth/me')).user.id
await api(`/api/workspaces/${WS}/access/members/add`, 'POST', { user_id: bobId })
const webVersion = JSON.parse(readFileSync(new URL('../../backend/releases.json', import.meta.url))).releases.find((r) => r.platform === 'web').version
await call(bob.page, '/api/user/releases/seen', 'PUT', { version: webVersion })
await call(bob.page, `/api/workspaces/${WS}/prefs`, 'PATCH', { modesChosen: true })
await bob.page.goto(`${B}/w/${WS}/doc/${deck.id}`, { waitUntil: 'networkidle' })
await bob.page.waitForSelector('.deck .rail-item', { timeout: 20000 })
ok('autre personne : trois diapositives', (await bob.page.locator('.rail-item').count()) === 3)
ok('autre personne : voit l’ellipse', (await bob.page.locator('.frame .sv-el[data-type="ellipse"]').count()) === 1)
ok('autre personne : voit le texte', (await bob.page.locator('.frame .sv-text').first().textContent()).includes('Bonjour les équipes'))

await bob.page.locator('header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click().catch(() => {})
await bob.page.locator('.rail-item .thumb').first().click()
await bob.page.locator('.bar .tb').nth(3).click()
await p.waitForSelector('.frame .sv-el[data-type="line"]', { timeout: 10000 })
ok('mes diapositives : la ligne de l’autre personne apparaît', (await p.locator('.frame .sv-el[data-type="line"]').count()) === 1)
ok('présence : sa sélection est visible chez moi', (await p.locator('.frame .peer-box').count()) >= 1)
await p.screenshot({ path: OUT + 'slides.png' })

// Enregistrement : le plan textuel est indexé (recherche).
let hit = false
for (let i = 0; i < 15 && !hit; i++) {
  const r = await api(`/api/workspaces/${WS}/search?q=${encodeURIComponent('équipes')}`)
  hit = JSON.stringify(r).includes(deck.id)
  if (!hit) await p.waitForTimeout(1000)
}
ok('recherche : le texte des diapositives est trouvé', hit)
ok('aucune erreur page', !alice.errors.length, alice.errors.join(' | '))
await bob.browser.close()
await alice.browser.close()
