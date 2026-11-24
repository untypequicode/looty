// node --test frontend/tests/notebookToolbar.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { markdownEdit, type MdPlaceholders } from '../app/editor/notebookToolbar.ts'

const ph: MdPlaceholders = { text: 'texte', link: 'lien', alt: 'image', url: 'adresse', path: 'images/fichier.png', formula: 'x' }
const apply = (doc: string, e: { from: number; to: number; insert: string }) => doc.slice(0, e.from) + e.insert + doc.slice(e.to)
const run = (action: Parameters<typeof markdownEdit>[0], doc: string, from: number, to = from) => {
  const e = markdownEdit(action, doc, from, to, ph)
  const out = apply(doc, e)
  return { out, picked: out.slice(e.anchor, e.head) }
}

test('bold wraps the selection, or a placeholder, and toggles back', () => {
  assert.deepEqual(run('bold', 'un mot ici', 3, 6), { out: 'un **mot** ici', picked: 'mot' })
  assert.deepEqual(run('bold', '', 0), { out: '**texte**', picked: 'texte' })
  assert.deepEqual(run('bold', 'un **mot** ici', 5, 8), { out: 'un mot ici', picked: 'mot' })
  assert.equal(run('bold', '**mot**', 0, 7).out, 'mot')
})

test('italic is not fooled by bold markers', () => {
  assert.equal(run('italic', 'un mot', 3, 6).out, 'un *mot*')
  assert.equal(run('italic', 'un *mot*', 4, 7).out, 'un mot')
  assert.equal(run('italic', 'un **mot**', 5, 8).out, 'un ***mot***')
})

test('math is inline on one line and a block on several', () => {
  assert.equal(run('math', 'a b', 0, 1).out, '$a$ b')
  assert.deepEqual(run('math', '', 0), { out: '$x$', picked: 'x' })
  assert.equal(run('math', 'a\nb', 0, 3).out, '$$\na\nb\n$$')
})

test('link and image select the part to fill in', () => {
  assert.deepEqual(run('link', 'voir ici', 5, 8), { out: 'voir [ici](adresse)', picked: 'adresse' })
  assert.deepEqual(run('link', 'https://exemple.org', 0, 19), { out: '[lien](https://exemple.org)', picked: 'lien' })
  assert.deepEqual(run('image', '', 0), { out: '![image](images/fichier.png)', picked: 'images/fichier.png' })
  assert.deepEqual(run('image', 'schéma', 0, 6), { out: '![schéma](images/fichier.png)', picked: 'images/fichier.png' })
})

test('list prefixes every line of the selection, and removes the prefix when all have it', () => {
  assert.equal(run('list', 'a\nb\nc', 0, 3).out, '- a\n- b\nc')
  assert.equal(run('list', 'a\n\nb', 0, 4).out, '- a\n\n- b')
  assert.equal(run('list', '- a\n- b', 0, 7).out, 'a\nb')
  assert.equal(run('list', 'x', 0).out, '- x')
  assert.equal(run('list', '', 0).out, '- ')
})
