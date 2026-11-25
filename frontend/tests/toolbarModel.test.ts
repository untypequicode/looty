// node --test frontend/tests/toolbarModel.test.ts
// Barre d'outils de l'éditeur : repli en « Plus », état de la sélection, menu « Insérer », préférence, clavier.
import test from 'node:test'
import assert from 'node:assert/strict'
import {
  TOOLBAR_GROUPS, TOOLBAR_COVERED, MORE_WIDTH, EMPTY_STATE, groupWidth, splitGroups, insertItems, toolbarState, sameState,
  toolbarVisible, rovingNext, keyboardInset, type ToolbarSource,
} from '../app/editor/toolbarModel.ts'
import { cleanColor, cleanAlign } from '../app/editor/textStyle.ts'

const total = TOOLBAR_GROUPS.reduce((n, g) => n + groupWidth(g), 0)

test('large screen: every group stays in the bar, no More menu', () => {
  const r = splitGroups(total + 10)
  assert.equal(r.overflow.length, 0)
  assert.deepEqual(r.visible, TOOLBAR_GROUPS.map((g) => g.id))
})

test('narrow screen: lowest-priority groups fold first, order is kept', () => {
  const r = splitGroups(total - 1)
  assert.equal(r.overflow[0], 'align')
  assert.ok(r.visible.includes('block') && r.visible.includes('marks') && r.visible.includes('more'))
  const order = TOOLBAR_GROUPS.map((g) => g.id)
  for (const part of [r.visible, r.overflow]) assert.deepEqual(part, order.filter((id) => part.includes(id)))
})

test('the bar plus its More button always fits the width, down to the essentials', () => {
  for (let w = 120; w < total; w += 17) {
    const r = splitGroups(w)
    const used = TOOLBAR_GROUPS.filter((g) => r.visible.includes(g.id)).reduce((n, g) => n + groupWidth(g), 0) + (r.overflow.length ? MORE_WIDTH : 0)
    const essentials = TOOLBAR_GROUPS.filter((g) => g.priority <= 0).reduce((n, g) => n + groupWidth(g), 0) + MORE_WIDTH
    assert.ok(used <= w || used <= essentials, `width ${w}`)
    assert.ok(r.visible.includes('block') && r.visible.includes('more'))
  }
})

test('insert menu reuses the slash list without repeating what the bar already has', () => {
  const slash = ['p', 'h1', 'ul', 'toggle', 'info', 'table', 'image', 'math', 'mathi', 'mermaid', 'embed', 'date'].map((id) => ({ id }))
  assert.deepEqual(insertItems(slash).map((i) => i.id), ['toggle', 'info', 'mathi', 'mermaid', 'embed', 'date'])
  assert.ok(TOOLBAR_COVERED.includes('h1') && !TOOLBAR_COVERED.includes('mermaid' as never))
})

function source(active: Record<string, Record<string, unknown> | true>, attrs: Record<string, Record<string, unknown>> = {}, can: Partial<ToolbarSource['can']> = {}): ToolbarSource {
  return {
    isActive: (n, a) => {
      const v = active[n]
      if (!v) return false
      return v === true || !a || Object.entries(a).every(([k, x]) => v[k] === x)
    },
    getAttributes: (n) => attrs[n] ?? {},
    can: { undo: () => false, redo: () => false, indent: () => false, outdent: () => false, mergeCells: () => true, splitCell: () => true, ...can },
  }
}

test('state mirrors the selection', () => {
  const s = toolbarState(source({ bold: true, heading: { level: 2 }, bulletList: true }, { heading: { textAlign: 'center' }, textColor: { color: 'red' } }, { undo: () => true }))
  assert.equal(s.bold, true)
  assert.equal(s.italic, false)
  assert.equal(s.block, 'h2')
  assert.equal(s.bulletList, true)
  assert.equal(s.align, 'center')
  assert.equal(s.color, 'red')
  assert.equal(s.canUndo, true)
  assert.equal(s.canRedo, false)
})

test('code block and quote win over paragraph; defaults without selection data', () => {
  assert.equal(toolbarState(source({ codeBlock: true })).block, 'code')
  assert.equal(toolbarState(source({ blockquote: true })).block, 'quote')
  assert.deepEqual(toolbarState(source({})), EMPTY_STATE)
})

test('table actions are only asked inside a table', () => {
  let asked = 0
  const can = { mergeCells: () => (asked++, true), splitCell: () => (asked++, true) }
  assert.equal(toolbarState(source({}, {}, can)).canMerge, false)
  assert.equal(asked, 0)
  const s = toolbarState(source({ table: true }, {}, can))
  assert.equal(s.inTable, true)
  assert.equal(s.canMerge && s.canSplit, true)
})

test('sameState lets a keystroke that changes nothing skip the redraw', () => {
  const a = toolbarState(source({ bold: true }))
  assert.equal(sameState(a, toolbarState(source({ bold: true }))), true)
  assert.equal(sameState(a, toolbarState(source({ bold: true, italic: true }))), false)
})

test('preference: shown unless explicitly hidden', () => {
  assert.equal(toolbarVisible(undefined), true)
  assert.equal(toolbarVisible(null), true)
  assert.equal(toolbarVisible(true), true)
  assert.equal(toolbarVisible(false), false)
})

test('roving focus wraps and honours Home and End', () => {
  assert.equal(rovingNext(0, 5, 'ArrowRight'), 1)
  assert.equal(rovingNext(4, 5, 'ArrowRight'), 0)
  assert.equal(rovingNext(0, 5, 'ArrowLeft'), 4)
  assert.equal(rovingNext(2, 5, 'Home'), 0)
  assert.equal(rovingNext(2, 5, 'End'), 4)
  assert.equal(rovingNext(2, 5, 'a'), 2)
  assert.equal(rovingNext(0, 0, 'ArrowRight'), -1)
})

test('keyboard inset follows the visual viewport', () => {
  assert.equal(keyboardInset(800, { height: 500, offsetTop: 0 }), 300)
  assert.equal(keyboardInset(800, { height: 800, offsetTop: 0 }), 0)
  assert.equal(keyboardInset(800, { height: 500, offsetTop: 40 }), 260)
  assert.equal(keyboardInset(800, null), 0)
})

test('colors and alignment only accept known values', () => {
  assert.equal(cleanColor('red'), 'red')
  assert.equal(cleanColor('#ff0000'), null)
  assert.equal(cleanColor('url(x)'), null)
  assert.equal(cleanAlign('center'), 'center')
  assert.equal(cleanAlign('left'), null)
  assert.equal(cleanAlign('foo'), null)
})
