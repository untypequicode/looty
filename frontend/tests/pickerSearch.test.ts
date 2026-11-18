// node --test frontend/tests/pickerSearch.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { filterHits, hitPlace, groupByKind, flatten, pushRecent, parseRecents, emptyQuerySections, type PickerHit } from '../app/utils/pickerSearch.ts'

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

const e = (kind: string, id = kind) => ({ kind, id })

test('regroupement par type dans l’ordre voulu', () => {
  const sections = groupByKind([e('file', 'f1'), e('task', 't1'), e('page', 'p1'), e('page', 'p2'), e('study', 's1'), e('project', 'x')])
  assert.deepEqual(sections.map((s) => s.id), ['page', 'study', 'task', 'file', 'project'])
  assert.deepEqual(sections[0]!.items.map((i) => i.id), ['p1', 'p2'])
  assert.deepEqual(flatten(sections).map((i) => i.id), ['p1', 'p2', 's1', 't1', 'f1', 'x'])
  assert.deepEqual(groupByKind([e('page', 'a'), e('file', 'b')], 'file').map((s) => s.id), ['file'])
  assert.deepEqual(groupByKind([], 'all'), [])
})

test('récents : le plus récent d’abord, sans doublon, plafonné', () => {
  let list = [] as ReturnType<typeof pushRecent>
  for (let i = 0; i < 10; i++) list = pushRecent(list, { kind: 'page', id: `p${i}`, ws: 'w', title: `P${i}` })
  assert.equal(list.length, 8)
  assert.equal(list[0]!.id, 'p9')
  list = pushRecent(list, { kind: 'page', id: 'p5', ws: 'w', title: 'P5' })
  assert.equal(list[0]!.id, 'p5')
  assert.equal(list.filter((r) => r.id === 'p5').length, 1)
  // Même identifiant mais autre type : deux éléments distincts.
  assert.equal(pushRecent(list, { kind: 'study', id: 'p5', ws: 'w', title: 'S' }).filter((r) => r.id === 'p5').length, 2)
})

test('lecture tolérante des récents enregistrés', () => {
  assert.deepEqual(parseRecents(null), [])
  assert.deepEqual(parseRecents('pas du json'), [])
  assert.deepEqual(parseRecents('{"a":1}'), [])
  assert.deepEqual(parseRecents(JSON.stringify([{ kind: 'page', id: 'a', ws: 'w', title: 't' }, { kind: 3 }, null])).map((r) => r.id), ['a'])
})

test('recherche vide : récents puis favoris, sans doublon, sections vides omises', () => {
  const recents = [e('page', 'a'), e('study', 'b')]
  const favs = [e('page', 'a'), e('page', 'c')]
  const s = emptyQuerySections(recents, favs)
  assert.deepEqual(s.map((x) => x.id), ['recent', 'favorite'])
  assert.deepEqual(s[1]!.items.map((i) => i.id), ['c'])
  assert.deepEqual(emptyQuerySections([], favs).map((x) => x.id), ['favorite'])
  assert.deepEqual(emptyQuerySections([], []), [])
})
