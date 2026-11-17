// node --test frontend/tests/richMarkdown.test.ts
import test from 'node:test'
import assert from 'node:assert/strict'
import { parseChatMarkdown, renderChatMarkdown, renderKatex, safeHref } from '../app/editor/chatMarkdown.ts'
import { injectMath } from '../app/editor/chatMathParse.ts'

const html = (s: string, inline = false) => parseChatMarkdown(s, { inline }).html
const full = (s: string, inline = false) => {
  const r = parseChatMarkdown(s, { inline })
  return injectMath(r.html, r.parts, renderKatex)
}

test('inline mode has no paragraph wrapper, block mode does', () => {
  assert.equal(html('Quel est $x$ ?', true).includes('<p>'), false)
  assert.ok(html('Quel est $x$ ?').startsWith('<p>'))
  assert.ok(html('a **b**', true).includes('<strong>b</strong>'))
})

test('math renders through KaTeX, inline and display', () => {
  assert.match(full('Coût $O(n^2)$'), /class="katex"/)
  assert.doesNotMatch(full('Coût $O(n^2)$'), /\$O/)
  assert.match(full('$$\\sum_i i$$'), /katex-display/)
  assert.match(full('```math\nx^2\n```'), /katex-display/)
  assert.match(full('\\(x\\)', true), /class="katex"/)
})

test('unclosed formula, fence or callout stays text while streaming', () => {
  assert.equal(html('a $x^2').includes('katex'), false)
  assert.match(html('a $x^2'), /\$x\^2/)
  assert.equal(html('```mermaid\ngraph TD\nA-->B').includes('rich-diagram'), false)
  assert.equal(html('```looty-graph\ns -> t').includes('rich-diagram'), false)
  assert.equal(html(':::info\nhello').includes('callout'), false)
  assert.equal(full('$$x').includes('katex'), false)
})

test('closed mermaid and looty-graph fences become diagram slots with escaped source', () => {
  const m = html('```mermaid\ngraph TD\nA-->B\n```')
  assert.match(m, /<div class="rich-diagram" data-kind="mermaid"><pre><code>graph TD\nA--&gt;B<\/code><\/pre><\/div>/)
  const g = html('```looty-graph\ns -> t : 3/5\n```')
  assert.match(g, /data-kind="graph"/)
  assert.match(g, /s -&gt; t : 3\/5/)
  // Une clôture plus longue ou en tildes ferme aussi.
  assert.match(html('~~~mermaid\nA\n~~~'), /rich-diagram/)
})

test('other fences stay code, with a language class only', () => {
  assert.match(html('```python\nprint(1)\n```'), /<pre><code class="language-python">print\(1\)/)
  assert.match(html('```py"onload=x\nA\n```'), /^<pre><code class="language-pyonloadx">/)
  assert.doesNotMatch(html('```py"><script>\nA\n```'), /<script/)
})

test('callouts: four kinds, markdown inside, text after', () => {
  for (const k of ['info', 'success', 'warning', 'tip']) assert.match(html(`:::${k}\nhello **x**\n:::\n\nafter`), new RegExp(`<div class="callout callout-${k}"><p>hello <strong>x</strong></p>`))
  assert.match(html(':::info\n:::'), /callout-info/)
  assert.doesNotMatch(html(':::danger\nx\n:::'), /callout/)
  assert.match(html(':::info\nhello\n:::\n\nafter'), /<p>after<\/p>/)
})

test('task lists are read-only boxes, never form inputs', () => {
  const h = html('- [x] fait\n- [ ] à faire')
  assert.equal(h.includes('<input'), false)
  assert.equal((h.match(/class="rich-check"/g) ?? []).length, 2)
  assert.match(h, /aria-checked="true"/)
  assert.match(h, /aria-checked="false"/)
})

test('tables render', () => {
  assert.match(html('| a | b |\n| - | - |\n| 1 | 2 |'), /<table>[\s\S]*<th>a<\/th>[\s\S]*<td>2<\/td>/)
})

test('raw HTML, images and dangerous links never survive as markup', () => {
  const payloads = ['<script>alert(1)</script>', '<img src=x onerror=alert(1)>', '<iframe src="javascript:alert(1)"></iframe>', '<svg onload=alert(1)>', '<a href="javascript:alert(1)">x</a>']
  for (const p of payloads) {
    const h = html(p + '\n\ntexte') + html(p, true)
    assert.equal(/<(script|img|iframe|svg)\b/i.test(h), false, p)
    assert.equal(/<a href="javascript/i.test(h), false, p)
  }
  assert.equal(/<img/.test(html('![x](http://a/b.png)')), false)
  const link = html('[x](javascript:alert(1)) [y](JaVaScRiPt:alert(1)) [z](&#106;avascript:alert(1)) [ok](https://a.b/c)')
  assert.doesNotMatch(link, /javascript/i)
  assert.match(link, /<a href="https:\/\/a\.b\/c">ok<\/a>/)
})

test('safeHref allows web, mail, anchors and relative paths only', () => {
  for (const ok of ['https://a.b', 'http://a.b', 'mailto:a@b.c', '#x', '/w/1', './x', '../x', 'page']) assert.equal(safeHref(ok), true, ok)
  for (const ko of ['javascript:alert(1)', ' JAVASCRIPT:alert(1)', 'java\tscript:alert(1)', 'data:text/html,x', 'vbscript:x', '//evil.example']) assert.equal(safeHref(ko), false, ko)
})

test('formula source is escaped, never executed', () => {
  const r = full('$\\href{javascript:alert(1)}{x}$')
  assert.equal(/<script|onerror=|javascript:alert/.test(r) && /<a /.test(r), false)
})

test('without a DOM the output is escaped text', () => {
  assert.equal(renderChatMarkdown('<b>x</b> $y$'), '&lt;b&gt;x&lt;/b&gt; $y$')
})
