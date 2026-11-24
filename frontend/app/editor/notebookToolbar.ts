/**
 * Barre d'outils Markdown d'une cellule en édition : chaque action transforme le texte et la
 * sélection (fonctions pures, appliquées par l'éditeur de la cellule).
 */
export type MdAction = 'bold' | 'italic' | 'link' | 'image' | 'math' | 'list'

export interface MdEdit {
  /** Remplace [from, to] du texte par `insert`. */
  from: number
  to: number
  insert: string
  /** Sélection résultante (positions dans le texte modifié). */
  anchor: number
  head: number
}

export interface MdPlaceholders { text: string; link: string; alt: string; url: string; path: string; formula: string }

const URL_LIKE = /^(https?:\/\/|www\.)\S+$/i

function wrap(doc: string, from: number, to: number, mark: string, placeholder: string): MdEdit {
  const sel = doc.slice(from, to)
  const n = mark.length
  const before = doc.slice(from - n, from)
  const after = doc.slice(to, to + n)
  // Déjà entouré : on retire les marques. Pour « * », « ** » voisin (gras) n'est pas de l'italique.
  const bold = mark === '*' && (doc[from - 2] === '*' || doc[to + 1] === '*')
  if (sel && before === mark && after === mark && !bold) return { from: from - n, to: to + n, insert: sel, anchor: from - n, head: from - n + sel.length }
  if (sel.length >= 2 * n && sel.startsWith(mark) && sel.endsWith(mark) && !(mark === '*' && sel.startsWith('**'))) {
    const inner = sel.slice(n, sel.length - n)
    return { from, to, insert: inner, anchor: from, head: from + inner.length }
  }
  const body = sel || placeholder
  return { from, to, insert: mark + body + mark, anchor: from + n, head: from + n + body.length }
}

function list(doc: string, from: number, to: number): MdEdit {
  const start = doc.lastIndexOf('\n', from - 1) + 1
  const nl = doc.indexOf('\n', to)
  const end = nl < 0 ? doc.length : nl
  const lines = doc.slice(start, end).split('\n')
  const filled = lines.filter((l) => l.trim())
  const all = filled.length > 0 && filled.every((l) => /^\s*[-*+] /.test(l))
  const next = lines.map((l) => (!l.trim() && lines.length > 1 ? l : all ? l.replace(/^(\s*)[-*+] /, '$1') : `- ${l}`)).join('\n')
  return { from: start, to: end, insert: next, anchor: start, head: start + next.length }
}

export function markdownEdit(action: MdAction, doc: string, from: number, to: number, ph: MdPlaceholders): MdEdit {
  if (from > to) [from, to] = [to, from]
  const sel = doc.slice(from, to)
  switch (action) {
    case 'bold': return wrap(doc, from, to, '**', ph.text)
    case 'italic': return wrap(doc, from, to, '*', ph.text)
    case 'math': {
      if (sel.includes('\n')) {
        const insert = `$$\n${sel}\n$$`
        return { from, to, insert, anchor: from + 3, head: from + 3 + sel.length }
      }
      return wrap(doc, from, to, '$', ph.formula)
    }
    case 'link': {
      if (URL_LIKE.test(sel)) return { from, to, insert: `[${ph.link}](${sel})`, anchor: from + 1, head: from + 1 + ph.link.length }
      const label = sel || ph.link
      const insert = `[${label}](${ph.url})`
      return { from, to, insert, anchor: from + label.length + 3, head: from + label.length + 3 + ph.url.length }
    }
    case 'image': {
      const alt = sel || ph.alt
      const insert = `![${alt}](${ph.path})`
      return { from, to, insert, anchor: from + alt.length + 4, head: from + alt.length + 4 + ph.path.length }
    }
    case 'list': return list(doc, from, to)
  }
}
