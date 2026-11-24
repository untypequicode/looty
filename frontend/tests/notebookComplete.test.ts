// node --test frontend/tests/notebookComplete.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { toCodePoints, fromCodePoints, fromCompleteReply, fromJedi } from '../app/editor/notebookComplete.ts'

test('positions convert between UTF-16 and code points', () => {
  const code = 'a😀b'
  assert.equal(code.length, 4)
  assert.equal(toCodePoints(code, 4), 3)
  assert.equal(toCodePoints(code, 1), 1)
  assert.equal(fromCodePoints(code, 3), 4)
  assert.equal(fromCodePoints(code, 2), 3)
  assert.equal(fromCodePoints('abc', 99), 3)
})

test('complete_reply becomes completions, with types and UTF-16 positions', () => {
  const code = '😀 = 1\nnp.ar'
  const r = fromCompleteReply(code, {
    status: 'ok', matches: ['np.array', 'np.arange', 'np.array'], cursor_start: 6, cursor_end: 11,
    metadata: { _jupyter_types_experimental: [{ text: 'np.array', type: 'function' }, { text: 'np.arange', type: 'class' }] },
  })!
  assert.equal(code.slice(r.from, r.to), 'np.ar')
  assert.deepEqual(r.options, [{ label: 'np.array', type: 'function' }, { label: 'np.arange', type: 'class' }])
})

test('failed or empty replies give nothing', () => {
  assert.equal(fromCompleteReply('x', { status: 'error' }), null)
  assert.equal(fromCompleteReply('x', { status: 'ok', matches: [], cursor_start: 0, cursor_end: 1 }), null)
  assert.equal(fromCompleteReply('x', { status: 'ok', matches: ['a'], cursor_start: 5, cursor_end: 1 }), null)
  assert.equal(fromJedi('x', 1, null), null)
})

test('Jedi replies replace the already typed prefix', () => {
  const r = fromJedi('import os\nos.pa', 15, { prefix: 2, items: [{ label: 'path', type: 'module' }, { label: 'pardir', type: 'statement' }] })!
  assert.equal(r.from, 13)
  assert.equal(r.to, 15)
  assert.deepEqual(r.options.map((o) => o.type), ['namespace', 'variable'])
})
