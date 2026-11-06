// Diagrammes et blocs graphe : styles de l'espace (validation, style par défaut), taille des
// diagrammes Mermaid, style choisi d'un clic, nœud ajouté au double-clic dans un graphe.
import { B, WS, OUT, ok, launch, login, api as call } from '../lib/index.mjs'

const { browser, page: p, errors } = await launch(1440, 1000)
await login(p)
const api = (path, method, body) => call(p, path, method, body)

const palette = { background: '#fff8e1', node: '#ffe0b2', border: '#e65100', text: '#3e2723', edge: '#6d4c41', accent: '#fff3e0' }
const style = { id: 'cours', name: 'Cours', look: 'classic', layout: 'elk', font: 'serif', shape: 'circle', light: palette, dark: palette }

// Les couleurs finissent dans le CSS des diagrammes : seules les couleurs #rrggbb passent.
const bad = await api(`/api/workspaces/${WS}/diagram-styles`, 'PUT', { default: 'cours', styles: [{ ...style, light: { ...palette, node: 'red;}*{display:none' } }] })
ok('styles : couleur invalide refusée', bad?.status === 400, JSON.stringify(bad))
const saved = await api(`/api/workspaces/${WS}/diagram-styles`, 'PUT', { default: 'cours', styles: [style] })
ok('styles : style personnalisé par défaut', saved?.default === 'cours' && saved.styles?.[0]?.name === 'Cours')

const mermaid = 'flowchart LR\n  A[Début] --> B{Choix}\n  B -- Oui --> C[Fin]\n  B -- Non --> D[Retour]\n  D --> A'
const graph = 's : Source @ 0 100\na @ 160 20\nb @ 160 180\nt : Puits @ 320 100\ns -> a : 3/4\ns -> b : 2/2\na -> t : 2/3 *\nb -> t : 3/5'
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
const html = `<p>Flot</p><div data-type="mermaid"><pre>${esc(mermaid)}</pre></div><div data-type="graph"><pre>${esc(graph)}</pre></div><p></p>`
const pg = await api(`/api/workspaces/${WS}/pages`, 'POST', { title: 'Graphes', content: html })

await p.goto(`${B}/w/${WS}/doc/${pg.id}`, { waitUntil: 'networkidle' })
await p.waitForSelector('.mmd-svg svg', { timeout: 20000 })
const box = await p.locator('.mmd-svg svg').boundingBox()
ok('mermaid : affiché à sa taille (pas réduit)', box && box.width > 300, `${box?.width}px`)
const css = await p.locator('.mmd-svg svg style').first().textContent()
ok('mermaid : couleurs du style de l’espace', css.includes('#ffe0b2') || css.toLowerCase().includes('255, 224, 178'))

await p.waitForSelector('.gr-host canvas', { timeout: 20000 })
const bg = await p.locator('.gr-stage').evaluate((el) => getComputedStyle(el).backgroundColor)
ok('graphe : fond du style de l’espace', bg === 'rgb(255, 248, 225)', bg)
await p.screenshot({ path: OUT + 'diagram-blocks.png' })

// Édition : style d'un clic, nœud au double-clic.
await p.locator('header').getByRole('button', { name: /^(Edit|Modifier)$/ }).first().click()
await p.waitForTimeout(600)
await p.locator('.mmd .dsp').click()
await p.getByRole('menuitem', { name: /^(High contrast|Contraste)$/ }).click()
await p.waitForTimeout(800)
ok('mermaid : style du bloc', (await p.locator('.mmd .dsp').textContent()).match(/Contraste|High contrast/))

await p.locator('.gr-host').scrollIntoViewIfNeeded()
const stage = await p.locator('.gr-host').boundingBox()
await p.mouse.dblclick(stage.x + 30, stage.y + stage.height - 30)
await p.waitForSelector('.gr-edit', { timeout: 5000 })
await p.keyboard.press('ControlOrMeta+a')
await p.keyboard.type('Puits 2')
await p.keyboard.press('Enter')
await p.waitForTimeout(3000)
const content = (await api(`/api/workspaces/${WS}/pages/${pg.id}`))?.content ?? ''
ok('graphe : nœud ajouté au double-clic', /A : Puits 2 @ -?\d+ -?\d+/.test(content), content.slice(0, 400))
ok('mermaid : style enregistré dans la page', content.includes('data-style="contraste"'))
await p.screenshot({ path: OUT + 'diagram-blocks-edit.png' })

// « Disposer » : tous les nœuds reçoivent une position, écrite dans la source.
await p.locator('.gr-tools').getByRole('button', { name: /Arrange|Disposer/ }).click()
await p.getByRole('menuitem', { name: /Circle|En cercle/ }).click()
await p.waitForTimeout(3000)
const arranged = (await api(`/api/workspaces/${WS}/pages/${pg.id}`))?.content ?? ''
ok('graphe : disposition écrite', !arranged.includes('s : Source @ 0 100') && /s : Source @ -?\d+ -?\d+/.test(arranged) && /\nb @ -?\d+ -?\d+/.test(arranged))

// Plein écran puis retour au document.
await p.locator('.gr-bar button[title="Full screen"], .gr-bar button[title="Plein écran"]').click()
await p.waitForSelector('.gr-full .gr-host canvas', { timeout: 5000 })
const fullBox = await p.locator('.gr-full .gr-host').boundingBox()
ok('graphe : plein écran', fullBox && fullBox.height > 700, `${fullBox?.height}px`)
await p.locator('.gr-full-bar button').last().click()
await p.waitForTimeout(500)
ok('graphe : retour dans la page', !(await p.locator('.gr-full').count()) && (await p.locator('.gr .gr-host canvas').count()) > 0)

// Réglages : section des styles, aperçu rendu.
await p.goto(`${B}/w/${WS}/settings/workspace#diagram-styles`, { waitUntil: 'networkidle' })
await p.waitForSelector('#diagram-styles .ds-preview svg', { timeout: 20000 })
ok('réglages : style personnalisé listé', (await p.locator('#diagram-styles .ds-item').first().textContent()).includes('Cours'))

ok('aucune erreur page', !errors.length, errors.join(' | '))
await browser.close()
