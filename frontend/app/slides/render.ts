import * as Y from 'yjs'

/**
 * Rendu des diapositives (ADR 0023) : couleurs (clés de la palette de l'éditeur ou #rrggbb) et texte
 * riche d'une zone de texte converti en HTML sûr (tout le texte est échappé, les liens filtrés).
 */

/** Les 11 clés de la palette (`--pal-*`, ADR 0021), dans l'ordre de textStyle.ts. */
export const PALETTE_KEYS = ['gray', 'brown', 'red', 'orange', 'yellow', 'green', 'teal', 'blue', 'indigo', 'purple', 'pink'] as const

const HEX = /^#[0-9a-fA-F]{6}$/

/** Valeur CSS d'une couleur de diapositive ; `fallback` pour « défaut », vide ou valeur inconnue. */
export function slideColor(value: unknown, fallback: string): string {
  if (typeof value !== 'string' || !value || value === 'default') return fallback
  if (value === 'none') return 'transparent'
  if ((PALETTE_KEYS as readonly string[]).includes(value)) return `var(--pal-${value})`
  if (HEX.test(value)) return value
  return fallback
}

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

/** Seuls http(s), mailto et les liens relatifs sont gardés. */
export function safeHref(href: unknown): string | null {
  if (typeof href !== 'string') return null
  return /^(https?:\/\/|mailto:|\/|#)/i.test(href.trim()) ? href.trim() : null
}

function textHtml(t: Y.XmlText): string {
  return t.toDelta().map((d: { insert?: unknown; attributes?: Record<string, any> }) => {
    if (typeof d.insert !== 'string') return ''
    let s = esc(d.insert)
    const a = d.attributes ?? {}
    if (a.code) s = `<code>${s}</code>`
    if (a.bold) s = `<strong>${s}</strong>`
    if (a.italic) s = `<em>${s}</em>`
    if (a.underline) s = `<u>${s}</u>`
    if (a.strike) s = `<s>${s}</s>`
    const href = a.link ? safeHref(a.link.href ?? a.link) : null
    if (href) s = `<a href="${esc(href)}" rel="noopener noreferrer nofollow">${s}</a>`
    return s
  }).join('')
}

const BLOCKS: Record<string, string> = { paragraph: 'p', bulletList: 'ul', orderedList: 'ol', listItem: 'li', blockquote: 'blockquote' }

function nodeHtml(n: Y.XmlElement | Y.XmlText | Y.XmlHook | Y.XmlFragment): string {
  if (n instanceof Y.XmlText) return textHtml(n)
  if (!(n instanceof Y.XmlElement)) return n instanceof Y.XmlFragment ? n.toArray().map((c) => nodeHtml(c as Y.XmlElement)).join('') : ''
  const inner = n.toArray().map((c) => nodeHtml(c as Y.XmlElement)).join('')
  const name = n.nodeName
  if (name === 'hardBreak') return '<br>'
  if (name === 'heading') {
    const level = Math.min(4, Math.max(1, Number(n.getAttribute('level')) || 1))
    return `<h${level}>${inner || '<br>'}</h${level}>`
  }
  if (name === 'codeBlock') return `<pre><code>${inner}</code></pre>`
  const tag = BLOCKS[name]
  if (!tag) return inner
  return `<${tag}>${tag === 'p' && !inner ? '<br>' : inner}</${tag}>`
}

/** HTML d'un texte de diapositive (tout est échappé). */
export function fragmentHtml(f: Y.XmlFragment): string {
  return f.toArray().map((c) => nodeHtml(c as Y.XmlElement)).join('')
}

export interface ArrowGeometry { x1: number; x2: number; y: number; endHead: string | null; startHead: string | null }

/** Tracé d'une flèche dans sa boîte (w x h) : le trait s'arrête à la base de la pointe, pointes en triangle. */
export function arrowGeometry(w: number, h: number, strokeWidth: number, heads: unknown): ArrowGeometry {
  const sw = Math.max(1, Number.isFinite(strokeWidth) ? strokeWidth : 1)
  const mode = heads === 'both' || heads === 'none' ? heads : 'end'
  const hl = Math.min(Math.max(sw * 3.5, 20), Math.max(w / 2, 1))
  const hw = hl * 0.55
  const y = h / 2
  const end = mode !== 'none'
  const start = mode === 'both'
  return {
    x1: start ? hl : 0, x2: end ? Math.max(0, w - hl) : w, y,
    endHead: end ? `${w},${y} ${w - hl},${y - hw} ${w - hl},${y + hw}` : null,
    startHead: start ? `0,${y} ${hl},${y - hw} ${hl},${y + hw}` : null,
  }
}
