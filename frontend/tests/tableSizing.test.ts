// node --test frontend/tests/tableSizing.test.ts
// Tableaux redimensionnables : largeur de colonne (colwidth) et hauteur de ligne (rowheight) survivent à un
// aller-retour HTML, y compris pour un ancien contenu sans taille.
import test from 'node:test'
import assert from 'node:assert/strict'
import { JSDOM } from 'jsdom'

const dom = new JSDOM('<!doctype html><html><body></body></html>', { url: 'https://looty.test/' })
const g = globalThis as any
for (const k of ['window', 'document', 'DOMParser', 'Node', 'Element', 'HTMLElement', 'DocumentFragment', 'MutationObserver', 'getComputedStyle']) {
  Object.defineProperty(g, k, { value: k === 'window' ? dom.window : (dom.window as any)[k], configurable: true, writable: true })
}

const { Editor } = await import('@tiptap/core')
const { default: StarterKit } = await import('@tiptap/starter-kit')
const { Table, TableCell, TableHeader } = await import('@tiptap/extension-table')
const { ResizableTableRow, clampRowHeight } = await import('../app/editor/tableSizing.ts')

const make = (content: string, editable = false) =>
  new Editor({ element: dom.window.document.createElement('div'), editable, content, extensions: [StarterKit, Table.configure({ resizable: true }), ResizableTableRow, TableHeader, TableCell] })

test('clampRowHeight bounds and rejects junk', () => {
  assert.equal(clampRowHeight(5), 24)
  assert.equal(clampRowHeight('80.4px'), 80)
  assert.equal(clampRowHeight(99999), 1000)
  assert.equal(clampRowHeight(null), null)
  assert.equal(clampRowHeight('auto'), null)
  assert.equal(clampRowHeight(-3), null)
})

test('row height and column width are kept in the document and rendered', () => {
  const ed = make('<table><tr><th colwidth="150"><p>a</p></th><th><p>b</p></th></tr><tr style="height: 64px"><td colwidth="150"><p>1</p></td><td><p>2</p></td></tr></table>')
  const json = ed.getJSON() as any
  const rows = json.content[0].content
  assert.equal(rows[0].attrs.rowheight, null)
  assert.equal(rows[1].attrs.rowheight, 64)
  assert.deepEqual(rows[0].content[0].attrs.colwidth, [150])
  const html = ed.getHTML()
  assert.match(html, /data-rowheight="64"/)
  assert.match(html, /height: 64px/)
  assert.match(html, /<col style="width: 150px;?">/)
  ed.destroy()
})

test('existing table without sizes is unchanged', () => {
  const ed = make('<table><tr><td><p>x</p></td></tr></table>')
  assert.doesNotMatch(ed.getHTML(), /rowheight|height:/)
  ed.destroy()
})

test('cell text direction survives a round trip and is styled inline', async () => {
  const { DirectionalCell, DirectionalHeader, cellDirection } = await import('../app/editor/tableSizing.ts')
  assert.equal(cellDirection('sideways'), null)
  const ed = new Editor({ element: dom.window.document.createElement('div'), editable: false, extensions: [StarterKit, Table, ResizableTableRow, DirectionalHeader, DirectionalCell],
    content: '<table><tr><th data-text-dir="up"><p>a</p></th><td data-text-dir="down"><p>b</p></td><td data-text-dir="bogus"><p>c</p></td></tr></table>' })
  const cells = (ed.getJSON() as any).content[0].content[0].content
  assert.deepEqual(cells.map((c: any) => c.attrs.textDirection), ['up', 'down', null])
  const html = ed.getHTML()
  assert.match(html, /data-text-dir="up" style="[^"]*writing-mode: sideways-lr/)
  assert.match(html, /writing-mode: vertical-rl/)
  ed.destroy()
})
