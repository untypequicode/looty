/**
 * Export des documents en HTML autonome et en PDF (impression du navigateur ou de
 * l'application de bureau). Les formules sont rendues avec KaTeX (préambule de l'espace et
 * macros définies dans le document comprises), les mentions de page deviennent des liens.
 */
import katex from 'katex'
import katexCssUrl from 'katex/dist/katex.min.css?url'
import { scopeBuilder, ensureMhchem } from './latex'
import { workspacePreamble } from './mathScope'

const KATEX_CDN = 'https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css'

const escapeHtml = (s: string) => s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!)

/** Contenu d'une page prêt à l'export : formules rendues, liens internes absolus ou réécrits. */
export async function renderForExport(html: string, opts: { link?: (href: string) => string } = {}): Promise<string> {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html')
  const maths = Array.from(doc.querySelectorAll<HTMLElement>('[data-type="math-inline"], [data-type="math-block"]'))
  if (maths.some((el) => /\\ce\b|\\pu\b/.test(el.getAttribute('latex') ?? ''))) await ensureMhchem().catch(() => {})
  const base = scopeBuilder().next(workspacePreamble.value)
  const builder = scopeBuilder(base.macros)
  for (const el of maths) {
    const latex = el.getAttribute('latex') ?? ''
    const display = el.dataset.type === 'math-block'
    const scope = builder.next(latex)
    const src = scope.parsed.rest
    if (!src) { el.remove(); continue }
    try {
      el.innerHTML = katex.renderToString(src, { displayMode: display, throwOnError: false, trust: false, macros: { ...scope.macros }, maxExpand: 500 })
    } catch {
      el.textContent = display ? `$$${latex}$$` : `$${latex}$`
    }
    el.removeAttribute('latex')
    el.classList.add(display ? 'math-block' : 'math-inline')
  }
  // Blocs dépliables : ouverts dans le document exporté.
  for (const d of Array.from(doc.querySelectorAll('details'))) d.setAttribute('open', '')
  for (const a of Array.from(doc.querySelectorAll<HTMLAnchorElement>('a[href]'))) {
    const href = a.getAttribute('href')!
    a.setAttribute('href', opts.link ? opts.link(href) : new URL(href, location.origin).href)
  }
  for (const img of Array.from(doc.querySelectorAll<HTMLImageElement>('img[src]'))) {
    if (!opts.link) img.setAttribute('src', new URL(img.getAttribute('src')!, location.origin).href)
  }
  return doc.body.innerHTML
}

/** Feuille de style des documents exportés (écran et impression). */
export const EXPORT_CSS = `
:root { color-scheme: light; }
body { font: 16px/1.65 Inter, system-ui, -apple-system, "Segoe UI", sans-serif; color: #1b1b1f; max-width: 760px; margin: 40px auto; padding: 0 24px; }
h1.doc-title { font-size: 2.2em; letter-spacing: -.02em; margin: 0 0 .6em; line-height: 1.2; }
h1, h2, h3, h4 { line-height: 1.3; margin: 1.4em 0 .5em; }
p { margin: .6em 0; }
a { color: #3b5bdb; }
a[data-type="page-mention"] { text-decoration: none; background: #eef1fd; padding: 0 4px; border-radius: 4px; }
img { max-width: 100%; height: auto; border-radius: 6px; }
pre { background: #f5f5f7; padding: 12px 14px; border-radius: 8px; overflow-x: auto; font-size: .88em; }
code { font-family: ui-monospace, "JetBrains Mono", monospace; font-size: .9em; background: #f2f2f5; padding: 1px 4px; border-radius: 4px; }
pre code { background: none; padding: 0; }
blockquote { margin: 1em 0; padding: 2px 16px; border-left: 3px solid #d0d0d8; color: #4a4a55; }
table { border-collapse: collapse; width: 100%; margin: 1em 0; }
td, th { border: 1px solid #dcdce2; padding: 6px 10px; vertical-align: top; text-align: left; }
th { background: #f5f5f7; }
ul[data-type="taskList"] { list-style: none; padding-left: 4px; }
ul[data-type="taskList"] li { display: flex; gap: 8px; }
ul[data-type="taskList"] input { margin-top: .45em; }
[data-type="notice"] { padding: 10px 14px; border: none; border-radius: 8px; background: #eef4ff; color: inherit; margin: 1em 0; }
[data-type="notice"].callout-warning { background: #fff6e5; }
[data-type="notice"].callout-success { background: #eaf8ef; }
[data-type="notice"].callout-tip { background: #f4efff; }
details { border-left: 2px solid #e2e2e8; padding-left: 12px; margin: .8em 0; }
summary { font-weight: 600; cursor: pointer; }
.math-block { display: block; text-align: center; margin: 1em 0; overflow-x: auto; }
mark { background: #fff3a3; }
hr { border: none; border-top: 1px solid #e2e2e8; margin: 2em 0; }
.page-break { break-after: page; }
@media print {
  body { margin: 0; max-width: none; padding: 0; font-size: 11.5pt; }
  a { color: inherit; }
  pre, blockquote, table, img, .math-block { break-inside: avoid; }
  h1, h2, h3 { break-after: avoid; }
}
`

/** Document HTML autonome. `katexHref` : feuille KaTeX (CDN pour un fichier exporté). */
export function htmlDocument(title: string, body: string, katexHref = KATEX_CDN): string {
  return `<!doctype html>
<html lang="${document.documentElement.lang || 'fr'}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="Looty">
<title>${escapeHtml(title)}</title>
<link rel="stylesheet" href="${katexHref}">
<style>${EXPORT_CSS}</style>
</head>
<body>
${body}
</body>
</html>
`
}

/**
 * PDF : impression dans un cadre caché (boîte de dialogue « Enregistrer en PDF » du
 * navigateur ; dans l'application de bureau, « Imprimer dans un fichier »).
 */
export async function printToPdf(title: string, sections: { title: string; html: string }[]) {
  const body = (await Promise.all(sections.map(async (s, i) =>
    `<article${i < sections.length - 1 ? ' class="page-break"' : ''}><h1 class="doc-title">${escapeHtml(s.title)}</h1>${await renderForExport(s.html)}</article>`))).join('\n')
  const frame = document.createElement('iframe')
  frame.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0;visibility:hidden'
  document.body.appendChild(frame)
  const doc = frame.contentDocument!
  // KaTeX servi par l'application (polices comprises), même hors ligne.
  doc.open()
  doc.write(htmlDocument(title, body, new URL(katexCssUrl, location.origin).href))
  doc.close()
  await new Promise<void>((resolve) => {
    const images = Array.from(doc.images)
    let pending = images.length + 1
    const done = () => { if (--pending <= 0) resolve() }
    images.forEach((img) => (img.complete ? done() : img.addEventListener('load', done, { once: true }) || img.addEventListener('error', done, { once: true })))
    const css = doc.querySelector('link[rel=stylesheet]') as HTMLLinkElement | null
    if (css && !css.sheet) css.addEventListener('load', done, { once: true }), css.addEventListener('error', done, { once: true })
    else done()
    setTimeout(resolve, 4000)
  })
  await doc.fonts?.ready
  frame.contentWindow!.focus()
  frame.contentWindow!.print()
  setTimeout(() => frame.remove(), 60_000)
}
