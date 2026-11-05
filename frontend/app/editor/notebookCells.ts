/**
 * Cellules de notebook au format nbformat 4 (presse-papiers, pile des cellules supprimées,
 * métadonnées JupyterLab : repli, durée d'exécution) et sommaire des titres Markdown.
 */

export interface NbCellJson {
  cell_type: 'code' | 'markdown' | 'raw'
  id?: string
  metadata: Record<string, any>
  source: string | string[]
  outputs?: unknown[]
  execution_count?: number | null
  attachments?: Record<string, unknown>
}

const parse = <T>(v: unknown, d: T): T => { try { return typeof v === 'string' ? JSON.parse(v) as T : d } catch { return d } }
const splitLines = (s: string) => s.split(/(?<=\n)/)

/** Champs d'une cellule du document partagé (Y.Map) → cellule nbformat. */
export function cellJson(get: (k: string) => unknown): NbCellJson {
  const type = (get('type') as NbCellJson['cell_type']) || 'code'
  const base: NbCellJson = { cell_type: type, id: String(get('id') ?? ''), metadata: parse(get('meta'), {}), source: splitLines(String((get('source') as { toString(): string } | undefined)?.toString() ?? '')) }
  const attachments = parse<Record<string, unknown> | null>(get('attachments'), null)
  if (type !== 'code' && attachments && Object.keys(attachments).length) base.attachments = attachments
  if (type === 'code') { base.outputs = parse(get('outputs'), []); base.execution_count = (get('count') as number | null) ?? null }
  return base
}

/** Cellule nbformat → champs d'une cellule du document partagé (hors identifiant et texte). */
export function cellFields(c: NbCellJson): { type: NbCellJson['cell_type']; source: string; outputs: string; count: number | null; meta: string; attachments?: string } {
  const type: NbCellJson['cell_type'] = c.cell_type === 'markdown' || c.cell_type === 'raw' ? c.cell_type : 'code'
  const out = {
    type,
    source: Array.isArray(c.source) ? c.source.join('') : String(c.source ?? ''),
    outputs: JSON.stringify(type === 'code' && Array.isArray(c.outputs) ? c.outputs : []),
    count: type === 'code' && typeof c.execution_count === 'number' ? c.execution_count : null,
    meta: JSON.stringify(c.metadata && typeof c.metadata === 'object' ? c.metadata : {}),
  }
  return c.attachments && typeof c.attachments === 'object' && Object.keys(c.attachments).length ? { ...out, attachments: JSON.stringify(c.attachments) } : out
}

/** Texte du presse-papiers : JSON nbformat (liste de cellules), lisible par JupyterLab. */
export const clipboardText = (cells: NbCellJson[]) => JSON.stringify(cells, null, 1)

/**
 * Cellules dans un texte collé : liste de cellules (Looty, JupyterLab), cellule seule ou
 * notebook entier ; null si ce n'en est pas.
 */
export function parseClipboardCells(text: string): NbCellJson[] | null {
  const t = text.trim()
  if (!t.startsWith('[') && !t.startsWith('{')) return null
  let v: any
  try { v = JSON.parse(t) } catch { return null }
  const list = Array.isArray(v) ? v : Array.isArray(v?.cells) ? v.cells : v && typeof v === 'object' ? [v] : null
  if (!list?.length) return null
  const ok = list.every((c: any) => c && typeof c === 'object' && ['code', 'markdown', 'raw'].includes(c.cell_type) && (typeof c.source === 'string' || Array.isArray(c.source)))
  return ok ? list.slice(0, 500) : null
}

/** Titres Markdown d'une cellule (hors blocs de code). */
export function markdownHeadings(src: string): { level: number; text: string }[] {
  const out: { level: number; text: string }[] = []
  let fence = false
  for (const line of src.split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) { fence = !fence; continue }
    if (fence) continue
    const m = /^ {0,3}(#{1,6})\s+(.+?)\s*#*\s*$/.exec(line)
    if (m) out.push({ level: m[1]!.length, text: m[2]!.replace(/[*_`]/g, '').replace(/\[([^\]]*)\]\([^)]*\)/g, '$1') })
  }
  return out
}

/** Repli de l'entrée et de la sortie (`metadata.jupyter`, comme JupyterLab). */
export const isSourceHidden = (meta: any) => meta?.jupyter?.source_hidden === true
export const isOutputsHidden = (meta: any) => meta?.jupyter?.outputs_hidden === true
export function withHidden(meta: any, key: 'source_hidden' | 'outputs_hidden', hidden: boolean) {
  const m = meta && typeof meta === 'object' ? { ...meta } : {}
  const j = { ...(m.jupyter && typeof m.jupyter === 'object' ? m.jupyter : {}) }
  if (hidden) j[key] = true
  else delete j[key]
  if (Object.keys(j).length) m.jupyter = j
  else delete m.jupyter
  return m
}

/** Durée d'exécution au format de JupyterLab (`metadata.execution`, option « Record timing »). */
export function withTiming(meta: any, start: Date, end: Date) {
  const m = meta && typeof meta === 'object' ? { ...meta } : {}
  m.execution = {
    'iopub.execute_input': start.toISOString(),
    'iopub.status.busy': start.toISOString(),
    'iopub.status.idle': end.toISOString(),
    'shell.execute_reply': end.toISOString(),
  }
  return m
}
/** Durée enregistrée, en millisecondes (null sans mesure). */
export function executionMs(meta: any): number | null {
  const e = meta?.execution
  if (!e) return null
  const start = Date.parse(e['iopub.execute_input'] ?? e['iopub.status.busy'] ?? '')
  const end = Date.parse(e['shell.execute_reply'] ?? e['iopub.status.idle'] ?? '')
  return Number.isFinite(start) && Number.isFinite(end) && end >= start ? end - start : null
}
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(ms < 10_000 ? 2 : 1)} s`
  const m = Math.floor(ms / 60_000)
  return `${m} min ${Math.round((ms % 60_000) / 1000)} s`
}
