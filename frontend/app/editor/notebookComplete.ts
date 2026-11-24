/**
 * Complétion de code d'une cellule : Jedi (Pyodide) ou `complete_request` (serveur Jupyter).
 * Jupyter et Python comptent les positions en points de code Unicode, CodeMirror en unités
 * UTF-16 : conversions ici, puis résultat au format de @codemirror/autocomplete.
 */
export interface CompletionItem { label: string; type?: string }
export interface CompletionOut { from: number; to: number; options: { label: string; type?: string }[]; validFor?: RegExp }

/** Position UTF-16 → points de code. */
export const toCodePoints = (code: string, offset: number) => Array.from(code.slice(0, offset)).length
/** Points de code → position UTF-16. */
export function fromCodePoints(code: string, n: number): number {
  let i = 0
  for (let k = 0; k < n && i < code.length; k++) i += code.codePointAt(i)! > 0xffff ? 2 : 1
  return i
}

/** Types Jupyter / Jedi → types CodeMirror (icône). */
export function cmType(t: string | undefined): string | undefined {
  switch ((t ?? '').toLowerCase()) {
    case 'function': case 'method': case 'builtin_function_or_method': case 'instancemethod': return 'function'
    case 'class': case 'type': return 'class'
    case 'module': return 'namespace'
    case 'keyword': return 'keyword'
    case 'property': return 'property'
    case 'param': case 'variable': case 'statement': case 'instance': case 'int': case 'str': case 'float': return 'variable'
    default: return undefined
  }
}

function build(code: string, start: number, end: number, items: CompletionItem[]): CompletionOut | null {
  const labels = new Set<string>()
  const options = []
  for (const it of items) {
    if (!it.label || labels.has(it.label)) continue
    labels.add(it.label)
    options.push({ label: it.label, type: cmType(it.type) })
    if (options.length >= 200) break
  }
  if (!options.length) return null
  return { from: fromCodePoints(code, start), to: fromCodePoints(code, end), options, validFor: /^[\w$]*$/ }
}

/** Réponse `complete_reply` de Jupyter (`content`) → complétions, ou null. */
export function fromCompleteReply(code: string, content: any): CompletionOut | null {
  if (!content || content.status !== 'ok' || !Array.isArray(content.matches)) return null
  const types: { text: string; type?: string }[] = content.metadata?._jupyter_types_experimental ?? []
  const typeOf = new Map(types.map((t) => [t.text, t.type]))
  const start = Number(content.cursor_start)
  const end = Number(content.cursor_end)
  if (!Number.isFinite(start) || !Number.isFinite(end) || start > end) return null
  return build(code, start, end, content.matches.map((m: unknown) => ({ label: String(m), type: typeOf.get(String(m)) })))
}

/** Réponse de Jedi côté Pyodide : `prefix` = nombre de points de code déjà saisis avant le curseur. */
export function fromJedi(code: string, cursor: number, reply: { prefix: number; items: CompletionItem[] } | null): CompletionOut | null {
  if (!reply || !Array.isArray(reply.items)) return null
  const start = Math.max(0, cursor - Math.max(0, Number(reply.prefix) || 0))
  return build(code, start, cursor, reply.items)
}
