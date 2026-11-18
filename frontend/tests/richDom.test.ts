// node --test frontend/tests/richDom.test.ts
// Rendu riche avec un vrai DOM (jsdom) : la configuration de DOMPurify, `sanitizeSvg` et le filtrage des liens
// face à de vraies charges XSS. Le DOM est posé avant l'import des modules (DOMPurify se lie à `window` à l'import).
import test from 'node:test'
import assert from 'node:assert/strict'
import { JSDOM } from 'jsdom'

const dom = new JSDOM('<!doctype html><html><body></body></html>', { url: 'https://looty.test/' })
const g = globalThis as any
for (const k of ['window', 'document', 'DOMParser', 'Node', 'Element', 'HTMLElement', 'DocumentFragment', 'HTMLTemplateElement', 'NodeFilter', 'NamedNodeMap', 'HTMLFormElement', 'MutationObserver']) {
  if (!(k in g) || k === 'window' || k === 'document') Object.defineProperty(g, k, { value: k === 'window' ? dom.window : (dom.window as any)[k], configurable: true, writable: true })
}

const { renderChatMarkdown } = await import('../app/editor/chatMarkdown.ts')
const { sanitizeSvg } = await import('../app/editor/sanitizeSvg.ts')

const parse = (html: string) => new JSDOM(`<body>${html}</body>`).window.document.body
const everyAttr = (root: Element) => [...root.querySelectorAll('*')].flatMap((el) => [...el.attributes].map((a) => ({ tag: el.tagName.toLowerCase(), name: a.name.toLowerCase(), value: a.value })))

test('chat markdown: raw script and event-handler HTML never become live elements', () => {
  const payloads = [
    '<script>alert(1)</script>',
    '<img src=x onerror=alert(1)>',
    '<svg onload=alert(1)>',
    '<iframe src="javascript:alert(1)"></iframe>',
    '<a href="javascript:alert(1)" onclick="alert(1)">x</a>',
    '<style>@import url(http://evil.test/x.css)</style>',
    '<form action="http://evil.test"><input name=a><button>go</button></form>',
    '<div style="background:url(http://evil.test/p.png)" onmouseover="alert(1)">x</div>',
  ]
  for (const p of payloads) {
    const out = parse(renderChatMarkdown(p))
    assert.equal(out.querySelector('script, img, svg, iframe, style, form, input, button'), null, p)
    assert.deepEqual(everyAttr(out).filter((a) => a.name.startsWith('on') || a.name === 'style'), [], p)
    assert.doesNotMatch(out.innerHTML, /<(script|img|svg|iframe|style|form|input|button)/i, p)
  }
})

test('chat markdown: only safe link targets survive, and kept links open safely', () => {
  const bad = ['javascript:alert(1)', 'JaVaScRiPt:alert(1)', ' javascript:alert(1)', 'java\tscript:alert(1)', 'data:text/html;base64,PHNjcmlwdD4=', 'vbscript:msgbox(1)', '//evil.test/x']
  for (const href of bad) {
    const out = parse(renderChatMarkdown(`[clic](${href.replace(/ /g, '%20').replace(/\t/g, '%09')}) et [clic2](<${href}>)`))
    const hrefs = [...out.querySelectorAll('a')].map((a) => a.getAttribute('href') ?? '')
    assert.deepEqual(hrefs.filter((h) => /^\s*(javascript|data|vbscript):|^\/\//i.test(h.replace(/[\u0000- ]/g, ''))), [], href)
  }
  const ok = parse(renderChatMarkdown('[a](https://example.org/x) [b](mailto:a@b.c) [c](#top) [d](/w/1)'))
  const links = [...ok.querySelectorAll('a')]
  assert.equal(links.length, 4)
  for (const a of links) {
    assert.equal(a.getAttribute('target'), '_blank')
    assert.match(a.getAttribute('rel') ?? '', /noopener/)
  }
})

test('chat markdown: markdown images are text, never <img>', () => {
  const out = parse(renderChatMarkdown('![alt](https://evil.test/track.png) ![x](javascript:alert(1))'))
  assert.equal(out.querySelector('img'), null)
})

test('chat markdown: formulas render through KaTeX without trust (no \\href / \\includegraphics)', () => {
  const out = parse(renderChatMarkdown('$\\href{javascript:alert(1)}{x}$ et $x^2$'))
  assert.equal(out.querySelector('a[href^="javascript"]'), null)
  assert.ok(out.querySelector('.katex'))
})

test('sanitizeSvg: scripts, handlers, embedded content and links are removed', () => {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)" viewBox="0 0 10 10">
    <script>alert(2)</script>
    <a href="javascript:alert(3)"><text onclick="alert(4)">x</text></a>
    <foreignObject><iframe src="javascript:alert(5)"></iframe><div onmouseover="alert(6)">ok</div><form><input></form></foreignObject>
    <use href="javascript:alert(7)"/><image href="javascript:alert(8)" onerror="alert(9)"/>
    <rect width="5" height="5" fill="red"/>
  </svg>`
  const out = parse(sanitizeSvg(svg))
  assert.equal(out.querySelector('script, iframe, object, embed, form, input, a'), null)
  const attrs = everyAttr(out)
  assert.deepEqual(attrs.filter((a) => a.name.startsWith('on')), [])
  assert.deepEqual(attrs.filter((a) => /javascript:/i.test(a.value)), [])
  assert.ok(out.querySelector('rect'), 'harmless shapes are kept')
})

test('sanitizeSvg: CSS cannot load anything external (style tags and style attributes)', () => {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg">
    <style>@import url(http://evil.test/x.css); .a { fill: url(https://evil.test/a.svg#p); stroke: red }</style>
    <rect class="a" style="background:url(http://evil.test/p.png); fill: blue" width="5" height="5"/>
  </svg>`
  const html = sanitizeSvg(svg)
  assert.doesNotMatch(html, /evil\.test/)
  assert.doesNotMatch(html, /@import/i)
  assert.match(html, /stroke:\s*red/, 'the rest of the CSS stays')
  assert.match(html, /fill:\s*blue/)
})
