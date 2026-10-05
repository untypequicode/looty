import { nextTick, ref, type Ref } from 'vue'
/**
 * Saisie LaTeX dans les champs de formule (en ligne et en bloc) :
 * fermeture automatique des accolades / crochets / parenthèses, \begin{…} → \end{…},
 * et complétion des commandes (\fr → \frac{}{}) avec Tab / Entrée.
 */
type Field = HTMLInputElement | HTMLTextAreaElement

const PAIRS: Record<string, string> = { '{': '}', '[': ']', '(': ')' }
const CLOSERS = new Set(Object.values(PAIRS))

/** Commandes proposées ; « {} » marque les arguments (le curseur va dans le premier). */
const COMMANDS: string[] = [
  // Structures
  '\\frac{}{}', '\\dfrac{}{}', '\\tfrac{}{}', '\\sqrt{}', '\\sqrt[]{}', '\\binom{}{}', '\\sum_{}^{}', '\\prod_{}^{}', '\\int_{}^{}', '\\iint', '\\iiint', '\\oint',
  '\\lim_{}', '\\limsup', '\\liminf', '\\sup', '\\inf', '\\max', '\\min', '\\arg', '\\det', '\\dim', '\\ker', '\\deg', '\\gcd',
  '\\left(', '\\right)', '\\left[', '\\right]', '\\left\\{', '\\right\\}', '\\left|', '\\right|', '\\langle', '\\rangle', '\\lfloor', '\\rfloor', '\\lceil', '\\rceil',
  '\\begin{}', '\\end{}', '\\text{}', '\\textbf{}', '\\mathrm{}', '\\mathbf{}', '\\mathbb{}', '\\mathcal{}', '\\mathfrak{}', '\\mathscr{}', '\\mathsf{}', '\\mathit{}', '\\operatorname{}',
  '\\vec{}', '\\hat{}', '\\widehat{}', '\\tilde{}', '\\widetilde{}', '\\bar{}', '\\overline{}', '\\underline{}', '\\dot{}', '\\ddot{}', '\\overbrace{}^{}', '\\underbrace{}_{}', '\\overrightarrow{}', '\\boxed{}', '\\cancel{}', '\\color{}{}',
  '\\newcommand{}{}', '\\renewcommand{}{}', '\\DeclareMathOperator{}{}', '\\usepackage{}',
  // Fonctions
  '\\sin', '\\cos', '\\tan', '\\cot', '\\sec', '\\csc', '\\arcsin', '\\arccos', '\\arctan', '\\sinh', '\\cosh', '\\tanh', '\\log', '\\ln', '\\exp', '\\mod', '\\bmod', '\\pmod{}',
  // Grec
  '\\alpha', '\\beta', '\\gamma', '\\delta', '\\epsilon', '\\varepsilon', '\\zeta', '\\eta', '\\theta', '\\vartheta', '\\iota', '\\kappa', '\\lambda', '\\mu', '\\nu', '\\xi', '\\pi', '\\varpi', '\\rho', '\\varrho', '\\sigma', '\\varsigma', '\\tau', '\\upsilon', '\\phi', '\\varphi', '\\chi', '\\psi', '\\omega',
  '\\Gamma', '\\Delta', '\\Theta', '\\Lambda', '\\Xi', '\\Pi', '\\Sigma', '\\Upsilon', '\\Phi', '\\Psi', '\\Omega',
  // Relations et opérateurs
  '\\leq', '\\geq', '\\neq', '\\approx', '\\equiv', '\\sim', '\\simeq', '\\cong', '\\propto', '\\ll', '\\gg', '\\prec', '\\succ', '\\perp', '\\parallel', '\\mid', '\\nmid',
  '\\in', '\\notin', '\\ni', '\\subset', '\\subseteq', '\\supset', '\\supseteq', '\\cup', '\\cap', '\\bigcup_{}', '\\bigcap_{}', '\\setminus', '\\emptyset', '\\varnothing',
  '\\forall', '\\exists', '\\nexists', '\\neg', '\\land', '\\lor', '\\implies', '\\impliedby', '\\iff', '\\to', '\\mapsto', '\\gets', '\\rightarrow', '\\leftarrow', '\\Rightarrow', '\\Leftarrow', '\\Leftrightarrow', '\\leftrightarrow', '\\longrightarrow', '\\hookrightarrow',
  '\\times', '\\cdot', '\\div', '\\pm', '\\mp', '\\ast', '\\star', '\\circ', '\\bullet', '\\oplus', '\\otimes', '\\wedge', '\\vee',
  '\\infty', '\\partial', '\\nabla', '\\hbar', '\\ell', '\\Re', '\\Im', '\\aleph', '\\angle', '\\degree', '\\prime',
  '\\cdots', '\\ldots', '\\vdots', '\\ddots', '\\quad', '\\qquad', '\\,', '\\;', '\\!',
  '\\N', '\\Z', '\\Q', '\\R', '\\C',
]

const ENVIRONMENTS = ['pmatrix', 'bmatrix', 'vmatrix', 'matrix', 'cases', 'aligned', 'align', 'array', 'gathered', 'split', 'smallmatrix']

export interface Completion { label: string; insert: string }

function setValue(f: Field, value: string, caret: number) {
  f.value = value
  f.setSelectionRange(caret, caret)
  // v-model suit l'évènement input.
  f.dispatchEvent(new Event('input', { bubbles: true }))
}

/** Mot de commande en cours avant le curseur (« \fr »), ou l'environnement de « \begin{pm ». */
function currentToken(f: Field): { start: number; text: string; env: boolean } | null {
  const pos = f.selectionStart ?? 0
  if (pos !== f.selectionEnd) return null
  const before = f.value.slice(0, pos)
  const env = /\\(?:begin|end)\{([a-zA-Z*]*)$/.exec(before)
  if (env) return { start: pos - env[1]!.length, text: env[1]!, env: true }
  const m = /\\[a-zA-Z]*$/.exec(before)
  if (!m) return null
  return { start: pos - m[0].length, text: m[0], env: false }
}

/** Suggestions pour le curseur actuel (commandes du document comprises). */
export function latexCompletions(f: Field | null, extra: string[] = []): Completion[] {
  if (!f) return []
  const tok = currentToken(f)
  if (!tok) return []
  if (tok.env) {
    return ENVIRONMENTS.filter((e) => e.startsWith(tok.text) && e !== tok.text).slice(0, 8).map((e) => ({ label: e, insert: e }))
  }
  if (tok.text.length < 2) return []
  const all = [...new Set([...extra.map((d) => (d.startsWith('\\') ? d : `\\${d}`)), ...COMMANDS])]
  const head = all.filter((c) => c.startsWith(tok.text) && c !== tok.text)
  return head.slice(0, 8).map((c) => ({ label: c.replace(/\{\}/g, '{…}').replace(/\[\]/g, '[…]'), insert: c }))
}

/** Applique une suggestion : remplace le jeton, curseur dans le premier argument. */
export function applyCompletion(f: Field, c: Completion) {
  const tok = currentToken(f)
  if (!tok) return
  const pos = f.selectionStart ?? 0
  let insert = c.insert
  let caret: number
  if (tok.env) {
    // \begin{pmatrix} → ajoute la fermeture \end{pmatrix} s'il n'y en a pas déjà une.
    const after = f.value.slice(pos)
    const closeBrace = after.startsWith('}') ? 1 : 0
    const isBegin = /\\begin\{[a-zA-Z*]*$/.test(f.value.slice(0, pos))
    const end = isBegin && !after.includes(`\\end{${insert}}`) ? ` \\end{${insert}}` : ''
    const value = f.value.slice(0, tok.start) + insert + '}' + (isBegin ? ' ' : '') + end + after.slice(closeBrace)
    caret = tok.start + insert.length + 1 + (isBegin ? 1 : 0)
    setValue(f, value, caret)
    return
  }
  const first = insert.search(/\{\}|\[\]/)
  caret = tok.start + (first >= 0 ? first + 1 : insert.length)
  // \begin{} : ouvre directement la liste des environnements.
  if (insert === '\\begin{}') caret = tok.start + 7
  setValue(f, f.value.slice(0, tok.start) + insert + f.value.slice(pos), caret)
}

/**
 * Touches d'édition : renvoie true si l'évènement a été traité (paires, saut de fermant,
 * suppression d'une paire vide).
 */
export function handleLatexKey(e: KeyboardEvent, f: Field): boolean {
  if (e.ctrlKey || e.metaKey || e.altKey) return false
  const start = f.selectionStart ?? 0
  const end = f.selectionEnd ?? 0
  const v = f.value
  if (PAIRS[e.key]) {
    const close = PAIRS[e.key]!
    e.preventDefault()
    // Sélection : l'entourer ; « \{ » s'écrit \{ \}.
    const escaped = v[start - 1] === '\\' && start === end
    const inner = v.slice(start, end)
    const closing = escaped ? `\\${close}` : close
    setValue(f, v.slice(0, start) + e.key + inner + closing + v.slice(end), start + 1 + inner.length)
    if (inner) f.setSelectionRange(start + 1, start + 1 + inner.length)
    return true
  }
  if (CLOSERS.has(e.key) && start === end && v[start] === e.key) {
    e.preventDefault()
    f.setSelectionRange(start + 1, start + 1)
    return true
  }
  if (e.key === 'Backspace' && start === end && start > 0 && PAIRS[v[start - 1]!] === v[start]) {
    e.preventDefault()
    setValue(f, v.slice(0, start - 1) + v.slice(start + 1), start - 1)
    return true
  }
  return false
}

/**
 * État de complétion d'un champ de formule. `onKeydown` renvoie true si la touche a été
 * consommée (navigation dans la liste, paires) : la vue ne doit alors rien faire d'autre.
 */
export function useLatexField(field: Ref<Field | null>, macros: () => string[]) {
  const items = ref<Completion[]>([])
  const active = ref(0)
  function refresh() {
    items.value = latexCompletions(field.value, macros())
    active.value = 0
  }
  function pick(c: Completion) {
    if (!field.value) return
    applyCompletion(field.value, c)
    refresh()
  }
  function onKeydown(e: KeyboardEvent): boolean {
    const f = field.value
    if (!f) return false
    if (items.value.length) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault()
        const n = items.value.length
        active.value = (active.value + (e.key === 'ArrowDown' ? 1 : n - 1)) % n
        return true
      }
      if (e.key === 'Tab' || (e.key === 'Enter' && !e.ctrlKey && !e.metaKey)) {
        e.preventDefault()
        pick(items.value[active.value]!)
        return true
      }
      if (e.key === 'Escape') {
        e.preventDefault()
        items.value = []
        return true
      }
    }
    if (handleLatexKey(e, f)) {
      nextTick(refresh)
      return true
    }
    return false
  }
  const close = () => { items.value = [] }
  return { items, active, refresh, pick, onKeydown, close }
}
