import type { NbAttachments } from './notebookMarkdown'
// eslint-disable-next-line no-control-regex
const stripAnsi = (s: string) => s.replace(/\u001b\[[0-9;]*m/g, '')

/**
 * Export d'un notebook : script Python (.py, format « percent »), page HTML autonome (aussi
 * imprimée en PDF) et .ipynb autonome dont les images du projet deviennent des pièces jointes
 * (ADR 0006 : dans le document partagé, elles restent des fichiers du projet).
 */
export interface ExportCell {
  type: 'code' | 'markdown' | 'raw'
  source: string
  outputs?: any[]
  count?: number | null
  attachments?: NbAttachments | null
  metadata?: Record<string, any>
}

const escapeHtml = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
const text = (v: unknown) => (Array.isArray(v) ? v.join('') : String(v ?? ''))

/** Script Python au format « percent » (`# %%`) ; texte et commandes magiques en commentaires. */
export function notebookToPython(cells: ExportCell[]): string {
  const comment = (s: string) => s.split('\n').map((l) => (l ? `# ${l}` : '#')).join('\n')
  const parts = cells.map((c) => {
    const src = c.source.replace(/\s+$/, '')
    if (c.type === 'markdown') return `# %% [markdown]\n${comment(src)}`
    if (c.type === 'raw') return `# %% [raw]\n${comment(src)}`
    // Commandes magiques (%pip, !ls) : non exécutables hors Jupyter, laissées en commentaire.
    const body = src.split('\n').map((l) => (/^\s*[%!]/.test(l) ? l.replace(/^(\s*)/, '$1# ') : l)).join('\n')
    return `# %%\n${body}`
  })
  return parts.join('\n\n') + '\n'
}

/** Pièces jointes : nom → { MIME → base64 } d'une image de projet. */
export interface FetchedImage { mime: string; base64: string }
const IMG = /!\[([^\]]*)\]\(\s*<?([^)\s>]+)>?((?:\s+"[^"]*")?)\s*\)/g
const isAbsolute = (u: string) => /^([a-z][a-z0-9+.-]*:|\/|#)/i.test(u)

/**
 * Remplace les images relatives (fichiers du projet) d'une cellule Markdown par des pièces
 * jointes. Une image introuvable reste telle quelle.
 */
export async function embedImages(
  source: string,
  existing: NbAttachments | null | undefined,
  fetchImage: (path: string) => Promise<FetchedImage | null>,
): Promise<{ source: string; attachments: NbAttachments | null }> {
  const attachments: NbAttachments = { ...(existing ?? {}) }
  const found = new Map<string, string>()
  const wanted = [...source.matchAll(IMG)].map((m) => m[2]!).filter((p) => !isAbsolute(p))
  for (const raw of new Set(wanted)) {
    let path = raw
    try { path = decodeURIComponent(raw) } catch { /* adresse mal encodée : telle quelle */ }
    const img = await fetchImage(path)
    if (!img) continue
    const base = path.split('/').pop() || 'image'
    let name = base
    for (let i = 2; attachments[name]; i++) name = base.replace(/(\.[^.]*)?$/, `-${i}$1`)
    attachments[name] = { [img.mime]: img.base64 }
    found.set(raw, name)
  }
  const out = source.replace(IMG, (all, alt, path, title) => (found.has(path) ? `![${alt}](attachment:${encodeURIComponent(found.get(path)!)}${title})` : all))
  return { source: out, attachments: Object.keys(attachments).length ? attachments : null }
}

/** Fichier .ipynb autonome à partir du .ipynb courant (métadonnées gardées) et des cellules à jour. */
export function notebookToIpynb(meta: Record<string, any>, cells: ExportCell[], ids: string[]): string {
  const lines = (s: string) => s.split(/(?<=\n)/)
  const out = cells.map((c, i) => {
    const base: Record<string, any> = { cell_type: c.type, id: ids[i] || `c${i}`, metadata: c.metadata ?? {}, source: lines(c.source) }
    if (c.type !== 'code' && c.attachments && Object.keys(c.attachments).length) base.attachments = c.attachments
    return c.type === 'code' ? { ...base, execution_count: c.count ?? null, outputs: c.outputs ?? [] } : base
  })
  return JSON.stringify({ cells: out, metadata: meta.metadata ?? {}, nbformat: 4, nbformat_minor: 5 }, null, 1) + '\n'
}

const dataUri = (mime: string, data: string) => `data:${mime};base64,${data.replace(/\s/g, '')}`

function outputHtml(o: any, sanitize: (html: string) => string, renderMarkdown: (s: string) => string): string {
  if (o.output_type === 'stream') return `<pre class="out ${o.name === 'stderr' ? 'err' : ''}">${escapeHtml(text(o.text))}</pre>`
  if (o.output_type === 'error') return `<pre class="out err">${escapeHtml(stripAnsi((o.traceback?.length ? o.traceback.join('\n') : `${o.ename}: ${o.evalue}`) as string))}</pre>`
  const d: Record<string, unknown> = o.data ?? {}
  if (d['text/html']) return `<div class="out html">${sanitize(text(d['text/html']))}</div>`
  if (d['image/png']) return `<img class="out" alt="" src="${dataUri('image/png', text(d['image/png']))}">`
  if (d['image/jpeg']) return `<img class="out" alt="" src="${dataUri('image/jpeg', text(d['image/jpeg']))}">`
  // SVG : affiché comme image, ses scripts ne s'exécutent donc pas.
  if (d['image/svg+xml']) return `<img class="out" alt="" src="data:image/svg+xml;charset=utf-8,${encodeURIComponent(text(d['image/svg+xml']))}">`
  if (d['text/markdown']) return `<div class="out md">${renderMarkdown(text(d['text/markdown']))}</div>`
  if (d['text/plain']) return `<pre class="out">${escapeHtml(text(d['text/plain']))}</pre>`
  return ''
}

const CSS = `
body{font:15px/1.6 system-ui,-apple-system,"Segoe UI",sans-serif;color:#1f2328;max-width:920px;margin:0 auto;padding:24px 20px}
h1.title{font-size:1.8em;margin:.2em 0 .8em}
.cell{margin:0 0 14px}
pre{margin:0;padding:10px 12px;border-radius:6px;overflow:auto;font:13px/1.5 ui-monospace,SFMono-Regular,Menlo,monospace;white-space:pre-wrap;word-break:break-word}
pre.code{background:#f6f8fa;border:1px solid #d0d7de}
pre.raw{background:#fff;border:1px dashed #d0d7de}
.out{margin:4px 0 0 0}
pre.out{background:transparent;padding:4px 12px}
pre.err{color:#b42318}
img{max-width:100%;height:auto}
table{border-collapse:collapse}th,td{border:1px solid #d0d7de;padding:3px 10px}
.md code{background:#f6f8fa;padding:1px 5px;border-radius:4px}
.count{color:#6e7781;font:11px ui-monospace,monospace;margin:0 0 2px}
@media print{body{max-width:none;padding:0}.cell{break-inside:avoid-page}pre{white-space:pre-wrap}}
`

export const KATEX_CSS = 'https://cdn.jsdelivr.net/npm/katex@0.19.0/dist/katex.min.css'

/**
 * Page HTML autonome : images en data URI (pièces jointes), rendu Markdown et nettoyage
 * fournis par l'appelant. Seule dépendance externe : la feuille de style KaTeX, s'il y a des formules.
 */
export function notebookToHtml(
  cells: ExportCell[],
  opts: { title: string; lang: string; renderMarkdown: (src: string, attachments: NbAttachments | null) => string; sanitize: (html: string) => string },
): string {
  const body = cells.map((c) => {
    if (c.type === 'markdown') return `<section class="cell md">${opts.renderMarkdown(c.source, c.attachments ?? null)}</section>`
    if (c.type === 'raw') return `<section class="cell"><pre class="raw">${escapeHtml(c.source)}</pre></section>`
    const outs = (c.outputs ?? []).map((o) => outputHtml(o, opts.sanitize, (s) => opts.renderMarkdown(s, null))).join('')
    return `<section class="cell"><div class="count">${c.count != null ? `[${c.count}]` : ''}</div><pre class="code">${escapeHtml(c.source)}</pre>${outs}</section>`
  }).join('\n')
  const css = body.includes('class="katex"') ? `<link rel="stylesheet" href="${KATEX_CSS}">` : ''
  return `<!doctype html>\n<html lang="${escapeHtml(opts.lang)}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${escapeHtml(opts.title)}</title>${css}<style>${CSS}</style></head><body>\n<h1 class="title">${escapeHtml(opts.title)}</h1>\n${body}\n</body></html>\n`
}
