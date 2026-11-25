// node --test frontend/tests/slidesModel.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import * as Y from 'yjs'
import {
  LOCAL, MAX_ELEMENTS, SLIDES_SCHEMA, addElement, alignElements, createSlide, createUndo, deleteElements, deleteSlide, distributeElements, docSchema, duplicateElements, duplicateSlide,
  expandGroups, groupElements, initDoc, isNewerThanClient, listSlides, moveSlide, moveZ, outline, posBetween, readSlide, reorderIds, slideIds, textOf,
  ungroupElements, updateElement,
} from '../app/slides/model.ts'

const fresh = () => { const d = new Y.Doc(); initDoc(d); return d }
const rect = (x = 0, y = 0) => ({ type: 'rect' as const, x, y, w: 100, h: 50, props: { fill: 'blue' } })
const sync = (from: Y.Doc, to: Y.Doc) => Y.applyUpdate(to, Y.encodeStateAsUpdate(from))

test('a new document has its schema, 16:9 size and one blank slide; init is idempotent', () => {
  const d = fresh()
  initDoc(d)
  assert.equal(docSchema(d), SLIDES_SCHEMA)
  assert.equal(isNewerThanClient(d), false)
  assert.equal(listSlides(d).length, 1)
  assert.deepEqual([d.getMap('meta').get('width'), d.getMap('meta').get('height')], [1920, 1080])
  d.getMap('meta').set('schema', SLIDES_SCHEMA + 1)
  assert.equal(isNewerThanClient(d), true)
})

test('create, move, duplicate and delete slides keep a stable order', () => {
  const d = fresh()
  const first = slideIds(d)[0]!
  const b = createSlide(d)!
  const c = createSlide(d, { afterId: first })!
  assert.deepEqual(slideIds(d), [first, c, b])
  moveSlide(d, b, 0)
  assert.deepEqual(slideIds(d), [b, first, c])
  moveSlide(d, b, 99)
  assert.deepEqual(slideIds(d), [first, c, b])
  const copy = duplicateSlide(d, first)!
  assert.deepEqual(slideIds(d), [first, copy, c, b])
  assert.equal(deleteSlide(d, copy), c)
  assert.equal(slideIds(d).length, 3)
})

test('deleting the last slide leaves a fresh blank one', () => {
  const d = fresh()
  const only = slideIds(d)[0]!
  const next = deleteSlide(d, only)!
  assert.notEqual(next, only)
  assert.deepEqual(slideIds(d), [next])
})

test('many insertions at the same place never collapse the order', () => {
  const d = fresh()
  const first = slideIds(d)[0]!
  const ids: string[] = []
  for (let i = 0; i < 60; i++) ids.unshift(createSlide(d, { afterId: first })!)
  assert.deepEqual(slideIds(d), [first, ...ids])
  assert.equal(posBetween(), 1)
  assert.equal(posBetween(2, 4), 3)
})

test('concurrent reorders and edits merge without losing or duplicating slides', () => {
  const a = fresh()
  const ids = [slideIds(a)[0]!, createSlide(a)!, createSlide(a)!]
  const b = new Y.Doc()
  sync(a, b)
  moveSlide(a, ids[2]!, 0)
  moveSlide(b, ids[0]!, 3)
  const el = addElement(a, ids[1]!, rect())!
  sync(a, b)
  sync(b, a)
  assert.deepEqual(slideIds(a), slideIds(b))
  assert.equal(new Set(slideIds(a)).size, 3)
  assert.equal(readSlide(b, ids[1]!)!.elements[0]!.id, el)
})

test('elements: add on top, patch only the given keys, delete', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  const a = addElement(d, s, rect(10, 20))!
  const b = addElement(d, s, { ...rect(), type: 'ellipse' })!
  assert.deepEqual(readSlide(d, s)!.elements.map((e) => e.id), [a, b])
  updateElement(d, s, a, { x: 300, props: { stroke: 'red' } })
  const e = readSlide(d, s)!.elements[0]!
  assert.deepEqual([e.x, e.y, e.props.fill, e.props.stroke], [300, 20, 'blue', 'red'])
  updateElement(d, s, a, { props: { stroke: undefined } })
  assert.equal(readSlide(d, s)!.elements[0]!.props.stroke, undefined)
  deleteElements(d, s, [a])
  assert.deepEqual(readSlide(d, s)!.elements.map((x) => x.id), [b])
})

test('two people changing different fields of the same element both win', () => {
  const a = fresh()
  const s = slideIds(a)[0]!
  const id = addElement(a, s, rect())!
  const b = new Y.Doc()
  sync(a, b)
  updateElement(a, s, id, { x: 500 })
  updateElement(b, s, id, { props: { fill: 'green' } })
  sync(b, a)
  sync(a, b)
  for (const d of [a, b]) {
    const e = readSlide(d, s)!.elements[0]!
    assert.deepEqual([e.x, e.props.fill], [500, 'green'])
  }
})

test('element limit', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  for (let i = 0; i < MAX_ELEMENTS; i++) addElement(d, s, rect())
  assert.equal(addElement(d, s, rect()), null)
})

test('z-order moves', () => {
  assert.deepEqual(reorderIds(['a', 'b', 'c', 'd'], new Set(['b']), 'front'), ['a', 'c', 'd', 'b'])
  assert.deepEqual(reorderIds(['a', 'b', 'c', 'd'], new Set(['c']), 'back'), ['c', 'a', 'b', 'd'])
  assert.deepEqual(reorderIds(['a', 'b', 'c', 'd'], new Set(['a', 'b']), 'forward'), ['c', 'a', 'b', 'd'])
  assert.deepEqual(reorderIds(['a', 'b', 'c', 'd'], new Set(['d']), 'backward'), ['a', 'b', 'd', 'c'])
  assert.deepEqual(reorderIds(['a', 'b'], new Set(['b']), 'forward'), ['a', 'b'])
  const d = fresh()
  const s = slideIds(d)[0]!
  const [a, b, c] = [addElement(d, s, rect())!, addElement(d, s, rect())!, addElement(d, s, rect())!]
  moveZ(d, s, [a], 'front')
  assert.deepEqual(readSlide(d, s)!.elements.map((e) => e.id), [b, c, a])
  assert.deepEqual(readSlide(d, s)!.elements.map((e) => e.z), [0, 1, 2])
})

test('groups expand a selection and ungroup clears them', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  const [a, b, c] = [addElement(d, s, rect())!, addElement(d, s, rect())!, addElement(d, s, rect())!]
  assert.equal(groupElements(d, s, [a]), null)
  const g = groupElements(d, s, [a, b])!
  const els = readSlide(d, s)!.elements
  assert.deepEqual(new Set(expandGroups(els, [a])), new Set([a, b]))
  assert.deepEqual(expandGroups(els, [c]), [c])
  assert.equal(els.find((e) => e.id === a)!.groupId, g)
  ungroupElements(d, s, [a])
  assert.ok(readSlide(d, s)!.elements.every((e) => e.groupId === ''))
})

test('align and distribute write positions, skipping locked elements', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  const a = addElement(d, s, rect(0, 0))!
  const b = addElement(d, s, rect(130, 0))!
  const c = addElement(d, s, rect(500, 0))!
  distributeElements(d, s, [a, b, c], 'x')
  assert.deepEqual(readSlide(d, s)!.elements.map((e) => e.x), [0, 250, 500])
  updateElement(d, s, c, { locked: true })
  alignElements(d, s, [a, b, c], 'left')
  assert.deepEqual(readSlide(d, s)!.elements.map((e) => e.x), [0, 0, 500])
})

test('duplicating elements shifts them, regroups with new ids and clones text independently', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  const t = addElement(d, s, { type: 'text', x: 0, y: 0, w: 400, h: 100, text: 'Hello\nWorld' })!
  const r = addElement(d, s, rect())!
  const g = groupElements(d, s, [t, r])!
  const copies = duplicateElements(d, s, [t, r])
  assert.equal(copies.length, 2)
  const els = readSlide(d, s)!.elements
  const ct = els.find((e) => e.id === copies[0])!
  assert.deepEqual([ct.x, ct.y], [24, 24])
  assert.notEqual(ct.groupId, g)
  assert.equal(els.find((e) => e.id === copies[1])!.groupId, ct.groupId)
  assert.match(textOf(d, s, copies[0]!)!.toString(), /Hello/)
  ;(textOf(d, s, t)!.get(0) as Y.XmlElement).delete(0, 1)
  assert.match(textOf(d, s, copies[0]!)!.toString(), /Hello/)
  const other = createSlide(d)!
  assert.equal(duplicateElements(d, s, [r], other, 0, 0).length, 1)
})

test('duplicating a slide copies elements with new ids', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  const e = addElement(d, s, { type: 'text', x: 0, y: 0, w: 10, h: 10, text: 'Hi' })!
  const copy = duplicateSlide(d, s)!
  const ce = readSlide(d, copy)!.elements[0]!
  assert.notEqual(ce.id, e)
  assert.match(textOf(d, copy, ce.id)!.toString(), /Hi/)
})

test('outline lists the text of each slide for search and export', () => {
  const d = fresh()
  const s = slideIds(d)[0]!
  addElement(d, s, { type: 'text', x: 0, y: 0, w: 10, h: 10, text: 'Quarterly <review> & plans' })
  addElement(d, s, { ...rect(), alt: 'blue box' })
  const o = createSlide(d)!
  addElement(d, o, { type: 'text', x: 0, y: 0, w: 10, h: 10, text: 'Second' })
  assert.equal(outline(d), '## 1\nQuarterly <review> & plans\nblue box\n\n## 2\nSecond')
})

test('undo only reverts the local person changes', () => {
  const a = fresh()
  const s = slideIds(a)[0]!
  const b = new Y.Doc()
  sync(a, b)
  const undo = createUndo(a)
  const mine = addElement(a, s, rect())!
  sync(a, b)
  const theirs = addElement(b, s, rect(5, 5))!
  Y.applyUpdate(a, Y.encodeStateAsUpdate(b), 'remote')
  assert.equal(readSlide(a, s)!.elements.length, 2)
  undo.undo()
  assert.deepEqual(readSlide(a, s)!.elements.map((e) => e.id), [theirs])
  undo.redo()
  assert.ok(readSlide(a, s)!.elements.some((e) => e.id === mine))
  assert.equal(LOCAL, 'looty-slides-local')
})
