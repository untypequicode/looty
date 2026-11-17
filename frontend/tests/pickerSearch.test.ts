// node --test frontend/tests/pickerSearch.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { filterHits, hitPlace, type PickerHit } from '../app/utils/pickerSearch.ts'

const hit = (kind: string, type: string, extra: Partial<PickerHit['r']> = {}): PickerHit => ({ kind, ws: 'w', wsName: 'Espace', r: { type, title: 't', ...extra } })

test('filtre par type et limite', () => {
  const hits = [hit('page', 'page'), hit('task', 'task'), hit('page', 'page')]
  assert.equal(filterHits(hits, 'all').length, 3)
  assert.equal(filterHits(hits, 'page').length, 2)
  assert.equal(filterHits(hits, 'all', 2).length, 2)
  assert.equal(filterHits(hits, 'event').length, 0)
})

test('chemin complet d’une page', () => {
  assert.equal(hitPlace(hit('page', 'page', { path: ['Cours', 'Algèbre'] })), 'Espace › Cours › Algèbre')
  assert.equal(hitPlace(hit('page', 'page')), 'Espace')
})

test('tableau d’une tâche et calendrier d’un événement', () => {
  assert.equal(hitPlace(hit('task', 'task', { path: ['Sprint'] })), 'Espace › Sprint')
  const fmt = () => '1 janv.'
  assert.equal(hitPlace(hit('event', 'event', { path: ['Perso'], date: '2026-01-01T10:00:00Z' }), { formatDate: fmt }), 'Espace › Perso · 1 janv.')
})

test('espace omis avec un seul espace', () => {
  assert.equal(hitPlace(hit('page', 'page', { path: ['A'] }), { multiWorkspace: false }), 'A')
  assert.equal(hitPlace(hit('page', 'page'), { multiWorkspace: false }), '')
})
