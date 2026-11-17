// node --test frontend/tests/treeMove.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { computeMove, effectiveWhere, lastRootPosition, type TreeEntry } from '../app/composables/treeMove.ts'

const all: TreeEntry[] = [
  { id: 'a', parent_id: null, position: 100 },
  { id: 'b', parent_id: 'a', position: 100 },
  { id: 's', parent_id: 'a', position: 200, study: true },
  { id: 'c', parent_id: null, position: 300 },
]

test('un entraînement déposé « dedans » se range après', () => {
  assert.equal(effectiveWhere(all[2]!, 'inside'), 'after')
  assert.equal(effectiveWhere(all[0]!, 'inside'), 'inside')
})

test('déposer un entraînement sous une page', () => {
  assert.deepEqual(computeMove(all, 's', 'c', 'inside'), { parentId: 'c', position: 65536 })
})

test('avant / après entre frères, entraînements compris', () => {
  const m = computeMove(all, 'c', 's', 'before')
  assert.equal(m?.parentId, 'a')
  assert.ok(m!.position > 100 && m!.position < 200)
  const after = computeMove(all, 'c', 's', 'inside')
  assert.equal(after?.parentId, 'a')
  assert.ok(after!.position > 200)
})

test('cycle et soi-même interdits', () => {
  assert.equal(computeMove(all, 'a', 'b', 'inside'), null)
  assert.equal(computeMove(all, 'a', 'a', 'before'), null)
})

test('racine : dernière position', () => {
  assert.equal(lastRootPosition(all), 300 + 65536)
})
