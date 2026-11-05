// « Nouvelle tâche » : le formulaire complet reprend les propriétés du panneau de la tâche
// (début, répétition, validation par personne…) ; la création rapide reste disponible.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'
const { browser, page: p, errors } = await launch()
await login(p)
const api = (path, method, body) => call(p, path, method, body)
const board = await api(`/api/workspaces/${WS}/boards`, 'POST', { name: 'Création complète', with_default_columns: true })
const tasks = async () => (await api(`/api/boards/${board.id}`)).columns.flatMap((c) => c.tasks)

// API : la création valide et enregistre la répétition comme la modification.
const col = (await api(`/api/boards/${board.id}`)).columns[1]
const bad = await api(`/api/boards/columns/${col.id}/tasks`, 'POST', { title: 'Mauvaise', repeat: 'hourly', due_date: new Date().toISOString() })
ok('API : répétition invalide refusée', bad?.status === 400)
const viaApi = await api(`/api/boards/columns/${col.id}/tasks`, 'POST', {
  title: 'Complète', repeat: 'weekly', due_date: '2030-01-07T00:00:00Z', start_date: '2030-01-01T00:00:00Z', is_per_person: true, require_all_validations: false,
})
ok('API : propriétés enregistrées', viaApi.repeat === 'weekly' && viaApi.is_per_person && !viaApi.require_all_validations && !!viaApi.start_date)

await p.goto(`${B}/w/${WS}/board/${board.id}`, { waitUntil: 'networkidle' })
await p.evaluate(() => localStorage.removeItem('looty.createTask.full'))
// Tableau ouvert en lecture (préférence de l'espace) : passage en modification.
await p.locator('.page-header, header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click()
await p.waitForTimeout(400)
const open = async () => {
  await p.getByRole('button', { name: /^(New task|Nouvelle tâche)$/ }).first().click()
  await p.waitForTimeout(500)
}
const dialog = p.locator('.dialog')

// Création rapide : titre puis Ctrl+Entrée, comme avant.
await open()
ok('création rapide par défaut', await dialog.locator('.ct-props').isVisible() && !(await dialog.locator('.ct-side').count()))
await p.keyboard.type('Rapide')
await p.keyboard.press('Control+Enter')
await p.waitForTimeout(800)
ok('création rapide : tâche créée', (await tasks()).some((t) => t.title === 'Rapide'))

// Formulaire complet.
await open()
await dialog.getByRole('button', { name: /^(Full form|Formulaire complet)$/ }).click()
await p.waitForTimeout(300)
const side = dialog.locator('.ct-side')
ok('formulaire complet affiché', await side.isVisible())
await dialog.locator('.ct-title input').fill('Complète par l’interface')
await dialog.locator('.ct-desc .ProseMirror').click()
await p.keyboard.type('Détails de la tâche')
// Échéance (aujourd'hui), puis répétition hebdomadaire.
await side.locator('.prop').nth(5).locator('button').click()
await p.locator('.dp-q').first().click()
await p.waitForTimeout(200)
await side.locator('.prop').nth(6).locator('button').click()
await p.locator('.combo-item').nth(2).click()
await p.waitForTimeout(200)
// Validation par personne, sans exiger tous les assignés.
await side.locator('.prop-switch').nth(0).locator('.switch').click()
await side.locator('.prop-switch').nth(1).locator('.switch').click()
await p.screenshot({ path: OUT + 'task-create-full.png' })
await dialog.getByRole('button', { name: /^(Create task|Créer la tâche)/ }).click()
await p.waitForTimeout(1000)
const created = (await tasks()).find((t) => t.title === 'Complète par l’interface')
ok('formulaire complet : tâche créée', !!created)
ok('description enregistrée', created?.description?.includes('Détails de la tâche'))
ok('échéance et répétition enregistrées', !!created?.due_date && created?.repeat === 'weekly', JSON.stringify({ due: created?.due_date, repeat: created?.repeat }))
ok('validation par personne enregistrée', created?.is_per_person === true && created?.require_all_validations === false)

// Le choix du formulaire complet est mémorisé.
await open()
ok('formulaire complet mémorisé', await dialog.locator('.ct-side').isVisible())
await p.keyboard.press('Escape')
ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
