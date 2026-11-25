// node --test frontend/tests/slidesRender.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import * as Y from 'yjs'
import { fragmentHtml, safeHref, slideColor } from '../app/slides/render.ts'
import { addElement, initDoc, slideIds, textOf } from '../app/slides/model.ts'

test('colors resolve palette keys, hex values and fall back for anything else', () => {
  assert.equal(slideColor('blue', 'x'), 'var(--pal-blue)')
  assert.equal(slideColor('#12ab9F', 'x'), '#12ab9F')
  assert.equal(slideColor('none', 'x'), 'transparent')
  assert.equal(slideColor('default', 'fb'), 'fb')
  assert.equal(slideColor('url(javascript:1)', 'fb'), 'fb')
  assert.equal(slideColor(undefined, 'fb'), 'fb')
})

test('links keep http, mailto and relative targets only', () => {
  assert.equal(safeHref('https://a.org'), 'https://a.org')
  assert.equal(safeHref('/doc/1'), '/doc/1')
  assert.equal(safeHref('javascript:alert(1)'), null)
  assert.equal(safeHref(3), null)
})

test('rich text becomes escaped html with marks, headings and lists', () => {
  const d = new Y.Doc()
  initDoc(d)
  const s = slideIds(d)[0]!
  const id = addElement(d, s, { type: 'text', x: 0, y: 0, w: 10, h: 10, text: 'a <b> & "c"\n' })!
  const f = textOf(d, s, id)!
  assert.equal(fragmentHtml(f), '<p>a &lt;b&gt; &amp; &quot;c&quot;</p><p><br></p>')
  const h = new Y.XmlElement('heading')
  h.setAttribute('level', '2')
  const t = new Y.XmlText()
  f.push([h])
  h.push([t])
  t.insert(0, 'Title', { bold: {}, link: { href: 'javascript:x' } })
  assert.match(fragmentHtml(f), /<h2><strong>Title<\/strong><\/h2>$/)
  const ul = new Y.XmlElement('bulletList')
  const li = new Y.XmlElement('listItem')
  const p = new Y.XmlElement('paragraph')
  f.push([ul])
  ul.push([li])
  li.push([p])
  const x = new Y.XmlText()
  p.push([x])
  x.insert(0, 'item', { italic: {}, link: { href: 'https://ok.org' } })
  assert.match(fragmentHtml(f), /<ul><li><p><a href="https:\/\/ok.org" rel="noopener noreferrer nofollow"><em>item<\/em><\/a><\/p><\/li><\/ul>$/)
})
