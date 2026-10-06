/**
 * Import / export compatible Outline (https://getoutline.com).
 *
 * Deux formats d'archive sont pris en charge, dans les deux sens :
 * - JSON : `metadata.json` + un `<Collection>.json` par collection (documents en
 *   ProseMirror au schéma Outline, arborescence `documentStructure`), pièces
 *   jointes stockées dans l'archive à leur `key` ;
 * - Markdown : un dossier par collection, sous-documents dans un dossier portant
 *   le titre du parent, pièces jointes référencées par chemin relatif.
 * Les fichiers .md / .html isolés sont aussi acceptés à l'import.
 */
import JSZip from 'jszip'
import { Marked, type TokenizerAndRendererExtension } from 'marked'
import { generateHTML, generateJSON, type JSONContent } from '@tiptap/core'
import type { Page } from '~/types'
import { api, uploadFile } from '~/composables/api'
import { links } from '~/composables/useWorkspace'
import { readonlyExtensions } from './extensions'
import { htmlToMarkdown } from './export'

type PM = JSONContent
type UrlMap = (url: string) => string

let _exts: ReturnType<typeof readonlyExtensions> | null = null
const exts = () => (_exts ??= readonlyExtensions())

const uuid = () => crypto.randomUUID()
const isEmoji = (s?: string | null) => !!s && /\p{Extended_Pictographic}/u.test(s)
/** Émoji en tête de titre (« 🚀 Feuille de route ») : séquences ZWJ, variantes, tons, drapeaux, touches. */
const LEADING_EMOJI = /^\s*((?:\p{Extended_Pictographic}(?:\uFE0F|\p{Emoji_Modifier})*(?:\u200D\p{Extended_Pictographic}(?:\uFE0F|\p{Emoji_Modifier})*)*)|\p{Regional_Indicator}{2}|[#*0-9]\uFE0F?\u20E3)\s*/u
export function splitTitleEmoji(title: string): { emoji: string | null; title: string } {
  const m = LEADING_EMOJI.exec(title)
  const rest = m ? title.slice(m[0].length).trim() : ''
  return m && rest ? { emoji: m[1]!, title: rest } : { emoji: null, title }
}
const safeName = (s: string) => (s || 'Untitled').replace(/[/\\?%*:|"<>\u0000-\u001f]/g, '-').trim().slice(0, 120) || 'Untitled'
const slugify = (s: string) => (s || 'untitled').normalize('NFKD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 60) || 'untitled'
const urlIdOf = (id: string) => id.replace(/-/g, '').slice(0, 10)
const textOf = (n?: PM): string => (n?.text ?? '') + (n?.content ?? []).map(textOf).join('')
const MERMAID = ['mermaid', 'mermaidjs']

// ---------------------------------------------------------------------------
// Conversion de schéma ProseMirror : Looty <-> Outline
// ---------------------------------------------------------------------------

const MARKS_TO_OUTLINE: Record<string, string> = { bold: 'strong', italic: 'em', underline: 'underline', strike: 'strikethrough', code: 'code_inline', link: 'link', highlight: 'highlight' }
const MARKS_FROM_OUTLINE: Record<string, string> = { ...Object.fromEntries(Object.entries(MARKS_TO_OUTLINE).map(([a, b]) => [b, a])), strike: 'strike', bold: 'bold', italic: 'italic', code: 'code' }

function mapMarks(marks: PM['marks'], table: Record<string, string>, url: UrlMap) {
  const out = (marks ?? []).flatMap((m) => {
    const type = table[m.type]
    if (!type) return []
    return [type === 'link' ? { type, attrs: { href: url(m.attrs?.href ?? '') } } : { type }]
  })
  return out.length ? out : undefined
}

const text = (t: string, marks?: PM['marks']): PM => ({ type: 'text', text: t, ...(marks ? { marks } : {}) })
const para = (content?: PM[]): PM => ({ type: 'paragraph', ...(content?.length ? { content } : {}) })

/** Document Looty (JSON TipTap) -> document Outline. */
export function toOutline(node: PM, url: UrlMap = (u) => u): PM {
  const kids = () => (node.content ?? []).flatMap((c) => { const r = toOutline(c, url); return r ? [r] : [] })
  const a = node.attrs ?? {}
  switch (node.type) {
    case 'text': return text(node.text ?? '', mapMarks(node.marks, MARKS_TO_OUTLINE, url))
    case 'doc': return { type: 'doc', content: kids() }
    case 'paragraph': case 'blockquote': case 'table': return { type: node.type, content: kids() }
    case 'heading': return { type: 'heading', attrs: { level: a.level ?? 1 }, content: kids() }
    case 'bulletList': return { type: 'bullet_list', content: kids() }
    case 'orderedList': return { type: 'ordered_list', attrs: { order: a.start ?? 1 }, content: kids() }
    case 'listItem': return { type: 'list_item', content: kids() }
    case 'taskList': return { type: 'checkbox_list', content: kids() }
    case 'taskItem': return { type: 'checkbox_item', attrs: { checked: !!a.checked }, content: kids() }
    case 'codeBlock': return { type: 'code_block', attrs: { language: a.language || 'none' }, content: textOf(node) ? [text(textOf(node))] : [] }
    case 'mermaid': return { type: 'code_block', attrs: { language: 'mermaidjs' }, content: a.code ? [text(a.code)] : [] }
    case 'excalidraw': return { type: 'code_block', attrs: { language: 'excalidraw' }, content: a.scene ? [text(a.scene)] : [] }
    case 'horizontalRule': return { type: 'horizontal_rule' }
    case 'hardBreak': return { type: 'br' }
    case 'image': return { type: 'image', attrs: { src: url(a.src ?? ''), alt: a.alt ?? null, layoutClass: a.align === 'left' ? 'left-50' : a.align === 'right' ? 'right-50' : null } }
    case 'tableRow': return { type: 'tr', content: kids() }
    case 'tableHeader': case 'tableCell':
      return { type: node.type === 'tableHeader' ? 'th' : 'td', attrs: { colspan: a.colspan ?? 1, rowspan: a.rowspan ?? 1, colwidth: a.colwidth ?? null, alignment: null }, content: kids() }
    case 'notice': return { type: 'container_notice', attrs: { style: a.type ?? 'info' }, content: kids() }
    case 'toggle': {
      const [summary, body] = node.content ?? []
      return { type: 'container_toggle', attrs: { id: uuid() }, content: [para((summary?.content ?? []).map((c) => toOutline(c, url))), ...(body?.content ?? []).map((c) => toOutline(c, url))] }
    }
    case 'mathBlock': return { type: 'math_block', content: a.latex ? [text(a.latex)] : [] }
    case 'mathInline': return { type: 'math_inline', content: a.latex ? [text(a.latex)] : [] }
    case 'mention': return text(`@${a.label ?? ''}`)
    case 'embed': return { type: 'embed', attrs: { href: a.src } }
    case 'pdfEmbed': return { type: 'attachment', attrs: { id: uuid(), href: url(a.src ?? ''), title: a.name || 'PDF', size: 0, contentType: 'application/pdf' } }
    default: return node.content ? { type: 'paragraph', content: kids() } : (null as unknown as PM)
  }
}

/** Document Outline -> document Looty (JSON TipTap). Tolère les variantes de noms. */
export function fromOutline(node: PM, url: UrlMap = (u) => u): PM[] {
  const kids = () => (node.content ?? []).flatMap((c) => fromOutline(c, url))
  const blocks = () => { const k = kids(); return k.length ? k : [para()] }
  const a = node.attrs ?? {}
  switch (node.type) {
    case 'text': return node.text ? [text(node.text, mapMarks(node.marks, MARKS_FROM_OUTLINE, url))] : []
    case 'doc': return [{ type: 'doc', content: blocks() }]
    case 'paragraph': return [para(kids())]
    case 'blockquote': return [{ type: 'blockquote', content: blocks() }]
    case 'heading': return [{ type: 'heading', attrs: { level: Math.min(Math.max(Number(a.level) || 1, 1), 4) }, content: kids() }]
    case 'bullet_list': case 'bulletList': return [{ type: 'bulletList', content: kids() }]
    case 'ordered_list': case 'orderedList': return [{ type: 'orderedList', attrs: { start: a.order ?? a.start ?? 1 }, content: kids() }]
    case 'list_item': case 'listItem': return [{ type: 'listItem', content: blocks() }]
    case 'checkbox_list': case 'taskList': return [{ type: 'taskList', content: kids() }]
    case 'checkbox_item': case 'taskItem': return [{ type: 'taskItem', attrs: { checked: !!a.checked }, content: blocks() }]
    case 'code_block': case 'code_fence': case 'codeBlock': {
      const lang = String(a.language ?? '').toLowerCase()
      const code = textOf(node)
      if (MERMAID.includes(lang)) return [{ type: 'mermaid', attrs: { code } }]
      if (lang === 'excalidraw') return [{ type: 'excalidraw', attrs: { scene: code } }]
      if (lang === 'math' || lang === 'latex') return [{ type: 'mathBlock', attrs: { latex: code } }]
      return [{ type: 'codeBlock', attrs: { language: lang && lang !== 'none' ? lang : null }, content: code ? [text(code)] : [] }]
    }
    case 'horizontal_rule': case 'hr': return [{ type: 'horizontalRule' }]
    case 'br': case 'hard_break': return [{ type: 'hardBreak' }]
    case 'image': {
      const align = String(a.layoutClass ?? '').startsWith('left') ? 'left' : String(a.layoutClass ?? '').startsWith('right') ? 'right' : 'center'
      return a.src ? [{ type: 'image', attrs: { src: url(a.src), alt: a.alt ?? null, align } }] : []
    }
    case 'table': return [{ type: 'table', content: kids() }]
    case 'tr': return [{ type: 'tableRow', content: kids() }]
    case 'th': case 'td': return [{ type: node.type === 'th' ? 'tableHeader' : 'tableCell', attrs: { colspan: a.colspan ?? 1, rowspan: a.rowspan ?? 1, colwidth: a.colwidth ?? null }, content: blocks() }]
    case 'container_notice': case 'notice': {
      const style = ['info', 'success', 'warning', 'tip'].includes(a.style) ? a.style : 'info'
      return [{ type: 'notice', attrs: { type: style }, content: blocks() }]
    }
    case 'container_toggle': {
      const [head, ...rest] = node.content ?? []
      const summary = head && ['paragraph', 'heading'].includes(head.type!) ? fromOutline(head, url)[0]?.content ?? [] : []
      const body = (head && summary.length ? rest : node.content ?? []).flatMap((c) => fromOutline(c, url))
      return [{ type: 'toggle', attrs: { open: true }, content: [{ type: 'toggleSummary', ...(summary.length ? { content: summary } : {}) }, { type: 'toggleContent', content: body.length ? body : [para()] }] }]
    }
    case 'math_block': return [{ type: 'mathBlock', attrs: { latex: textOf(node) } }]
    case 'math_inline': return [{ type: 'mathInline', attrs: { latex: textOf(node) } }]
    case 'mention': return [text(`@${a.label ?? ''}`)]
    case 'emoji': return [text(a['data-name'] ? `:${a['data-name']}:` : textOf(node))]
    case 'embed': return a.href ? [{ type: 'embed', attrs: { src: a.href } }] : []
    case 'attachment': case 'video': {
      const href = url(a.href ?? a.src ?? '')
      if (!href) return []
      if (a.contentType === 'application/pdf' || /\.pdf($|\?)/i.test(href)) return [{ type: 'pdfEmbed', attrs: { src: href, name: a.title ?? '' } }]
      return [para([text(a.title || href, [{ type: 'link', attrs: { href } }])])]
    }
    default: return kids()
  }
}

const docToHtml = (doc: PM) => generateHTML(doc, exts())
const htmlToDoc = (html: string): PM => generateJSON(html || '<p></p>', exts())

/** Applique `fn` à toutes les URL d'un HTML (images, liens, PDF, embeds Looty). */
async function rewriteHtmlUrls(html: string, fn: (url: string) => Promise<string>): Promise<string> {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html')
  for (const [sel, attr] of [['img[src]', 'src'], ['a[href]', 'href'], ['[data-src]', 'data-src']] as const) {
    for (const el of Array.from(doc.querySelectorAll(sel))) el.setAttribute(attr, await fn(el.getAttribute(attr)!))
  }
  return doc.body.innerHTML
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

export type ExportFormat = 'json' | 'markdown' | 'html'
export interface ExportOptions {
  wsId: string
  /** Nom de la collection Outline (nom de l'espace ou titre de la page racine). */
  name: string
  /** Pages à exporter ; les pages dont le parent est absent deviennent des racines. */
  pages: Page[]
  format: ExportFormat
  user: { id: string; name: string; email?: string | null }
  /** Contenu HTML déjà connu (ex. page ouverte dans l'éditeur). */
  overrides?: Record<string, string>
  onProgress?: (done: number, total: number) => void
}

interface NavNode { id: string; url: string; title: string; icon?: string | null; children: NavNode[] }

/** Construit l'archive : JSON au format Outline, Markdown ou HTML (arborescence de fichiers). */
export async function buildExport(o: ExportOptions): Promise<Blob> {
  const zip = new JSZip()
  const ids = new Set(o.pages.map((p) => p.id))
  const byParent = new Map<string | null, Page[]>()
  for (const p of o.pages) {
    const key = p.parent_id && ids.has(p.parent_id) ? p.parent_id : null
    byParent.set(key, [...(byParent.get(key) ?? []), p])
  }
  for (const l of byParent.values()) l.sort((a, b) => a.position - b.position)
  const docUrl = (p: Page) => `/doc/${slugify(p.title)}-${urlIdOf(p.id)}`

  // Contenu HTML de chaque page
  const html = new Map<string, string>()
  let done = 0
  for (const p of o.pages) {
    html.set(p.id, o.overrides?.[p.id] ?? (await api<Page>(`/api/workspaces/${o.wsId}/pages/${p.id}`)).content ?? '')
    o.onProgress?.(++done, o.pages.length * 2)
  }

  // Pièces jointes : chaque fichier /uploads/ référencé est inclus une seule fois.
  const attachments = new Map<string, { id: string; documentId: string; contentType: string; name: string; size: number; key: string }>()
  async function attach(src: string, documentId: string) {
    const path = src.replace(/^https?:\/\/[^/]+/, '').split(/[?#]/)[0] ?? ''
    if (!path.startsWith('/uploads/')) return null
    if (!attachments.has(path)) {
      const res = await fetch(path, { credentials: 'include' })
      if (!res.ok) return null
      const blob = await res.blob()
      const id = uuid()
      const name = decodeURIComponent(path.split('/').pop()!)
      const key = `uploads/${o.user.id}/${id}/${name}`
      zip.file(key, blob)
      attachments.set(path, { id, documentId, contentType: blob.type || 'application/octet-stream', name, size: blob.size, key })
    }
    return attachments.get(path)!
  }
  const internal = new RegExp(`^(?:https?://[^/]+)?/w/[^/]+/doc/([0-9a-f-]{36})(#.*)?$`, 'i')

  if (o.format === 'json') {
    const documents: Record<string, any> = {}
    for (const p of o.pages) {
      const prepared = await rewriteHtmlUrls(html.get(p.id)!, async (u) => {
        const m = u.match(internal)
        if (m) { const target = o.pages.find((x) => x.id === m[1]); return target ? docUrl(target) + (m[2] ?? '') : new URL(u, location.origin).href }
        const att = await attach(u, p.id)
        return att ? `/api/attachments.redirect?id=${att.id}` : u
      })
      const parent = p.parent_id && ids.has(p.parent_id) ? p.parent_id : null
      documents[p.id] = {
        id: p.id, urlId: urlIdOf(p.id), title: p.title || 'Untitled',
        icon: p.icon ?? null, emoji: isEmoji(p.icon) ? p.icon : null, color: null,
        data: toOutline(htmlToDoc(prepared)),
        createdById: o.user.id, createdByName: o.user.name, createdByEmail: o.user.email ?? null,
        createdAt: p.created_at, updatedAt: p.updated_at, publishedAt: p.updated_at ?? p.created_at,
        fullWidth: false, template: false, parentDocumentId: parent,
      }
      o.onProgress?.(++done, o.pages.length * 2)
    }
    const nav = (parent: string | null): NavNode[] =>
      (byParent.get(parent) ?? []).map((p) => ({ id: p.id, url: docUrl(p), title: p.title || 'Untitled', icon: p.icon ?? null, children: nav(p.id) }))
    const collectionId = uuid()
    zip.file('metadata.json', JSON.stringify({ exportVersion: 1, version: 'looty', createdAt: new Date().toISOString(), createdById: o.user.id, createdByEmail: o.user.email ?? null }, null, 2))
    zip.file(`${safeName(o.name)}.json`, JSON.stringify({
      collection: { id: collectionId, urlId: urlIdOf(collectionId), name: o.name, description: '', permission: null, color: '#4E5C6E', icon: null, sort: { field: 'index', direction: 'asc' }, documentStructure: nav(null) },
      documents,
      attachments: Object.fromEntries([...attachments.values()].map((a) => [a.id, a])),
    }, null, 2))
  } else {
    // Chemins des fichiers : <Collection>/<Titre>.md, enfants dans <Collection>/<Titre>/
    const paths = new Map<string, string>()
    const assign = (parent: string | null, dir: string) => {
      const used = new Set<string>()
      for (const p of byParent.get(parent) ?? []) {
        let base = safeName(p.title)
        for (let i = 1; used.has(base.toLowerCase()); i++) base = `${safeName(p.title)} (${i})`
        used.add(base.toLowerCase())
        paths.set(p.id, `${dir}${base}`)
        assign(p.id, `${dir}${base}/`)
      }
    }
    assign(null, `${safeName(o.name)}/`)
    const rel = (fromFile: string, to: string) => {
      const from = fromFile.split('/').slice(0, -1)
      const target = to.split('/')
      let i = 0
      while (i < from.length && from[i] === target[i]) i++
      return encodeURI([...from.slice(i).map(() => '..'), ...target.slice(i)].join('/'))
    }
    const ext = o.format === 'html' ? 'html' : 'md'
    const { renderForExport, htmlDocument } = o.format === 'html' ? await import('./exportDoc') : ({} as typeof import('./exportDoc'))
    for (const p of o.pages) {
      const file = `${paths.get(p.id)}.${ext}`
      const prepared = await rewriteHtmlUrls(html.get(p.id)!, async (u) => {
        const m = u.match(internal)
        if (m) return paths.has(m[1]!) ? rel(file, `${paths.get(m[1]!)}.${ext}`) + (m[2] ?? '') : new URL(u, location.origin).href
        const att = await attach(u, p.id)
        return att ? rel(file, att.key) : u
      })
      if (o.format === 'html') {
        const title = p.title || 'Untitled'
        const body = await renderForExport(prepared, { link: (h) => h })
        zip.file(file, htmlDocument(title, `<h1 class="doc-title">${title.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]!)}</h1>\n${body}`))
      } else {
        zip.file(file, `# ${p.title || 'Untitled'}\n\n${htmlToMarkdown(prepared)}\n`)
      }
      o.onProgress?.(++done, o.pages.length * 2)
    }
  }
  return zip.generateAsync({ type: 'blob', compression: 'DEFLATE' })
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

/** Nœud de l'arbre à importer, indépendant du format d'origine. */
export interface ImportNode {
  key: string
  title: string
  icon?: string | null
  /** Collection Outline ou dossier sans document associé. */
  container?: boolean
  /** Produit le HTML Looty ; `ctx` résout les liens internes et téléverse les pièces jointes. */
  render?: (ctx: RenderCtx) => Promise<string>
  children: ImportNode[]
}
interface RenderCtx { pageUrl: (key: string) => string | null; upload: (zipPath: string) => Promise<string | null> }

export interface ImportPlan {
  format: 'outline-json' | 'markdown' | 'html'
  roots: ImportNode[]
  documents: number
  collections: number
}

const decode = (s: string) => { try { return decodeURIComponent(s) } catch { return s } }
const normPath = (path: string) => {
  const out: string[] = []
  for (const part of path.split('/')) {
    if (!part || part === '.') continue
    if (part === '..') out.pop()
    else out.push(part)
  }
  return out.join('/')
}
async function uploadDataUri(u: string) {
  try {
    const blob = await (await fetch(u)).blob()
    const ext = (blob.type.split('/')[1] || 'bin').replace(/\W.*$/, '')
    return (await uploadFile(new File([blob], `image.${ext}`, { type: blob.type }))).url
  } catch { return null }
}
const isLocal = (u: string) => !!u && !/^([a-z][a-z0-9+.-]*:|\/\/|#)/i.test(u) && !u.startsWith('/')

// --- Markdown -> HTML Looty ---------------------------------------------------

const mathBlock: TokenizerAndRendererExtension = {
  name: 'mathBlock', level: 'block',
  start: (src) => src.match(/\$\$/)?.index,
  tokenizer(src) { const m = /^\$\$([\s\S]+?)\$\$[^\S\n]*(?:\n|$)/.exec(src); if (m) return { type: 'mathBlock', raw: m[0], text: m[1]!.trim() } },
  renderer: (t) => `<div data-type="math-block" latex="${escapeAttr(t.text)}"></div>`,
}
const mathInline: TokenizerAndRendererExtension = {
  name: 'mathInline', level: 'inline',
  start: (src) => src.match(/\$/)?.index,
  tokenizer(src) { const m = /^\$(?!\s)([^$\n]+?)(?<!\s)\$(?!\d)/.exec(src); if (m) return { type: 'mathInline', raw: m[0], text: m[1] } },
  renderer: (t) => `<span data-type="math-inline" latex="${escapeAttr(t.text)}"></span>`,
}
const highlight: TokenizerAndRendererExtension = {
  name: 'highlight', level: 'inline',
  start: (src) => src.match(/==/)?.index,
  tokenizer(src) { const m = /^==([^=\n]+)==/.exec(src); if (m) return { type: 'highlight', raw: m[0], tokens: this.lexer.inlineTokens(m[1]!) } },
  renderer(t) { return `<mark>${this.parser.parseInline(t.tokens!)}</mark>` },
}
const notice: TokenizerAndRendererExtension = {
  name: 'notice', level: 'block',
  start: (src) => src.match(/^:::/m)?.index,
  tokenizer(src) {
    const m = /^:::[^\S\n]*(info|success|warning|tip|notice)?[^\n]*\n([\s\S]*?)\n:::[^\S\n]*(?:\n|$)/.exec(src)
    if (m) return { type: 'notice', raw: m[0], style: m[1] === 'notice' || !m[1] ? 'info' : m[1], tokens: this.lexer.blockTokens(m[2]!, []) }
  },
  renderer(t) { return `<blockquote data-type="notice" data-notice-type="${t.style}">${this.parser.parse(t.tokens!)}</blockquote>` },
}
function escapeAttr(s: string) { return s.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;') }

const marked = new Marked({ gfm: true, breaks: false })
marked.use({
  extensions: [mathBlock, mathInline, highlight, notice],
  renderer: {
    code({ text: code, lang }) {
      const l = (lang ?? '').trim().toLowerCase()
      if (MERMAID.includes(l)) return `<div data-type="mermaid"><pre>${escapeAttr(code)}</pre></div>`
      if (l === 'math' || l === 'latex') return `<div data-type="math-block" latex="${escapeAttr(code)}"></div>`
      if (l === 'excalidraw') return `<div data-type="excalidraw" data-scene="${escapeAttr(code)}"></div>`
      return false
    },
  },
})

/** Normalise du HTML (Markdown rendu ou export HTML) vers le balisage Looty. */
function normalizeHtml(html: string): string {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html')
  // Listes de tâches GFM / Outline
  for (const input of Array.from(doc.querySelectorAll('li > input[type=checkbox], li > p > input[type=checkbox]'))) {
    const li = input.closest('li')!
    li.setAttribute('data-type', 'taskItem')
    li.setAttribute('data-checked', String((input as HTMLInputElement).hasAttribute('checked')))
    li.parentElement?.setAttribute('data-type', 'taskList')
    input.remove()
  }
  // <details> -> toggle Looty
  for (const det of Array.from(doc.querySelectorAll('details:not([data-type])'))) {
    const summary = det.querySelector(':scope > summary')
    const body = doc.createElement('div')
    body.setAttribute('data-type', 'toggle-content')
    for (const c of Array.from(det.childNodes)) if (c !== summary) body.appendChild(c)
    if (!body.textContent?.trim() && !body.querySelector('img,div')) body.innerHTML = '<p></p>'
    det.setAttribute('data-type', 'toggle')
    det.setAttribute('open', '')
    if (!summary) det.prepend(doc.createElement('summary'))
    det.appendChild(body)
  }
  // Images : taille Outline « =LxH » dans le titre
  for (const img of Array.from(doc.querySelectorAll('img[title^="="]'))) img.removeAttribute('title')
  // Paragraphes vides d'Outline (« \ » seul)
  for (const p of Array.from(doc.querySelectorAll('p'))) if (p.textContent === '\\') p.innerHTML = ''
  return doc.body.innerHTML
}

/** Markdown -> HTML Looty ; renvoie aussi le titre si le document commence par « # Titre ». */
export function markdownToHtml(md: string): { title: string | null; html: string } {
  let title: string | null = null
  const src = md.replace(/^﻿/, '').replace(/\r\n?/g, '\n').replace(/^\s*#\s+(.+?)\s*#*\s*(\n|$)/, (_m, t) => { title = t.trim(); return '' })
  return { title, html: normalizeHtml(marked.parse(src, { async: false }) as string) }
}

function htmlFileToHtml(src: string): { title: string | null; html: string } {
  const doc = new DOMParser().parseFromString(src, 'text/html')
  const h1 = doc.body.querySelector('h1')
  let title = doc.title?.trim() || null
  if (h1 && doc.body.firstElementChild === h1) { title ||= h1.textContent?.trim() || null; h1.remove() }
  return { title, html: normalizeHtml(doc.body.innerHTML) }
}

// --- Lecture des archives -----------------------------------------------------

interface Source { files: Map<string, () => Promise<Blob>>; text: (path: string) => Promise<string> }

async function readSources(files: File[]): Promise<Source> {
  const map = new Map<string, () => Promise<Blob>>()
  for (const f of files) {
    if (/\.zip$/i.test(f.name)) {
      const zip = await JSZip.loadAsync(f)
      zip.forEach((path, entry) => {
        if (entry.dir || path.startsWith('__MACOSX/') || /(^|\/)\.DS_Store$/.test(path)) return
        map.set(normPath(path), () => entry.async('blob'))
      })
    } else {
      map.set(normPath((f as any).webkitRelativePath || f.name), async () => f)
    }
  }
  return { files: map, text: async (p) => (await map.get(p)!()).text() }
}

/** Analyse les fichiers choisis et prépare l'arbre à importer (aucune écriture). */
export async function planImport(files: File[]): Promise<ImportPlan> {
  const src = await readSources(files)
  const paths = [...src.files.keys()]

  // Export JSON Outline : <Collection>.json contenant { collection, documents }
  const jsonCollections: any[] = []
  for (const p of paths.filter((x) => /\.json$/i.test(x) && !/(^|\/)metadata\.json$/i.test(x))) {
    try {
      const data = JSON.parse(await src.text(p))
      if (data?.collection && data?.documents) jsonCollections.push({ data, dir: p.split('/').slice(0, -1).join('/') })
    } catch { /* fichier JSON étranger : ignoré */ }
  }
  if (jsonCollections.length) return planOutlineJson(jsonCollections, src)
  return planMarkdown(src, paths)
}

function planOutlineJson(collections: { data: any; dir: string }[], src: Source): ImportPlan {
  let documents = 0
  const roots: ImportNode[] = collections.map(({ data, dir }) => {
    const docs: Record<string, any> = data.documents ?? {}
    const atts: Record<string, any> = data.attachments ?? {}
    const byKey = new Map(Object.values(atts).map((a: any) => [normPath(a.key ?? ''), a]))
    const resolveAttachment = async (u: string, ctx: RenderCtx) => {
      const m = u.match(/attachments\.redirect\?id=([0-9a-f-]{36})/i)
      const att = m ? atts[m[1]!] : byKey.get(normPath(decode(u.replace(/^\/+/, ''))))
      if (!att?.key) return null
      const key = normPath(att.key)
      return (await ctx.upload(src.files.has(key) ? key : normPath(`${dir}/${key}`)))
    }
    const docLink = (u: string, ctx: RenderCtx) => {
      const m = u.match(/\/doc\/(?:[^/?#]*-)?([A-Za-z0-9]{10})(?:[/?][^#]*)?(#.*)?$/)
      const target = m && Object.values(docs).find((d: any) => d.urlId === m[1])
      if (!target) return null
      const url = ctx.pageUrl(target.id)
      return url ? url + (m![2] ?? '') : null
    }
    const node = (d: any, seen: Set<string>): ImportNode => {
      seen.add(d.id)
      documents++
      return {
        key: d.id,
        title: d.title || 'Untitled',
        icon: isEmoji(d.icon) ? d.icon : isEmoji(d.emoji) ? d.emoji : null,
        render: async (ctx) => {
          // Pré-résolution asynchrone des URL, puis conversion synchrone.
          const urls = new Map<string, string>()
          const collect = (n: PM) => {
            for (const v of [n.attrs?.src, n.attrs?.href, ...(n.marks ?? []).map((mk) => mk.attrs?.href)]) if (typeof v === 'string') urls.set(v, v)
            n.content?.forEach(collect)
          }
          if (d.data) collect(d.data)
          for (const u of urls.keys()) urls.set(u, docLink(u, ctx) ?? (await resolveAttachment(u, ctx)) ?? u)
          if (d.data) return docToHtml(fromOutline(d.data, (u) => urls.get(u) ?? u)[0]!)
          // Anciennes versions : Markdown dans `text`
          return rewriteHtmlUrls(markdownToHtml(d.text ?? '').html, async (u) => docLink(u, ctx) ?? (await resolveAttachment(u, ctx)) ?? u)
        },
        children: [],
      }
    }
    // Arborescence : documentStructure (ordre), complétée par parentDocumentId.
    const seen = new Set<string>()
    const walk = (navs: any[]): ImportNode[] => navs.filter((n) => docs[n.id] && !seen.has(n.id)).map((n) => { const x = node(docs[n.id], seen); x.children = walk(n.children ?? []); return x })
    const top = walk(data.collection?.documentStructure ?? [])
    const all = new Map<string, ImportNode>()
    const index = (l: ImportNode[]) => l.forEach((n) => { all.set(n.key, n); index(n.children) })
    index(top)
    const rest = Object.values(docs).filter((d: any) => !seen.has(d.id)).map((d: any) => ({ d, n: node(d, seen) }))
    for (const { n } of rest) all.set(n.key, n)
    for (const { d, n } of rest) ((d.parentDocumentId !== d.id && all.get(d.parentDocumentId)?.children) || top).push(n)

    const c = data.collection ?? {}
    return {
      key: `collection:${c.id ?? uuid()}`,
      title: c.name || 'Collection',
      icon: isEmoji(c.icon) ? c.icon : null,
      container: true,
      render: async () => (c.data ? docToHtml(fromOutline(c.data)[0]!) : c.description ? markdownToHtml(c.description).html : ''),
      children: top,
    }
  })
  return { format: 'outline-json', roots, documents, collections: roots.length }
}

function planMarkdown(src: Source, paths: string[]): ImportPlan {
  const docs = paths.filter((p) => /\.(md|markdown|html?)$/i.test(p)).sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
  const nodes = new Map<string, ImportNode>()
  const roots: ImportNode[] = []
  const stem = (p: string) => p.replace(/\.(md|markdown|html?)$/i, '')
  const ensureFolder = (dir: string): ImportNode | null => {
    if (!dir) return null
    if (nodes.has(dir)) return nodes.get(dir)!
    const n: ImportNode = { key: dir, title: dir.split('/').pop()!, container: true, children: [] }
    nodes.set(dir, n)
    ;(ensureFolder(dir.split('/').slice(0, -1).join('/'))?.children ?? roots).push(n)
    return n
  }
  // Documents d'abord, pour qu'un dossier « Titre/ » se rattache au document « Titre.md ».
  for (const p of docs) {
    const key = stem(p)
    const isHtml = /\.html?$/i.test(p)
    const node: ImportNode = {
      key,
      title: key.split('/').pop()!,
      children: [],
      render: async (ctx) => {
        const raw = await src.text(p)
        const parsed = isHtml ? htmlFileToHtml(raw) : markdownToHtml(raw)
        const dir = p.split('/').slice(0, -1).join('/')
        return rewriteHtmlUrls(parsed.html, async (u) => {
          if (u.startsWith('data:')) return (await uploadDataUri(u)) ?? u
          if (!isLocal(u)) return u
          const [path, hash] = decode(u).split('#')
          const target = normPath(`${dir}/${path}`)
          if (/\.(md|markdown|html?)$/i.test(target)) return ctx.pageUrl(stem(target)) ? ctx.pageUrl(stem(target))! + (hash ? `#${hash}` : '') : u
          return (await ctx.upload(target)) ?? (await ctx.upload(normPath(path ?? ''))) ?? u
        })
      },
    }
    nodes.set(key, node)
  }
  for (const p of docs) {
    const key = stem(p)
    ;(ensureFolder(key.split('/').slice(0, -1).join('/'))?.children ?? roots).push(nodes.get(key)!)
  }
  return { format: docs.some((p) => /\.html?$/i.test(p)) && !docs.some((p) => /\.(md|markdown)$/i.test(p)) ? 'html' : 'markdown', roots, documents: docs.length, collections: roots.filter((r) => r.container).length }
}

export interface ImportOptions {
  wsId: string
  parentId: string | null
  /** Crée une page par collection / dossier racine ; sinon leurs documents sont importés directement. */
  containerPages: boolean
  onProgress?: (done: number, total: number) => void
}

/** Crée les pages (arborescence d'abord, contenu ensuite pour résoudre les liens internes). */
export async function runImport(files: File[], plan: ImportPlan, o: ImportOptions): Promise<Page[]> {
  const src = await readSources(files)
  const created = new Map<string, Page>()
  const order: { node: ImportNode; page: Page }[] = []
  // Seules les collections / dossiers racines peuvent être omis ; les dossiers imbriqués restent des pages.
  const flatten = (l: ImportNode[]): ImportNode[] => l.flatMap((n) => (n.container && !o.containerPages ? n.children : [n]))
  let total = 0
  const count = (l: ImportNode[]) => l.forEach((n) => { total++; count(n.children) })
  const top = flatten(plan.roots)
  count(top)
  total *= 2
  let done = 0

  // Titre réel des documents Markdown / HTML (première ligne « # Titre »).
  async function titleOf(n: ImportNode) {
    if (plan.format === 'outline-json' || n.container) return n.title
    const path = [...src.files.keys()].find((p) => p.replace(/\.(md|markdown|html?)$/i, '') === n.key)
    if (!path) return n.title
    const raw = await src.text(path)
    return (/\.html?$/i.test(path) ? htmlFileToHtml(raw) : markdownToHtml(raw)).title || n.title
  }
  async function create(l: ImportNode[], parentId: string | null) {
    for (const n of l) {
      // L'émoji en tête de titre devient l'icône de la page.
      const { emoji, title } = splitTitleEmoji(await titleOf(n))
      const page = await api<Page>(`/api/workspaces/${o.wsId}/pages`, { method: 'POST', body: { title: title.slice(0, 255), icon: n.icon ?? emoji ?? undefined, parent_id: parentId } })
      created.set(n.key, page)
      order.push({ node: n, page })
      o.onProgress?.(++done, total)
      await create(n.children, page.id)
    }
  }
  await create(top, o.parentId)

  const uploads = new Map<string, Promise<string | null>>()
  const ctx: RenderCtx = {
    pageUrl: (key) => { const p = created.get(key); return p ? links.doc(o.wsId, p.id) : null },
    upload: (path) => {
      if (!src.files.has(path)) return Promise.resolve(null)
      if (!uploads.has(path)) {
        uploads.set(path, (async () => {
          try {
            const blob = await src.files.get(path)!()
            return (await uploadFile(new File([blob], path.split('/').pop()!, { type: blob.type }))).url
          } catch { return null }
        })())
      }
      return uploads.get(path)!
    },
  }
  for (const { node, page } of order) {
    const content = node.render ? await node.render(ctx) : ''
    if (content.replace(/<p><\/p>/g, '').trim()) {
      await api(`/api/workspaces/${o.wsId}/pages/${page.id}`, { method: 'PUT', body: { content } })
      page.content = content
    }
    o.onProgress?.(++done, total)
  }
  return order.map((x) => x.page)
}
