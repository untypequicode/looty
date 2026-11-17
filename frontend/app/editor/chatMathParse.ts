// Repérage des formules LaTeX dans une réponse Markdown, sans dépendance au DOM.
// Une formule n'est reconnue que lorsqu'elle est fermée : pendant le flux, une formule
// ouverte reste du texte brut, puis se rend d'un coup à la fermeture.

export interface MathPart {
  placeholder: string
  tex: string
  display: boolean
}

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

/** Remplace chaque formule fermée par un jeton neutre ; code (bloc ou en ligne) et `\$` intacts. */
export function extractMath(src: string, nonce = 'LTYMATH'): { text: string; parts: MathPart[] } {
  const parts: MathPart[] = []
  let out = ''
  const push = (tex: string, display: boolean) => {
    const placeholder = `${nonce}${parts.length}X`
    parts.push({ placeholder, tex, display })
    out += placeholder
  }
  const n = src.length
  let i = 0
  let atLineStart = true
  while (i < n) {
    const c = src[i]!
    // Bloc de code clôturé : copié tel quel jusqu'à la clôture (ou la fin, pendant le flux).
    if (atLineStart) {
      const m = /^( {0,3})(`{3,}|~{3,})[^\n]*(\n|$)/.exec(src.slice(i))
      if (m) {
        const fence = m[2]!
        const close = new RegExp(`(^|\\n) {0,3}${fence[0] === '`' ? '`' : '~'}{${fence.length},}[ \\t]*(\\n|$)`)
        const rest = src.slice(i + m[0].length)
        const cm = close.exec(rest)
        const end = cm ? i + m[0].length + cm.index + cm[0].length : n
        out += src.slice(i, end)
        i = end
        atLineStart = true
        continue
      }
    }
    atLineStart = c === '\n'
    if (c === '`') {
      let k = i
      while (src[k] === '`') k++
      const ticks = src.slice(i, k)
      const close = src.indexOf(ticks, k)
      if (close >= 0) {
        out += src.slice(i, close + ticks.length)
        i = close + ticks.length
        continue
      }
      out += ticks
      i = k
      continue
    }
    if (c === '\\') {
      const nx = src[i + 1]
      if (nx === '[' || nx === '(') {
        const closer = nx === '[' ? '\\]' : '\\)'
        const end = src.indexOf(closer, i + 2)
        if (end > i + 2 && (nx === '[' || !src.slice(i + 2, end).includes('\n\n'))) {
          push(src.slice(i + 2, end).trim(), nx === '[')
          i = end + 2
          continue
        }
      }
      // Échappement (\$, \\ …) : deux caractères, le second n'ouvre rien.
      out += nx === undefined ? c : c + nx
      i += nx === undefined ? 1 : 2
      continue
    }
    if (c === '$') {
      if (src[i + 1] === '$') {
        const end = findClose(src, i + 2, '$$')
        if (end > i + 2) {
          push(src.slice(i + 2, end).trim(), true)
          i = end + 2
          continue
        }
        out += '$$'
        i += 2
        continue
      }
      const end = inlineEnd(src, i + 1)
      if (end > 0) {
        push(src.slice(i + 1, end), false)
        i = end + 1
        continue
      }
    }
    out += c
    i++
  }
  return { text: out, parts }
}

function findClose(s: string, from: number, closer: string): number {
  for (let k = from; k < s.length; k++) {
    if (s[k] === '\\') { k++; continue }
    if (s.startsWith(closer, k)) return k
  }
  return -1
}

/** `$…$` en ligne (règles pandoc) : ni espace après l'ouverture ni avant la fermeture, pas de chiffre après la fermeture. */
function inlineEnd(s: string, from: number): number {
  const first = s[from]
  if (first === undefined || /\s/.test(first)) return -1
  for (let k = from; k < s.length; k++) {
    const c = s[k]!
    if (c === '\\') { k++; continue }
    if (c === '\n' && s[k + 1] === '\n') return -1
    if (c === '`') return -1
    if (c === '$') {
      if (/\s/.test(s[k - 1]!) || /\d/.test(s[k + 1] ?? '')) return -1
      return k > from ? k : -1
    }
  }
  return -1
}

/** Remet les formules rendues à la place des jetons (le texte entourant est déjà nettoyé). */
export function injectMath(html: string, parts: MathPart[], renderOne: (tex: string, display: boolean) => string): string {
  let out = html
  for (const p of parts) {
    let r: string
    try { r = renderOne(p.tex, p.display) } catch { r = `<code>${esc(p.tex)}</code>` }
    out = out.split(p.placeholder).join(r)
  }
  return out
}
