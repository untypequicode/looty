// Aller-retour .ipynb → document Yjs → .ipynb (node --test).
const { test } = require('node:test')
const assert = require('node:assert')
const Y = require('yjs')
const { notebookState, notebookText } = require('./notebook')

const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=='
const ipynb = {
  cells: [
    { cell_type: 'markdown', id: 'md1', metadata: {}, source: ['# Titre\n', '![x](attachment:image.png)'], attachments: { 'image.png': { 'image/png': PNG } } },
    { cell_type: 'code', id: 'c1', metadata: { jupyter: { source_hidden: true } }, execution_count: 3, outputs: [{ output_type: 'stream', name: 'stdout', text: ['42\n'] }], source: 'print(42)' },
    { cell_type: 'raw', id: 'r1', metadata: {}, source: 'brut', attachments: { 'a.txt': { 'text/plain': 'aGk=' } } },
    { cell_type: 'markdown', id: 'md2', metadata: {}, source: 'sans pièce jointe' },
  ],
  metadata: { kernelspec: { name: 'python3', display_name: 'Python 3', language: 'python' } },
  nbformat: 4,
  nbformat_minor: 5,
}

test('attachments survive the round-trip', () => {
  const out = JSON.parse(notebookText(notebookState(JSON.stringify(ipynb))))
  assert.deepStrictEqual(out.cells[0].attachments, ipynb.cells[0].attachments)
  assert.deepStrictEqual(out.cells[2].attachments, ipynb.cells[2].attachments)
  assert.ok(!('attachments' in out.cells[3]))
  assert.ok(!('attachments' in out.cells[1]))
  assert.strictEqual(out.cells[0].source.join(''), ipynb.cells[0].source.join(''))
  assert.deepStrictEqual(out.cells[1].metadata, { jupyter: { source_hidden: true } })
  assert.strictEqual(out.cells[1].execution_count, 3)
  assert.deepStrictEqual(out.cells[1].outputs, ipynb.cells[1].outputs)
})

test('attachments are stored as JSON in the cell map', () => {
  const doc = new Y.Doc()
  Y.applyUpdate(doc, notebookState(JSON.stringify(ipynb)))
  const cells = doc.getArray('cells').toArray()
  assert.deepStrictEqual(JSON.parse(cells[0].get('attachments')), ipynb.cells[0].attachments)
  assert.strictEqual(cells[3].get('attachments'), undefined)
  doc.destroy()
})

test('second round-trip is stable', () => {
  const once = notebookText(notebookState(JSON.stringify(ipynb)))
  assert.strictEqual(notebookText(notebookState(once)), once)
})
