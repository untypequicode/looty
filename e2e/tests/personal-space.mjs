// Espace perso : un espace privé par compte, joignable depuis la barre latérale de chaque
// espace (documents, tableaux, raccourcis), où personne d'autre ne peut entrer.
import { B, WS, OUT, ok, launch, login, api } from '../lib/index.mjs'

const N = Date.now() % 100000
const PERSO = /^(Espace perso|Personal space)$/
const { browser, page: p, errors } = await launch(1440, 950)
await login(p)

// Créé à la première demande, toujours le même ensuite.
const first = await api(p, '/api/workspaces/personal')
const again = await api(p, '/api/workspaces/personal')
const PID = first.id
ok('espace perso créé une seule fois', !!PID && PID === again.id)
const list = await api(p, '/api/workspaces')
ok('marqué « perso » dans la liste', list.workspaces.some((w) => w.id === PID && w.personal) && list.workspaces.some((w) => w.id === WS && !w.personal))
await api(p, `/api/workspaces/${PID}/prefs`, 'PATCH', { modes: { docs: 'edit', boards: 'edit', calendar: 'edit' }, modesChosen: true })

// Personne d'autre n'y entre, et on ne peut ni le quitter ni le supprimer.
const invite = await api(p, `/api/workspaces/${PID}/invites`, 'POST', { role: 'member' })
ok('invitation refusée', invite?.status === 409, JSON.stringify(invite))
ok('départ refusé', (await api(p, `/api/workspaces/${PID}/leave`, 'POST'))?.status === 409)
ok('suppression refusée', (await api(p, `/api/workspaces/${PID}`, 'DELETE'))?.status === 409)
const other = await browser.newContext()
const bob = await other.newPage()
await login(bob, 'bob@x.io')
ok('invisible pour un autre compte', (await api(bob, `/api/workspaces/${PID}`))?.status === 404)
ok('un autre compte a son propre espace', (await api(bob, '/api/workspaces/personal')).id !== PID)
await other.close()

// Contenu perso, créé par l'API : il apparaît dans la barre latérale d'un autre espace.
const board = await api(p, `/api/workspaces/${PID}/boards`, 'POST', { name: `Courses perso ${N}`, with_default_columns: true })
await api(p, `/api/workspaces/${PID}/pages`, 'POST', { title: `Journal ${N}`, icon: '📓' })

await p.goto(`${B}/w/${WS}`, { waitUntil: 'networkidle' })
const section = p.locator('.sb-section', { has: p.getByText(PERSO, { exact: true }) })
ok('section « Espace perso » dans la barre latérale', await section.isVisible())
ok('tableau perso listé', await section.getByText(`Courses perso ${N}`).isVisible())
ok('document perso listé', await section.getByText(`Journal ${N}`).isVisible())
ok('raccourcis (accueil, calendrier, mail, projets, drive, canevas)', (await section.locator('.sb-perso-link').count()) === 6)
await p.screenshot({ path: OUT + 'personal-sidebar.png' })

const wsName = (await p.locator('.ws-trigger .ws-name').textContent())?.trim()
const shellKept = async () => (await p.locator('.ws-trigger .ws-name').textContent())?.trim() === wsName && await section.isVisible()

// Un élément perso s'ouvre sans quitter l'espace courant : sa barre latérale reste.
await section.getByText(`Courses perso ${N}`).click()
await p.waitForURL(new RegExp(`/w/${PID}/board/${board.id}`), { timeout: 8000 })
await p.waitForTimeout(500)
ok('tableau perso ouvert', await p.locator('main').getByText(`Courses perso ${N}`).first().isVisible())
ok('espace courant gardé autour du tableau perso', await shellKept())
ok('accueil de la barre = espace courant', (await p.locator('.sb-scroll .sb-group').first().locator('a').first().getAttribute('href')) === `/w/${WS}`)
await p.screenshot({ path: OUT + 'personal-borrowed.png' })
await p.reload({ waitUntil: 'networkidle' })
ok('gardé après rechargement', await shellKept())

// Création directe : document, puis tableau, toujours dans l'espace courant.
await section.hover()
await section.locator('.sb-section-add').click()
await p.getByText(/^(Nouveau document|New document)$/).first().click()
await p.waitForURL(new RegExp(`/w/${PID}/doc/`), { timeout: 8000 })
ok('nouveau document perso, espace courant gardé', await shellKept())
await section.hover()
await section.locator('.sb-section-add').click()
await p.getByText(/^(Nouveau tableau|New board)$/).first().click()
await p.locator('.form input').first().fill(`Tableau perso ${N}`)
await p.keyboard.press('Enter')
await p.waitForURL(new RegExp(`/w/${PID}/board/`), { timeout: 8000 })
ok('nouveau tableau créé dans l’espace perso', (await api(p, `/api/workspaces/${PID}/boards`)).some((b) => b.name === `Tableau perso ${N}`))
ok('nouveau tableau, espace courant gardé', await shellKept())
await p.locator('.sb-scroll a', { hasText: /^(Accueil|Home)$/ }).first().click()
await p.waitForURL(new RegExp(`/w/${WS}$`), { timeout: 8000 })
ok('retour à l’espace courant', p.url().endsWith(`/w/${WS}`))

// Le menu des espaces ouvre l'espace perso pour lui-même.
await p.locator('.ws-trigger').click()
await p.locator('.ws-row', { hasText: /Espace perso|Personal space/ }).click()
await p.waitForURL(new RegExp(`/w/${PID}$`), { timeout: 8000 })
await p.waitForTimeout(500)
ok('menu : « Espace perso »', PERSO.test((await p.locator('.ws-trigger .ws-name').textContent())?.trim() ?? ''))
ok('pas de section perso dans l’espace perso', !(await p.locator('.sb-section', { has: p.getByText(PERSO, { exact: true }) }).count()))
await p.locator('.ws-trigger').click()
ok('menu : entrée de l’espace perso cochée', await p.locator('.ws-row', { hasText: /Espace perso|Personal space/ }).locator('.accent').isVisible())
ok('menu : pas d’invitation', !(await p.locator('.ws-row', { hasText: /Inviter|Invite/ }).count()))
await p.keyboard.press('Escape')
await p.goto(`${B}/w/${PID}/settings/workspace`, { waitUntil: 'networkidle' })
ok('réglages : pas de membres', !(await p.locator('.st-group').getByText(/^(Membres|Members)$/).count()))
ok('réglages : pas de zone de danger', !(await p.getByText(/Quitter l’espace|Leave workspace/).count()))
await p.screenshot({ path: OUT + 'personal-settings.png' })

ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
