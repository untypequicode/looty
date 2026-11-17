// node --test frontend/tests/conversationGroups.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { bucketOf, groupConversations } from '../app/utils/conversationGroups.ts'

// Midi, heure locale : le test ne dépend pas du fuseau.
const now = new Date(2026, 10, 17, 12, 0, 0)
const at = (dayOffset: number, h = 9) => new Date(2026, 10, 17 + dayOffset, h, 0, 0).toISOString()

test('aujourd’hui, cette semaine, plus ancien', () => {
  assert.equal(bucketOf(at(0, 0), now), 'today')
  assert.equal(bucketOf(at(-1, 23), now), 'week')
  assert.equal(bucketOf(at(-6, 0), now), 'week')
  assert.equal(bucketOf(at(-7, 23), now), 'older')
  assert.equal(bucketOf(at(-40), now), 'older')
})

test('regroupe en gardant l’ordre et omet les rubriques vides', () => {
  const list = [{ id: 'a', updated_at: at(-30) }, { id: 'b', updated_at: at(0) }, { id: 'c', updated_at: at(-30, 10) }]
  const g = groupConversations(list, now)
  assert.deepEqual(g.map((x) => x.key), ['today', 'older'])
  assert.deepEqual(g[1]!.items.map((x) => x.id), ['a', 'c'])
  assert.deepEqual(groupConversations([], now), [])
})
