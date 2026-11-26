// node --test frontend/tests/shareAccess.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { docIdOfPath, groupShared, levelRank, moveKind, needsManageWarning, outsiders, pathLabel, sectionsOf, sortMoveChanges, type AccessPerson, type SharedItem } from '../app/utils/shareAccess.ts'

const person = (name: string, over: Partial<AccessPerson> = {}): AccessPerson => ({ user_id: name, name, email: `${name}@x.org`, level: 'read', origin: 'direct', is_member: false, inherit: true, ...over })
const item = (title: string, over: Partial<SharedItem> = {}): SharedItem => ({ id: title, workspace_id: 'w1', workspace_name: 'Team', title, updated_at: '2026-01-01T00:00:00Z', level: 'read', root: true, origin: 'direct', path: [], path_hidden: false, ...over })

test('seule la gestion demande une confirmation', () => {
  assert.equal(needsManageWarning('manage'), true)
  for (const l of ['read', 'comment', 'edit', 'deny']) assert.equal(needsManageWarning(l), false)
})

test('les niveaux sont ordonnés et le refus ne donne rien', () => {
  assert.ok(levelRank('read') < levelRank('comment') && levelRank('comment') < levelRank('edit') && levelRank('edit') < levelRank('manage'))
  assert.equal(levelRank('deny'), 0)
  assert.equal(levelRank('inconnu'), 0)
})

test('les lignes se rangent par origine et les extérieurs se comptent sans les refus', () => {
  const ppl = [person('a'), person('b', { origin: 'group' }), person('c', { origin: 'inherited' }), person('d', { origin: 'space', is_member: true }), person('e', { level: 'deny' })]
  const s = sectionsOf(ppl)
  assert.deepEqual([s.direct.map((p) => p.name), s.inherited.map((p) => p.name), s.space.map((p) => p.name)], [['a', 'b', 'e'], ['c'], ['d']])
  assert.deepEqual(outsiders(ppl).map((p) => p.name), ['a', 'b', 'c'])
})

test('un déplacement : accès perdus d’abord, puis gagnés, puis niveaux changés', () => {
  const c = [
    { user_id: '1', name: 'Zoé', before: 'edit', after: 'read' },
    { user_id: '2', name: 'Ana', before: 'none', after: 'comment' },
    { user_id: '3', name: 'Bob', before: 'comment', after: 'none' },
    { user_id: '4', name: 'Abel', before: 'edit', after: 'none' },
  ]
  assert.deepEqual(c.map(moveKind), ['changed', 'gained', 'lost', 'lost'])
  assert.deepEqual(sortMoveChanges(c).map((x) => x.name), ['Abel', 'Bob', 'Ana', 'Zoé'])
})

test('Partagé avec moi : groupé par espace, récent d’abord, filtré par recherche', () => {
  const items = [
    item('Vieux', { updated_at: '2026-01-01T00:00:00Z' }),
    item('Récent', { updated_at: '2026-03-01T00:00:00Z', shared_by: 'Éloïse' }),
    item('Autre', { workspace_id: 'w2', workspace_name: 'Club', updated_at: '2026-02-01T00:00:00Z' }),
  ]
  const g = groupShared(items)
  assert.deepEqual(g.map((x) => [x.name, x.items.map((i) => i.title)]), [['Team', ['Récent', 'Vieux']], ['Club', ['Autre']]])
  assert.deepEqual(groupShared(items, 'eloise').map((x) => x.items.map((i) => i.title)), [['Récent']])
  assert.deepEqual(groupShared(items, 'club').map((x) => x.name), ['Club'])
  assert.deepEqual(groupShared(items, 'zzz'), [])
})

test('le chemin ne montre que les parents visibles et signale les autres', () => {
  assert.equal(pathLabel({ path: [], path_hidden: false }), '')
  assert.equal(pathLabel({ path: [{ id: 'a', title: 'Projet' }], path_hidden: true }), '… / Projet')
  assert.equal(pathLabel({ path: [{ id: 'a', title: 'A' }, { id: 'b', title: 'B' }], path_hidden: false }), 'A / B')
})

test('l’identifiant d’une page se lit dans son chemin', () => {
  const id = '3f2b8c1e-1111-4222-8333-444455556666'
  assert.equal(docIdOfPath(`/w/abc/doc/${id}`), id)
  assert.equal(docIdOfPath(`/w/abc/doc/${id}?comment=1`), id)
  assert.equal(docIdOfPath('/w/abc/docs'), null)
  assert.equal(docIdOfPath('/w/abc/board/x'), null)
})
