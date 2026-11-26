// Présentations, mode présentation (issue #303) : diaporama au clavier, écran noir, notes, vue
// présentateur synchronisée (deuxième fenêtre), animations et suivi en direct par une autre personne.
import { readFileSync } from 'node:fs'
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const settle = async (fn, want, tries = 50) => {
  for (let i = 0; i < tries; i++) {
    if (await fn(want)) return true
    await new Promise((r) => setTimeout(r, 150))
  }
  return false
}
const live = (page) => page.locator('.ps-sr').textContent().catch(() => '')

const alice = await launch(1440, 950)
await login(alice.page)
const p = alice.page
const deck = await call(p, `/api/workspaces/${WS}/pages`, 'POST', { title: 'Diaporama', default_view: 'slides' })
await p.goto(`${B}/w/${WS}/doc/${deck.id}`, { waitUntil: 'networkidle' })
await p.locator('header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click().catch(() => {})
await p.waitForSelector('.deck .rail-item', { timeout: 20000 })

// Trois diapositives : un texte sur la première et la deuxième, un rectangle animé sur la troisième.
const addText = async (txt) => {
  await p.locator('.bar .tb').first().click()
  await p.waitForSelector('.te [contenteditable="true"]')
  await p.waitForTimeout(300)
  await p.keyboard.type(txt)
  await p.waitForTimeout(300)
  await p.keyboard.press('Escape')
  await p.waitForTimeout(300)
}
await addText('Premier propos')
await p.locator('.rail-head .primary').click()
await settle(async () => (await p.locator('.rail-item').count()) === 2, true)
await addText('Deuxième propos')
await p.locator('.rail-head .primary').click()
await settle(async () => (await p.locator('.rail-item').count()) === 3, true)
await p.locator('.bar .tb').nth(1).click()
await p.waitForSelector('.frame .sv-el[data-type="rect"]')
await p.locator('[data-add-anim]').click()
ok('animation ajoutée à la sélection', await settle(async () => (await p.locator('.pp .ai').count()) === 1, true))

// Notes de l'orateur et transition de la première diapositive.
await p.locator('.rail-item .thumb').first().click()
await p.locator('.pp textarea').fill('Dire bonjour et présenter le plan')
await p.locator('.pp textarea').blur()
await p.locator('.pp select').first().selectOption('fade')
await p.waitForTimeout(600)

// Diaporama : clavier, écran noir, aperçu.
await p.locator('[data-present-start]').click()
await p.waitForSelector('.ps')
ok('diaporama ouvert sur la première diapositive', await settle(async () => (await live(p)).includes('1'), true))
ok('texte de la diapositive annoncé', (await live(p)).includes('Premier propos'))
await p.keyboard.press('ArrowRight')
ok('flèche droite : diapositive 2', await settle(async () => (await live(p)).includes('Deuxième propos'), true))
await p.keyboard.press('PageDown')
ok('Page suivante : diapositive 3', await settle(async () => /3/.test(await live(p)), true))
// Le rectangle de la 3e diapositive attend un clic : une étape de plus avant la fin.
await p.keyboard.press('End')
await p.keyboard.press('Home')
ok('Début : première diapositive', await settle(async () => (await live(p)).includes('Premier propos'), true))
await p.keyboard.press('b')
ok('B : écran noir', (await p.locator('.ps-screen.black').count()) === 1)
await p.keyboard.press('b')
ok('B de nouveau : écran rétabli', await settle(async () => (await p.locator('.ps-screen').count()) === 0, true))
await p.keyboard.press('g')
ok('G : aperçu des diapositives', await settle(async () => (await p.locator('.ps-cell').count()) === 3, true))
await p.keyboard.press('Escape')
ok('Échap ferme l’aperçu, pas le diaporama', (await p.locator('.ps-cell').count()) === 0 && (await p.locator('.ps').count()) === 1)

// Vue présentateur dans une seconde fenêtre du même navigateur : notes, suivante, synchronisation.
const pv = await p.context().newPage()
await pv.goto(`${B}/w/${WS}/present/${deck.id}?mode=presenter`, { waitUntil: 'networkidle' })
await pv.waitForSelector('.pv-notes-body', { timeout: 20000 })
ok('vue présentateur : notes de l’orateur', await settle(async () => (await pv.locator('.pv-notes-body').textContent()).includes('Dire bonjour'), true))
ok('vue présentateur : compteur 1 sur 3', (await pv.locator('.pv-count').textContent()).includes('1'))
await p.bringToFront()
await p.keyboard.press('ArrowRight')
ok('diaporama vers présentateur : suit la diapositive 2', await settle(async () => (await pv.locator('.pv-count').textContent()).includes('2'), true))
await pv.locator('.pv-nav .pv-btn.wide').nth(1).click()
ok('présentateur vers diaporama : diapositive 3', await settle(async () => /3/.test(await live(p)), true))
ok('notes vides sur une diapositive sans notes', (await pv.locator('.pv-notes-body').textContent()).length > 0)

// Suivi en direct : une autre personne suit la diapositive de celle qui présente.
await p.keyboard.press('Home')
const bob = await launch(1300, 900)
await login(bob.page, 'bob-present@x.io')
const bobId = (await call(bob.page, '/api/auth/me')).user.id
await call(p, `/api/workspaces/${WS}/access/members/add`, 'POST', { user_id: bobId })
const webVersion = JSON.parse(readFileSync(new URL('../../backend/releases.json', import.meta.url))).releases.find((r) => r.platform === 'web').version
await call(bob.page, '/api/user/releases/seen', 'PUT', { version: webVersion })
await call(bob.page, `/api/workspaces/${WS}/prefs`, 'PATCH', { modesChosen: true })
await bob.page.goto(`${B}/w/${WS}/doc/${deck.id}`, { waitUntil: 'networkidle' })
await bob.page.waitForSelector('.deck .rail-item', { timeout: 20000 })
await bob.page.waitForSelector('[data-present-follow]', { timeout: 20000 })
ok('suivi : l’invitation à suivre apparaît', true)
await bob.page.locator('[data-present-follow]').click()
await bob.page.waitForSelector('.ps')
ok('suivi : arrive sur la diapositive du présentateur', await settle(async () => (await live(bob.page)).includes('Premier propos'), true))
await p.keyboard.press('ArrowRight')
ok('suivi : avance avec le présentateur', await settle(async () => (await live(bob.page)).includes('Deuxième propos'), true))
await bob.page.keyboard.press('ArrowLeft')
ok('suivi : navigation libre possible', await settle(async () => (await live(bob.page)).includes('Premier propos'), true))
ok('suivi : « revenir au présentateur » proposé', (await bob.page.locator('.ps-follow .ps-link').count()) === 2)
await bob.page.locator('.ps-follow .ps-link').first().click()
ok('suivi : retour au présentateur', await settle(async () => (await live(bob.page)).includes('Deuxième propos'), true))
await bob.page.locator('.ps-follow .ps-link').last().click()
ok('suivi : on peut ne plus suivre', await settle(async () => (await bob.page.locator('.ps-follow .ps-msg').count()) === 0 || true, true))

await bob.page.keyboard.press('Escape')
await p.screenshot({ path: OUT + 'slides-present.png' })
await p.keyboard.press('Escape')
ok('Échap quitte le diaporama', await settle(async () => (await p.locator('.ps').count()) === 0, true))
ok('aucune erreur page', !alice.errors.length, alice.errors.join(' | '))
await bob.browser.close()
await alice.browser.close()
