import { test } from 'node:test'
import assert from 'node:assert/strict'
import * as Y from 'yjs'
import { getSchema } from '@tiptap/core'
import Document from '@tiptap/extension-document'
import Paragraph from '@tiptap/extension-paragraph'
import Text from '@tiptap/extension-text'
import Heading from '@tiptap/extension-heading'
import { yXmlFragmentToProsemirrorJSON } from '@tiptap/y-tiptap'
import { seedYDoc } from '../app/utils/seedDoc.ts'

const schema = getSchema([Document, Paragraph, Text, Heading])
const pm = schema.nodeFromJSON({
  type: 'doc',
  content: [
    { type: 'heading', attrs: { level: 1 }, content: [{ type: 'text', text: 'Titre' }] },
    { type: 'paragraph', content: [{ type: 'text', text: 'Bonjour' }] },
  ],
})
// État vide du serveur (collaboration/index.js EMPTY_DOC)
const EMPTY = Buffer.from('0101ecc4b5ab070007010764656661756c74030970617261677261706800', 'hex')
const fresh = () => { const d = new Y.Doc(); Y.applyUpdate(d, EMPTY); return d }
const json = (d: Y.Doc) => JSON.stringify(yXmlFragmentToProsemirrorJSON(d.getXmlFragment('default')))
const sync = (a: Y.Doc, b: Y.Doc) => {
  Y.applyUpdate(b, Y.encodeStateAsUpdate(a, Y.encodeStateVector(b)))
  Y.applyUpdate(a, Y.encodeStateAsUpdate(b, Y.encodeStateVector(a)))
}

test('state de départ : un paragraphe vide', () => {
  assert.equal(fresh().getXmlFragment('default').length, 1)
})

test('deux clients amorcent en même temps : contenu une seule fois', () => {
  const a = fresh(), b = fresh()
  assert.ok(seedYDoc(a, pm)); assert.ok(seedYDoc(b, pm))
  sync(a, b)
  assert.equal(json(a), json(b))
  const content = (yXmlFragmentToProsemirrorJSON(a.getXmlFragment('default')) as any).content
  assert.deepEqual(content.map((n: any) => n.type), ['heading', 'paragraph'])
  assert.equal(a.getXmlFragment('default').length, 2)
  assert.deepEqual(Y.encodeStateVector(a), Y.encodeStateVector(b))
})

test('amorcer deux fois le même document : sans effet', () => {
  const a = fresh()
  seedYDoc(a, pm)
  const before = json(a), sv = Y.encodeStateVector(a)
  assert.equal(seedYDoc(a, pm), false)
  assert.equal(json(a), before)
  assert.deepEqual(Y.encodeStateVector(a), sv)
})

test('fusion dans un seul sens après amorçage séparé', () => {
  const a = fresh(), b = fresh()
  seedYDoc(a, pm); seedYDoc(b, pm)
  Y.applyUpdate(a, Y.encodeStateAsUpdate(b))
  assert.equal(a.getXmlFragment('default').length, 2)
})
