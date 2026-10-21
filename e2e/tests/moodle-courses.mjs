// K2 : panneau Cours complet (vues, tri, filtre, recherche, regroupement, dossiers, favori/masqué, personnalisation, panneau Dossier).
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch(1400, 1000)
await login(p)
const api = (path, method, body) => call(p, path, method, body)
await api(`/api/workspaces/${WS}/prefs`, 'PATCH', { moodleBoard: null, moodleCourses: null })
const M = `/api/workspaces/${WS}/moodle`
for (const [id, v] of [[3, true], [2, false]]) await api(`${M}/courses/${id}/hidden`, 'PUT', { value: v })
await api(`${M}/courses/2/favourite`, 'PUT', { value: false })
const all = await api(`${M}/courses?all=true`)
ok('api: all=true returns 6 incl. hidden flag + image', all.length === 6 && all.find((c) => c.id === 3).hidden === true && all[0].image?.startsWith('/api/'), JSON.stringify(all.map((c) => [c.id, c.hidden])))
ok('api: default list omits hidden', (await api(`${M}/courses`)).length === 5)
const prefs = async () => (await api(`/api/workspaces/${WS}/prefs`)).moodleCourses
const panel = p.locator('.gp', { has: p.locator('.gp-head h3', { hasText: /Mes cours|My courses/ }) })
const names = async () => panel.locator('.ci .ci-text').allInnerTexts()
await p.goto(`${B}/w/${WS}/moodle`, { waitUntil: 'networkidle' }); await p.waitForTimeout(1500)
let n = await names()
ok('default: in progress, cards, grouped by category', n.length === 3 && n.includes('Algèbre linéaire') && n.includes('Informatique') && !n.includes('Géométrie') && !n.includes('Probabilités'), JSON.stringify(n))
ok('category headings', (await panel.locator('.cp-toggle').allInnerTexts()).join('|').includes('L3') && (await panel.locator('.cp-toggle').allInnerTexts()).join('|').match(/Sans catégorie|No category/))
await p.screenshot({ path: OUT + 'k2-default.png' })

// Options (menu) : filtre
const opt = async (label) => { await panel.locator('button[aria-label*="filtre"], button[aria-label*="filter"]').first().click(); await p.waitForTimeout(250); await p.locator('.menu-item', { hasText: label }).first().click(); await p.waitForTimeout(500) }
await opt(/^Tous$|^All$/)
ok('filter all: 5 visible (hidden excluded)', (await names()).length === 5, JSON.stringify(await names()))
await opt(/^Passés$|^Past$/)
ok('filter past: Probabilités', JSON.stringify(await names()) === '["Probabilités"]', JSON.stringify(await names()))
await opt(/^Masqués$|^Hidden$/)
ok('filter hidden: Cours caché', JSON.stringify(await names()) === '["Cours caché"]', JSON.stringify(await names()))
await opt(/^À venir$|^Upcoming$/)
ok('filter upcoming: Géométrie', JSON.stringify(await names()) === '["Géométrie"]')
await opt(/^Favoris$|^Favourites$/)
ok('filter favourites: Algèbre', JSON.stringify(await names()) === '["Algèbre linéaire"]')
await opt(/^Tous$|^All$/)
// Tri par nom, regroupement aucun
await opt(/^Nom$|^Name$/); await opt(/^Aucun$|^None$/)
ok('sort by name, no group', JSON.stringify(await names()) === JSON.stringify(['Algèbre linéaire', 'Analyse', 'Géométrie', 'Informatique', 'Probabilités']), JSON.stringify(await names()))
await opt(/^Progression$|^Progress$/)
ok('sort by progress asc', (await names())[0] === 'Géométrie', JSON.stringify(await names()))
await opt(/Prochaine échéance|Next deadline/)
ok('sort by next deadline: Algèbre first', (await names())[0] === 'Algèbre linéaire' && (await names())[1] === 'Analyse', JSON.stringify(await names()))
// Recherche
await panel.locator('.cp-search input').fill('géom'); await p.waitForTimeout(300)
ok('search', JSON.stringify(await names()) === '["Géométrie"]')
await panel.locator('.cp-search input').fill('')
// Vues
for (const [title, cls] of [['Liste|List', 'list'], ['Compact', 'compact'], ['Cartes|Cards', 'cards']]) {
  await panel.locator('.seg-btn').filter({ has: p.locator('svg') }).evaluateAll((els, t) => els.find((e) => new RegExp(t).test(e.title))?.click(), title); await p.waitForTimeout(400)
  ok(`view ${cls}`, await panel.locator(`.cp-items.${cls}`).count() > 0)
}
let board = (await api(`/api/workspaces/${WS}/prefs`)).moodleBoard?.panels?.find((x) => x.kind === 'courses')
ok('panel options saved in the board', board?.opts?.sort === 'deadline' && board.opts.view === 'cards' && board.opts.group === 'none', JSON.stringify(board?.opts))

// Favori (étoile) synchronisé avec Moodle
await panel.locator('.ci', { hasText: 'Analyse' }).hover()
await panel.locator('.ci', { hasText: 'Analyse' }).locator('.ci-star').click(); await p.waitForTimeout(800)
ok('star synced to Moodle', (await api(`${M}/courses?all=true`)).find((c) => c.id === 2).favourite === true)
// Masquer par le menu
const item = (name) => panel.locator('.ci', { hasText: name }).first()
const itemMenu = async (name, label) => { await item(name).hover(); await item(name).locator('button[aria-label*="actions"], button[aria-label*="More"]').click(); await p.waitForTimeout(250); await p.locator('.menu-item', { hasText: label }).first().click(); await p.waitForTimeout(700) }
await itemMenu('Informatique', /Masquer le cours|Hide course/)
ok('hide synced to Moodle', (await api(`${M}/courses?all=true`)).find((c) => c.id === 6).hidden === true && !(await names()).includes('Informatique'))
await api(`${M}/courses/6/hidden`, 'PUT', { value: false })

// Dossiers : regroupement par dossiers, création, déplacement par le menu, glisser-déposer
await opt(/^Mes dossiers$|^My folders$/)
await panel.locator('button[aria-label*="Nouveau dossier"], button[aria-label*="New folder"]').first().click(); await p.waitForTimeout(300)
await p.locator('.confirm input, [role=dialog] input, dialog input').first().fill('Licence'); await p.keyboard.press('Enter'); await p.waitForTimeout(800)
ok('folder created', (await prefs())?.folders?.some((f) => f.name === 'Licence'), JSON.stringify(await prefs()))
await p.reload({ waitUntil: 'networkidle' }); await p.waitForTimeout(1500)
await item('Analyse').hover(); await item('Analyse').locator('button[aria-label*="actions"], button[aria-label*="More"]').click(); await p.waitForTimeout(250)
await p.locator('.menu-item', { hasText: /^Licence$/ }).click(); await p.waitForTimeout(800)
const fid = (await prefs()).folders[0].id
ok('moved to folder via menu', (await prefs()).assign['2'] === fid, JSON.stringify((await prefs()).assign))
// glisser Algèbre sur l'en-tête du dossier
const dt = await p.evaluateHandle(() => new DataTransfer())
await item('Algèbre').dispatchEvent('dragstart', { dataTransfer: dt })
await panel.locator('.cp-sec-head', { hasText: 'Licence' }).dispatchEvent('drop', { dataTransfer: dt }); await p.waitForTimeout(800)
ok('moved by drag and drop', (await prefs()).assign['1'] === fid, JSON.stringify((await prefs()).assign))
await p.screenshot({ path: OUT + 'k2-folders.png' })
// sous-dossier
await panel.locator('.cp-sec-head', { hasText: 'Licence' }).locator('button').last().click(); await p.waitForTimeout(250)
await p.locator('.menu-item', { hasText: /sous-dossier|subfolder/ }).click(); await p.waitForTimeout(300)
await p.locator('[role=dialog] input, dialog input, .confirm input').first().fill('L3'); await p.keyboard.press('Enter'); await p.waitForTimeout(800)
ok('subfolder created', (await prefs()).folders.some((f) => f.name === 'L3' && f.parent === fid))

// Personnalisation
await itemMenu('Analyse', /Personnaliser|Customize/)
await p.locator('[role=dialog] input').first().fill('Ana'); await p.locator('.cz-color').nth(2).click()
await p.locator('[role=dialog] input').nth(1).fill('📐')
await p.locator('[role=dialog] button', { hasText: /Enregistrer|Save/ }).click(); await p.waitForTimeout(800)
const cu = (await prefs()).custom['2']
ok('customization saved', cu?.short === 'Ana' && cu.icon === '📐' && cu.color, JSON.stringify(cu))
ok('custom name shown in panel', (await names()).includes('Ana'))
ok('custom name in deadlines panel', (await p.locator('.dl-meta').allInnerTexts()).some((x) => x.startsWith('Ana')), JSON.stringify(await p.locator('.dl-meta').allInnerTexts()))
await p.screenshot({ path: OUT + 'k2-custom.png' })

// Panneau Dossier
await p.locator('button', { hasText: /Ajouter un panneau|Add a panel/ }).click(); await p.waitForTimeout(200)
await p.locator('.menu-item', { hasText: /^Dossier$|^Folder$/ }).click(); await p.waitForTimeout(800)
await p.locator('.mb-course', { hasText: /^Licence$/ }).click(); await p.waitForTimeout(1000)
const fp = p.locator('.gp', { has: p.locator('.gp-head h3', { hasText: 'Licence' }) }).last()
ok('folder panel shows its courses', (await fp.locator('.ci .ci-text').allInnerTexts()).sort().join() === 'Algèbre linéaire,Ana', JSON.stringify(await fp.locator('.ci .ci-text').allInnerTexts()))
// Échéances : cours masqué
await api(`${M}/courses/1/hidden`, 'PUT', { value: true }); await p.reload({ waitUntil: 'networkidle' }); await p.waitForTimeout(1500)
const dls = async () => p.locator('.dl').count()
ok('hidden course deadlines hidden by default', (await dls()) === 1, String(await dls()))
await p.locator('.gp', { has: p.locator('.gp-head h3', { hasText: /Échéances|Deadlines/ }) }).locator('.gp-head button').last().click(); await p.waitForTimeout(250)
await p.locator('.menu-item', { hasText: /Inclure les cours masqués|Include hidden/ }).click(); await p.waitForTimeout(700)
ok('include hidden courses option', (await dls()) === 3, String(await dls()))
await api(`${M}/courses/1/hidden`, 'PUT', { value: false })
// Supprimer un dossier libère ses cours
await p.reload({ waitUntil: 'networkidle' }); await p.waitForTimeout(1500)
await panel.locator('.cp-sec-head', { hasText: 'Licence' }).locator('button').last().click(); await p.waitForTimeout(250)
await p.locator('.menu-item', { hasText: /^Supprimer$|^Delete$/ }).click(); await p.waitForTimeout(300)
await p.locator('button', { hasText: /^Supprimer$|^Delete$/ }).last().click(); await p.waitForTimeout(800)
const pr = await prefs()
ok('delete folder releases its courses', pr.folders.length === 0 && Object.keys(pr.assign).length === 0, JSON.stringify(pr))
ok('custom kept', pr.custom['2']?.short === 'Ana')
ok('no page errors', errors.length === 0, errors.join(' | '))
await browser.close()
