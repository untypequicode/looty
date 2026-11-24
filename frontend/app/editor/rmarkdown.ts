/**
 * Documents R Markdown (.Rmd) et Quarto (.qmd) <-> notebook (.ipynb), à la manière de jupytext.
 *
 * - l'en-tête YAML devient une cellule « raw » (`metadata.looty.rmd.kind = 'yaml'`) ;
 * - un chunk ```{r nom, echo=FALSE} devient une cellule de code ; l'en-tête exact du chunk (options
 *   comprises) est gardé dans `metadata.looty.rmd.header`, les lignes `#| echo: false` de Quarto
 *   restent dans le code ;
 * - un chunk d'un autre moteur (bash, sql…) devient une cellule « raw » portant le même en-tête :
 *   il n'est jamais exécuté par le noyau du notebook ;
 * - le texte entre les chunks devient une cellule Markdown ;
 * - les lignes vides entre cellules (autres que la ligne vide habituelle), la fin de ligne
 *   (LF / CRLF) et le retour final sont mémorisés : un document ouvert puis réécrit est identique.
 * Les sorties ne sont pas écrites dans le .Rmd (comme jupytext) et les pièces jointes Markdown non plus.
 */
import type { ExportCell } from './notebookExport.ts'

export type NbLanguage = 'python' | 'r'

/** Langage d'un notebook, d'après `kernelspec` / `language_info` (R, sinon Python). */
export function notebookLanguage(meta: Record<string, any> | null | undefined): NbLanguage {
  const md = meta?.metadata ?? meta ?? {}
  const name = String(md?.kernelspec?.language ?? md?.language_info?.name ?? '').toLowerCase()
  return name === 'r' ? 'r' : 'python'
}

export interface ChunkHeader { engine: string; label: string; options: Record<string, string> }

/** Sépare une liste d'options `a=1, b="x,y", c=c(1,2)` sur les virgules de premier niveau. */
function splitTop(s: string): string[] {
  const out: string[] = []
  let depth = 0
  let quote = ''
  let cur = ''
  for (const ch of s) {
    if (quote) { cur += ch; if (ch === quote) quote = ''; continue }
    if (ch === '"' || ch === "'") { quote = ch; cur += ch; continue }
    if (ch === '(' || ch === '[' || ch === '{') depth++
    if (ch === ')' || ch === ']' || ch === '}') depth--
    if (ch === ',' && depth <= 0) { out.push(cur); cur = ''; continue }
    cur += ch
  }
  if (cur.trim()) out.push(cur)
  return out.map((p) => p.trim()).filter(Boolean)
}

/** En-tête d'un chunk (le texte entre accolades) : moteur, étiquette et options. */
export function parseChunkHeader(header: string): ChunkHeader {
  const m = /^\s*([A-Za-z0-9_.+-]+)\s*(.*)$/s.exec(header)
  const engine = (m?.[1] ?? 'r').toLowerCase()
  let rest = (m?.[2] ?? '').trim()
  if (rest.startsWith(',')) rest = rest.slice(1)
  const options: Record<string, string> = {}
  let label = ''
  splitTop(rest).forEach((part, i) => {
    const eq = /^([A-Za-z0-9_.-]+)\s*=\s*(.*)$/s.exec(part)
    if (eq) options[eq[1]!] = eq[2]!.trim()
    else if (i === 0 && /^[^=\s]+$/.test(part)) label = part
  })
  return { engine, label, options }
}

const stripQuotes = (v: string) => v.replace(/^(["'])(.*)\1$/, '$2').trim()
const isFalse = (v: string | undefined) => v !== undefined && /^(false|f|0|no)$/i.test(stripQuotes(v))

/**
 * Options effectives d'une cellule pour le rendu : en-tête du chunk (`echo=FALSE`) et lignes
 * `#| echo: false` de Quarto, ces dernières l'emportant.
 */
export function chunkRender(header: string | undefined, source: string): { echo: boolean; include: boolean; hideResults: boolean } {
  const o = header ? parseChunkHeader(header).options : {}
  for (const line of source.split('\n')) {
    const m = /^#\|\s*([A-Za-z0-9_.-]+)\s*:\s*(.*?)\s*$/.exec(line)
    if (m) o[m[1]!] = m[2]!
    else if (line.trim() && !line.startsWith('#|')) break
  }
  const results = stripQuotes(o.results ?? '')
  return { echo: !isFalse(o.echo), include: !isFalse(o.include), hideResults: results === 'hide' || isFalse(o.output) }
}

export interface RmdDocument {
  /** Métadonnées du notebook (nbformat 4) : noyau et mise en forme conservée. */
  metadata: Record<string, any>
  cells: ExportCell[]
}

const OPEN = /^(`{3,})\s*\{([^}]*)\}\s*$/
const closes = (line: string, n: number) => new RegExp('^`{' + n + ',}\\s*$').test(line)

const KERNELS: Record<NbLanguage, Record<string, any>> = {
  r: { kernelspec: { name: 'ir', display_name: 'R', language: 'R' }, language_info: { name: 'R' }, looty: { engine: 'jupyter', kernel: 'ir' } },
  python: { kernelspec: { name: 'python3', display_name: 'Python 3 (Pyodide)', language: 'python' }, language_info: { name: 'python' } },
}

const isBlank = (l: string) => l.trim() === ''

/** Texte .Rmd / .qmd -> cellules de notebook. Ne perd rien : voir `notebookToRmd`. */
export function rmdToNotebook(text: string, opts: { flavor?: 'rmd' | 'qmd' } = {}): RmdDocument {
  const crlf = text.includes('\r\n')
  let src = crlf ? text.replace(/\r\n/g, '\n') : text
  const finalNewline = src.endsWith('\n')
  if (finalNewline) src = src.slice(0, -1)
  const lines = src === '' && !finalNewline ? [] : src.split('\n')

  type Block = { kind: 'yaml' | 'prose' | 'chunk'; lines: string[]; header?: string; fence?: number }
  const blocks: Block[] = []
  const gaps: number[] = []
  let lead = 0
  let i = 0
  while (i < lines.length && isBlank(lines[i]!)) { lead++; i++ }

  let prose: string[] = []
  const flushProse = () => {
    if (!prose.length) return
    let end = prose.length
    while (end > 0 && isBlank(prose[end - 1]!)) end--
    const trailing = prose.length - end
    if (end > 0) { blocks.push({ kind: 'prose', lines: prose.slice(0, end) }); gaps.push(trailing) } else if (blocks.length) gaps[gaps.length - 1]! += trailing
    prose = []
  }
  // En-tête YAML : première ligne `---`, fermée par `---` ou `...`.
  if (i < lines.length && lines[i] === '---') {
    let j = i + 1
    while (j < lines.length && lines[j] !== '---' && lines[j] !== '...') j++
    if (j < lines.length) {
      blocks.push({ kind: 'yaml', lines: lines.slice(i, j + 1) })
      gaps.push(0)
      i = j + 1
    }
  }
  while (i < lines.length) {
    const line = lines[i]!
    const open = OPEN.exec(line)
    if (open) {
      const n = open[1]!.length
      let j = i + 1
      while (j < lines.length && !closes(lines[j]!, n)) j++
      if (j < lines.length) {
        flushProse()
        // Lignes vides avant le chunk : rattachées à la cellule précédente.
        blocks.push({ kind: 'chunk', lines: lines.slice(i + 1, j), header: open[2]!, fence: n })
        gaps.push(0)
        i = j + 1
        continue
      }
    } else if (/^`{3,}/.test(line)) {
      // Bloc de code ordinaire (sans accolades) : fait partie du texte, ses lignes ne sont pas analysées.
      const n = /^`+/.exec(line)![0].length
      let j = i + 1
      while (j < lines.length && !closes(lines[j]!, n)) j++
      if (j < lines.length) { prose.push(...lines.slice(i, j + 1)); i = j + 1; continue }
    }
    // Ligne de texte : les lignes vides qui suivent un chunk ou l'en-tête s'ajoutent à son écart.
    if (!prose.length && isBlank(line) && blocks.length) { gaps[gaps.length - 1]!++; i++; continue }
    prose.push(line)
    i++
  }
  flushProse()

  const langs = blocks.filter((b) => b.kind === 'chunk').map((b) => parseChunkHeader(b.header!).engine)
  const firstEngine = langs.find((e) => e === 'r' || e === 'python') ?? (opts.flavor === 'qmd' && langs.length === 0 ? 'python' : 'r')
  const language: NbLanguage = firstEngine === 'python' ? 'python' : 'r'

  const rmdMeta: Record<string, any> = {}
  if (crlf) rmdMeta.eol = 'crlf'
  if (!finalNewline && lines.length) rmdMeta.final_newline = false
  if (lead) rmdMeta.lead = lead
  if (opts.flavor) rmdMeta.flavor = opts.flavor

  const cells: ExportCell[] = blocks.map((b, k) => {
    const gap = gaps[k] ?? 0
    const meta: Record<string, any> = {}
    const rmd: Record<string, any> = {}
    const defaultGap = k === blocks.length - 1 ? 0 : 1
    if (gap !== defaultGap) rmd.gap = gap
    if (b.kind === 'yaml') {
      rmd.kind = 'yaml'
      if (b.lines[b.lines.length - 1] === '...') rmd.yaml_end = '...'
      const inner = b.lines.slice(1, -1)
      if (inner.length === 1 && inner[0] === '') rmd.blank_body = true
      return { type: 'raw', source: inner.join('\n'), metadata: { looty: { rmd } } }
    }
    if (b.kind === 'prose') {
      if (Object.keys(rmd).length) meta.looty = { rmd }
      return { type: 'markdown', source: b.lines.join('\n'), metadata: meta }
    }
    const header = b.header!
    const engine = parseChunkHeader(header).engine
    rmd.header = header
    if (b.fence !== 3) rmd.fence = b.fence
    if (b.lines.length === 1 && b.lines[0] === '') rmd.blank_body = true
    meta.looty = { rmd }
    const body = b.lines.join('\n')
    return engine === language
      ? { type: 'code', source: body, outputs: [], count: null, metadata: meta }
      : { type: 'raw', source: body, metadata: meta }
  })

  const metadata: Record<string, any> = { ...KERNELS[language] }
  if (Object.keys(rmdMeta).length) metadata.looty = { ...(metadata.looty ?? {}), rmd: rmdMeta }
  return { metadata, cells }
}

const longestTicks = (s: string) => Math.max(0, ...(s.match(/`+/g) ?? []).map((t) => t.length))

/** Cellules de notebook -> texte .Rmd / .qmd (lecture inverse de `rmdToNotebook`). */
export function notebookToRmd(meta: Record<string, any> | null | undefined, cells: ExportCell[]): string {
  const md = meta?.metadata ?? meta ?? {}
  const docMeta = md.looty?.rmd ?? {}
  const language = notebookLanguage(meta)
  const parts: string[] = []
  cells.forEach((c, k) => {
    const rmd = c.metadata?.looty?.rmd ?? {}
    const src = c.type === 'markdown' ? c.source.replace(/\n+$/, '') : c.source
    let body: string
    if (c.type === 'raw' && rmd.kind === 'yaml') body = `---${src || rmd.blank_body ? `\n${src}` : ''}\n${rmd.yaml_end === '...' ? '...' : '---'}`
    else if (c.type === 'raw' && typeof rmd.header === 'string') body = fence(rmd.header, src, rmd.fence, rmd.blank_body)
    else if (c.type === 'code') body = fence(typeof rmd.header === 'string' ? rmd.header : language, src, rmd.fence, rmd.blank_body)
    else body = src
    const defaultGap = k === cells.length - 1 ? 0 : 1
    const gap = typeof rmd.gap === 'number' ? rmd.gap : defaultGap
    parts.push(body + '\n' + '\n'.repeat(gap))
  })
  let out = '\n'.repeat(Number(docMeta.lead) || 0) + parts.join('')
  if (docMeta.final_newline === false && out.endsWith('\n')) out = out.slice(0, -1)
  return docMeta.eol === 'crlf' ? out.replace(/\n/g, '\r\n') : out
}

function fence(header: string, body: string, wanted?: number, blank?: boolean): string {
  const n = Math.max(3, Number(wanted) || 3, longestTicks(body) + 1)
  const ticks = '`'.repeat(n)
  return `${ticks}{${header}}${body || blank ? `\n${body}` : ''}\n${ticks}`
}
