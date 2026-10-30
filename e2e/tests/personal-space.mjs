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

// Création directe depuis l'espace courant.
await section.hover()
await section.locator('.sb-section-add').click()
await p.getByText(/^(Nouveau document|New document)$/).first().click()
await p.waitForURL(new RegExp(`/w/${PID}/doc/`), { timeout: 8000 })
ok('nouveau document ouvert dans l’espace perso', p.url().includes(`/w/${PID}/doc/`))

// Dans l'espace perso : il porte mon nom, sans membres ni rôles dans les réglages.
ok('menu : « Espace perso »', PERSO.test((await p.locator('.ws-trigger .ws-name').textContent())?.trim() ?? ''))
await p.locator('.ws-trigger').click()
ok('menu : entrée de l’espace perso cochée', await p.locator('.ws-row', { hasText: PERSO.source.includes('Personal') ? /Espace perso|Personal space/ : PERSO }).locator('.accent').isVisible())
ok('menu : pas d’invitation', !(await p.locator('.ws-row', { hasText: /Inviter|Invite/ }).count()))
await p.keyboard.press('Escape')
ok('pas de section perso dans l’espace perso', !(await p.locator('.sb-section', { has: p.getByText(PERSO, { exact: true }) }).count()))
await p.goto(`${B}/w/${PID}/board/${board.id}`, { waitUntil: 'networkidle' })
ok('tableau perso ouvert', await p.getByText(`Courses perso ${N}`).first().isVisible())
await p.goto(`${B}/w/${PID}/settings/workspace`, { waitUntil: 'networkidle' })
ok('réglages : pas de membres', !(await p.locator('.st-group').getByText(/^(Membres|Members)$/).count()))
ok('réglages : pas de zone de danger', !(await p.getByText(/Quitter l’espace|Leave workspace/).count()))
await p.screenshot({ path: OUT + 'personal-settings.png' })

ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
