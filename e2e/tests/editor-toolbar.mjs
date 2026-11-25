// Barre d'outils fixe de l'éditeur de pages : gras, titre, lien, tableau, annuler/rétablir,
// absence en lecture, masquage mémorisé, menu « Plus » sur écran étroit.
import { B, WS, OUT, ok, launch, login, api } from '../lib/index.mjs'

const { browser, page: p, errors } = await launch(1440, 900)
await login(p)
await api(p, `/api/workspaces/${WS}/prefs`, 'PATCH', { modes: { docs: 'read', boards: 'write', calendar: 'write' }, modesChosen: true })
await api(p, '/api/user/prefs', 'PATCH', { editorToolbar: null })
const pg = await api(p, `/api/workspaces/${WS}/pages`, 'POST', { title: 'Barre', content: '<p>Bonjour monde</p>' })

const tb = p.locator('[data-doc-toolbar]')
const btn = (id) => tb.locator(`[data-tb="${id}"]`)
const editor = p.locator('.ProseMirror').first()

await p.goto(`${B}/w/${WS}/doc/${pg.id}`, { waitUntil: 'networkidle' })
await editor.waitFor({ timeout: 15000 })
ok('lecture : pas de barre d’outils', (await tb.count()) === 0)

await p.locator('header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click()
await tb.waitFor({ timeout: 10000 })
ok('édition : barre visible, role toolbar', (await tb.getAttribute('role')) === 'toolbar' && !!(await tb.getAttribute('aria-label')))
await p.screenshot({ path: OUT + 'editor-toolbar-light.png' })

// Gras sur un mot sélectionné.
await editor.locator('p').first().click()
await p.keyboard.press('ControlOrMeta+A')
await btn('bold').click()
ok('gras : appliqué', (await editor.locator('strong').count()) > 0)
await p.waitForFunction(() => document.querySelector('[data-tb="bold"]')?.getAttribute('aria-pressed') === 'true', null, { timeout: 5000 }).catch(() => {})
ok('gras : bouton actif', (await btn('bold').getAttribute('aria-pressed')) === 'true')
await btn('bold').click()
ok('gras : retiré', (await editor.locator('strong').count()) === 0)

// Titre par le menu de style de bloc.
await btn('block').click()
await p.getByRole('menuitem', { name: /^(Heading 1|Titre 1)$/ }).click()
ok('titre 1', (await editor.locator('h1').count()) === 1)

// Lien.
await editor.locator('h1').click()
await p.keyboard.press('ControlOrMeta+A')
await btn('link').click()
await p.getByRole('textbox', { name: /^(Link|Lien)$/ }).fill('example.org')
await p.keyboard.press('Enter')
ok('lien', (await editor.locator('a[href="https://example.org"]').count()) === 1)

// Tableau + action contextuelle.
await editor.click({ position: { x: 20, y: 300 } })
await btn('table').click()
await p.waitForTimeout(800) // pas de fusion avec l'étape suivante dans l'historique
ok('tableau inséré', (await editor.locator('table').count()) === 1)
const cellsBefore = await editor.locator('table tr').count()
await btn('table-actions').click()
await p.getByRole('menuitem', { name: /^(Row below|Ligne dessous)$/ }).click()
await p.waitForTimeout(800)
ok('tableau : ligne ajoutée', (await editor.locator('table tr').count()) === cellsBefore + 1)

// Annuler / rétablir.
await p.waitForFunction(() => !document.querySelector('[data-tb="undo"]')?.disabled, null, { timeout: 5000 }).catch(() => {})
await btn('undo').click()
ok('annuler : ligne retirée', (await editor.locator('table tr').count()) === cellsBefore)
await btn('redo').click()
ok('rétablir : ligne revenue', (await editor.locator('table tr').count()) === cellsBefore + 1)

// Clavier : flèches entre les boutons, un seul arrêt de tabulation.
await btn('undo').focus()
await p.keyboard.press('ArrowRight')
ok('clavier : flèche droite', (await p.evaluate(() => document.activeElement?.getAttribute('data-tb'))) !== 'undo')
await p.keyboard.press('ArrowLeft')
ok('clavier : flèche gauche', (await p.evaluate(() => document.activeElement?.getAttribute('data-tb'))) === 'undo')
ok('clavier : un seul bouton tabulable', (await tb.locator('.tb-btn[tabindex="0"]').count()) === 1)

// Masquer (mémorisé) puis réafficher par le menu de la page.
await p.locator('.doc-more').click()
await p.getByRole('menuitem', { name: /^(Hide the toolbar|Masquer la barre d’outils)$/ }).click()
await p.waitForTimeout(400)
ok('masquée', (await tb.count()) === 0)
await p.reload({ waitUntil: 'networkidle' })
await editor.waitFor({ timeout: 15000 })
await p.waitForTimeout(800)
ok('masquage mémorisé après rechargement', (await tb.count()) === 0)
ok('préférence enregistrée', (await api(p, '/api/user/prefs')).editorToolbar === false)
await api(p, '/api/user/prefs', 'PATCH', { editorToolbar: null })

// Écran étroit (390 px) : repli dans « Plus ».
await p.setViewportSize({ width: 390, height: 800 })
await p.reload({ waitUntil: 'networkidle' })
await tb.waitFor({ timeout: 15000 })
ok('mobile : menu Plus', (await btn('more').count()) === 1)
ok('mobile : barre dans la largeur', (await tb.boundingBox()).width <= 390)
await p.screenshot({ path: OUT + 'editor-toolbar-390.png' })
await btn('more').click()
ok('mobile : le menu propose l’alignement', (await p.getByRole('menuitem', { name: /^(Center|Centrer)$/ }).count()) === 1)
await p.keyboard.press('Escape')

ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
