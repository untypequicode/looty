/**
 * Préambule LaTeX partagé dans un document.
 *
 * KaTeX rend chaque formule isolément : une macro définie dans un bloc n'existe pas
 * dans le suivant. On analyse donc nous-mêmes les déclarations de chaque formule
 * (\newcommand, \renewcommand, \providecommand, \DeclareMathOperator, \def, \let,
 * \usepackage) et on les propage, dans l'ordre du document, aux formules suivantes
 * (comme en LaTeX). Les déclarations sont retirées du rendu de la formule.
 *
 * Les macros sont compilées en fonctions KaTeX, ce qui permet aussi l'argument
 * optionnel de \newcommand ([n][défaut]) que KaTeX ne gère pas.
 */

export type MacroFn = (ctx: any) => string
export type Macros = Record<string, string | MacroFn>

export interface Statement {
  kind: 'define' | 'package'
  /** Texte source exact de la déclaration. */
  source: string
  start: number
  end: number
  name?: string
  packages?: string[]
}

export interface Parsed {
  statements: Statement[]
  /** Formule sans ses déclarations. */
  rest: string
  error?: string
}

// ---------------------------------------------------------------------------
// Analyse
// ---------------------------------------------------------------------------

const STMT = /\\(newcommand|renewcommand|providecommand|DeclareMathOperator|def|gdef|edef|xdef|let|usepackage|RequirePackage)(?![a-zA-Z])/g

function skipSpaces(s: string, i: number) {
  while (i < s.length && /\s/.test(s[i]!)) i++
  return i
}

/** Groupe {…} équilibré à partir de `i` (qui doit pointer sur « { »). */
function readGroup(s: string, i: number): { text: string; end: number } | null {
  if (s[i] !== '{') return null
  let depth = 0
  for (let j = i; j < s.length; j++) {
    const c = s[j]
    if (c === '\\') { j++; continue }
    if (c === '{') depth++
    else if (c === '}' && --depth === 0) return { text: s.slice(i + 1, j), end: j + 1 }
  }
  return null
}

/** Argument optionnel […] (crochets au premier niveau). */
function readOptional(s: string, i: number): { text: string; end: number } | null {
  i = skipSpaces(s, i)
  if (s[i] !== '[') return null
  let depth = 0
  for (let j = i + 1; j < s.length; j++) {
    const c = s[j]
    if (c === '\\') { j++; continue }
    if (c === '{') depth++
    else if (c === '}') depth--
    else if (c === ']' && depth === 0) return { text: s.slice(i + 1, j), end: j + 1 }
  }
  return null
}

/** Nom de commande : « \nom », « {\nom} » ou « \x » (symbole). */
function readName(s: string, i: number): { name: string; end: number } | null {
  i = skipSpaces(s, i)
  if (s[i] === '{') {
    const g = readGroup(s, i)
    const n = g?.text.trim()
    return g && n && /^\\([a-zA-Z@]+|.)$/.test(n) ? { name: n, end: g.end } : null
  }
  const m = /^\\([a-zA-Z@]+|.)/.exec(s.slice(i))
  return m ? { name: m[0], end: i + m[0].length } : null
}

export function parseLatex(src: string): Parsed {
  const statements: Statement[] = []
  let error: string | undefined
  STMT.lastIndex = 0
  let m: RegExpExecArray | null
  while ((m = STMT.exec(src))) {
    const start = m.index
    // « \\newcommand » : saut de ligne suivi du texte, pas une déclaration.
    let bs = 0
    for (let k = start - 1; k >= 0 && src[k] === '\\'; k--) bs++
    if (bs % 2) continue
    const cmd = m[1]!
    let i = start + m[0].length
    const fail = (msg: string) => { error ??= msg; STMT.lastIndex = i }

    if (cmd === 'usepackage' || cmd === 'RequirePackage') {
      const opt = readOptional(src, i)
      if (opt) i = opt.end
      const g = readGroup(src, skipSpaces(src, i))
      if (!g) { fail(`\\${cmd}: {package} expected`); continue }
      i = g.end
      statements.push({ kind: 'package', source: src.slice(start, i), start, end: i, packages: g.text.split(',').map((p) => p.trim()).filter(Boolean) })
    } else if (cmd === 'let') {
      const a = readName(src, i)
      if (!a) { fail('\\let: \\name expected'); continue }
      let j = skipSpaces(src, a.end)
      if (src[j] === '=') j++
      const b = readName(src, j)
      if (!b) { fail('\\let: target expected'); continue }
      i = b.end
      statements.push({ kind: 'define', source: src.slice(start, i), start, end: i, name: a.name })
    } else if (cmd.endsWith('def')) {
      const a = readName(src, i)
      if (!a) { fail(`\\${cmd}: \\name expected`); continue }
      const brace = src.indexOf('{', a.end)
      const g = brace >= 0 ? readGroup(src, brace) : null
      if (!g) { fail(`\\${cmd}${a.name}: {body} expected`); continue }
      i = g.end
      statements.push({ kind: 'define', source: src.slice(start, i), start, end: i, name: a.name })
    } else {
      if (src[i] === '*') i++
      const a = readName(src, i)
      if (!a) { fail(`\\${cmd}: \\name expected`); continue }
      i = a.end
      if (cmd !== 'DeclareMathOperator') {
        const n = readOptional(src, i)
        if (n) {
          i = n.end
          const d = readOptional(src, i)
          if (d) i = d.end
        }
      }
      const g = readGroup(src, skipSpaces(src, i))
      if (!g) { fail(`${a.name}: {definition} expected`); continue }
      i = g.end
      statements.push({ kind: 'define', source: src.slice(start, i), start, end: i, name: a.name })
    }
    STMT.lastIndex = i
  }
  let rest = ''
  let last = 0
  for (const s of statements) { rest += src.slice(last, s.start); last = s.end }
  rest += src.slice(last)
  return { statements, rest: rest.trim(), error }
}

// ---------------------------------------------------------------------------
// Compilation en macros KaTeX
// ---------------------------------------------------------------------------

/** Texte d'une liste de jetons KaTeX (ordre normal). */
function tokensText(tokens: { text: string }[]) {
  let out = ''
  for (const t of tokens) {
    if (/\\[a-zA-Z@]+$/.test(out) && /^[a-zA-Z@]/.test(t.text)) out += ' '
    out += t.text
  }
  return out
}

/**
 * Macro à `numArgs` arguments, dont le premier optionnel si `optDefault` est défini.
 * Les arguments sont substitués textuellement (#1, #2…), comme en TeX.
 */
export function makeMacro(body: string, numArgs: number, optDefault?: string): MacroFn {
  return (ctx: any) => {
    const args: string[] = []
    let required = numArgs
    if (optDefault !== undefined && numArgs > 0) {
      required--
      ctx.consumeSpaces()
      if (ctx.future().text === '[') {
        ctx.popToken()
        const toks: { text: string }[] = []
        let depth = 0
        for (;;) {
          const t = ctx.popToken()
          if (t.text === 'EOF') throw new Error('missing ] for optional argument')
          if (t.text === '{') depth++
          else if (t.text === '}') depth--
          else if (t.text === ']' && depth === 0) break
          toks.push(t)
        }
        args.push(tokensText(toks))
      } else {
        args.push(optDefault)
      }
    }
    if (required > 0) {
      for (const a of ctx.consumeArgs(required) as { text: string }[][]) args.push(tokensText([...a].reverse()))
    }
    return body.replace(/##|#(\d)/g, (m, d) => (m === '##' ? '#' : args[Number(d) - 1] ?? ''))
  }
}

/** Applique une déclaration à `macros` (modifié sur place). */
function applyDefinition(stmt: Statement, macros: Macros) {
  const s = stmt.source
  const cmd = /^\\([a-zA-Z]+)/.exec(s)![1]!
  let i = cmd.length + 1
  if (cmd === 'let') {
    const a = readName(s, i)!
    let j = skipSpaces(s, a.end)
    if (s[j] === '=') j++
    const b = readName(s, j)!
    const target = macros[b.name]
    macros[a.name] = target ?? b.name
    return
  }
  if (cmd.endsWith('def')) {
    const a = readName(s, i)!
    const brace = s.indexOf('{', a.end)
    const params = s.slice(a.end, brace)
    const g = readGroup(s, brace)!
    const n = (params.match(/#\d/g) ?? []).length
    macros[a.name] = makeMacro(g.text, n)
    return
  }
  const star = s[i] === '*'
  if (star) i++
  const a = readName(s, i)!
  i = a.end
  if (cmd === 'DeclareMathOperator') {
    const g = readGroup(s, skipSpaces(s, i))!
    macros[a.name] = `\\operatorname${star ? '*' : ''}{${g.text}}`
    return
  }
  let numArgs = 0
  let optDefault: string | undefined
  const n = readOptional(s, i)
  if (n) {
    numArgs = Math.min(9, Math.max(0, parseInt(n.text, 10) || 0))
    i = n.end
    const d = readOptional(s, i)
    if (d) { optDefault = d.text; i = d.end }
  }
  const g = readGroup(s, skipSpaces(s, i))!
  // \providecommand ne remplace pas une macro déjà définie dans le document.
  if (cmd === 'providecommand' && a.name in macros) return
  // \newcommand / \renewcommand : on n'impose pas la règle « existe / n'existe pas »
  // de LaTeX, sinon \newcommand{\R} échouerait (\R est prédéfinie par KaTeX).
  macros[a.name] = makeMacro(g.text, numArgs, optDefault)
}

// ---------------------------------------------------------------------------
// Paquets
// ---------------------------------------------------------------------------

/** Paquets dont les commandes sont déjà fournies par KaTeX (ou sans effet en mode math). */
const NATIVE = new Set([
  'amsmath', 'amssymb', 'amsfonts', 'amstext', 'amsbsy', 'amsthm', 'amscd', 'mathtools', 'bm', 'cancel', 'color', 'xcolor',
  'mathrsfs', 'esint', 'braket', 'latexsym', 'textcomp', 'stmaryrd', 'unicode-math', 'fontenc', 'inputenc', 'babel',
  'geometry', 'hyperref', 'graphicx', 'enumitem', 'lmodern', 'microtype', 'array', 'booktabs', 'float', 'url', 'xparse',
])

const m = (body: string, n = 0, opt?: string) => makeMacro(body, n, opt)

/** Commandes ajoutées par les paquets courants que KaTeX ne connaît pas. */
const PACKS: Record<string, Macros> = {
  physics: {
    '\\abs': m('\\left\\lvert #1 \\right\\rvert', 1),
    '\\norm': m('\\left\\lVert #1 \\right\\rVert', 1),
    '\\qty': m('\\left( #1 \\right)', 1),
    '\\pqty': m('\\left( #1 \\right)', 1),
    '\\bqty': m('\\left[ #1 \\right]', 1),
    '\\Bqty': m('\\left\\{ #1 \\right\\}', 1),
    '\\vqty': m('\\left\\lvert #1 \\right\\rvert', 1),
    '\\dd': '\\mathrm{d}',
    '\\dv': m('\\frac{\\mathrm{d}^{#1} #2}{\\mathrm{d} #3^{#1}}', 3, ''),
    '\\pdv': m('\\frac{\\partial^{#1} #2}{\\partial #3^{#1}}', 3, ''),
    '\\grad': '\\nabla',
    '\\div': '\\nabla\\cdot',
    '\\curl': '\\nabla\\times',
    '\\laplacian': '\\nabla^2',
    '\\vb': m('\\mathbf{#1}', 1),
    '\\va': m('\\vec{\\mathbf{#1}}', 1),
    '\\vu': m('\\hat{\\mathbf{#1}}', 1),
    '\\bra': m('\\left\\langle #1 \\right\\rvert', 1),
    '\\ket': m('\\left\\lvert #1 \\right\\rangle', 1),
    '\\braket': m('\\left\\langle #1 \\middle\\vert #2 \\right\\rangle', 2),
    '\\ketbra': m('\\left\\lvert #1 \\middle\\rangle\\!\\middle\\langle #2 \\right\\rvert', 2),
    '\\expval': m('\\left\\langle #1 \\right\\rangle', 1),
    '\\mel': m('\\left\\langle #1 \\middle\\vert #2 \\middle\\vert #3 \\right\\rangle', 3),
    '\\comm': m('\\left[ #1, #2 \\right]', 2),
    '\\acomm': m('\\left\\{ #1, #2 \\right\\}', 2),
    '\\order': m('\\mathcal{O}\\left( #1 \\right)', 1),
    '\\tr': '\\operatorname{tr}',
    '\\Tr': '\\operatorname{Tr}',
    '\\rank': '\\operatorname{rank}',
    '\\erf': '\\operatorname{erf}',
    '\\Res': '\\operatorname{Res}',
    '\\Re': '\\operatorname{Re}',
    '\\Im': '\\operatorname{Im}',
  },
  siunitx: {
    '\\si': m('\\mathrm{#1}', 1),
    '\\unit': m('\\mathrm{#1}', 1),
    '\\SI': m('#1\\,\\mathrm{#2}', 2),
    '\\qty': m('#1\\,\\mathrm{#2}', 2),
    '\\num': m('#1', 1),
    '\\ang': m('#1^{\\circ}', 1),
    '\\metre': 'm', '\\meter': 'm', '\\second': 's', '\\gram': 'g', '\\kilogram': 'kg', '\\ampere': 'A', '\\kelvin': 'K',
    '\\mole': 'mol', '\\candela': 'cd', '\\newton': 'N', '\\joule': 'J', '\\watt': 'W', '\\pascal': 'Pa', '\\hertz': 'Hz',
    '\\volt': 'V', '\\ohm': '\\Omega', '\\coulomb': 'C', '\\litre': 'L', '\\liter': 'L', '\\hour': 'h', '\\minute': 'min',
    '\\percent': '\\%', '\\degreeCelsius': '^{\\circ}\\mathrm{C}',
    '\\kilo': 'k', '\\mega': 'M', '\\giga': 'G', '\\milli': 'm', '\\micro': '\\mu', '\\nano': 'n', '\\centi': 'c',
    '\\per': '/', '\\squared': '^{2}', '\\cubed': '^{3}',
  },
  dsfont: { '\\mathds': m('\\mathbb{#1}', 1) },
  bbm: { '\\mathbbm': m('\\mathbb{#1}', 1) },
  gensymb: { '\\celsius': '^{\\circ}\\mathrm{C}', '\\ohm': '\\Omega', '\\micro': '\\mu', '\\perthousand': '\\text{‰}' },
  // \ce est fourni par l'extension mhchem de KaTeX, chargée à la demande.
  mhchem: {},
}

export type PackageStatus = 'native' | 'emulated' | 'unknown'
export function packageStatus(name: string): PackageStatus {
  return NATIVE.has(name) ? 'native' : name in PACKS ? 'emulated' : 'unknown'
}

let mhchem: Promise<unknown> | null = null
/** Charge l'extension mhchem (\ce, \pu) ; `onReady` déclenche un nouveau rendu. */
export function ensureMhchem(onReady: () => void) {
  if (!mhchem) mhchem = import('katex/contrib/mhchem').then(onReady, () => {})
}

// ---------------------------------------------------------------------------
// Portée dans un document
// ---------------------------------------------------------------------------

export interface Scope {
  /** Macros héritées des formules précédentes (avant les déclarations de celle-ci). */
  inherited: Macros
  /** Macros disponibles pour cette formule (préambule du document + ses propres déclarations). */
  macros: Macros
  parsed: Parsed
  /** Noms définis par cette formule. */
  defines: string[]
  packages: { name: string; status: PackageStatus }[]
  needsMhchem: boolean
}

/** Texte source de chaque portée (pour savoir si un brouillon en diffère). */
const sources = new WeakMap<Scope, string>()

export interface ScopeBuilder {
  /** Ajoute la formule suivante du document et renvoie sa portée. */
  next(latex: string): Scope
}

export function scopeBuilder(initial: Macros = {}, mhchemInitially = false): ScopeBuilder {
  let macros: Macros = initial
  let mhchemUsed = mhchemInitially
  return {
    next(latex: string) {
      const inherited = macros
      const parsed = parseLatex(latex)
      const packages: Scope['packages'] = []
      if (parsed.statements.length) {
        macros = { ...macros }
        for (const st of parsed.statements) {
          if (st.kind === 'package') {
            for (const p of st.packages ?? []) {
              packages.push({ name: p, status: packageStatus(p) })
              if (p === 'mhchem') mhchemUsed = true
              Object.assign(macros, PACKS[p] ?? {})
            }
          } else {
            try { applyDefinition(st, macros) } catch { parsed.error ??= `${st.name}: invalid definition` }
          }
        }
      }
      const scope: Scope = {
        inherited,
        macros,
        parsed,
        defines: parsed.statements.filter((s) => s.kind === 'define').map((s) => s.name!),
        packages,
        needsMhchem: mhchemUsed,
      }
      sources.set(scope, latex)
      return scope
    },
  }
}

/** Portée d'une formule en cours d'édition, à partir de ce qu'elle hérite. */
export function draftScope(latex: string, base?: Scope): Scope {
  return scopeBuilder(base?.inherited ?? {}, base?.needsMhchem ?? false).next(latex)
}

export interface MathRender {
  html: string
  error: string
  scope: Scope
}

/**
 * Rend une formule avec les macros de son document. `committed` = portée calculée
 * pour le contenu enregistré ; si `latex` diffère (brouillon), on la recalcule.
 */
export function renderMath(katex: { renderToString: (s: string, o: object) => string }, latex: string, committed: Scope | undefined, displayMode: boolean): MathRender {
  const scope = committed && sameSource(committed, latex) ? committed : draftScope(latex, committed)
  let html = ''
  let error = scope.parsed.error ?? ''
  if (scope.parsed.rest) {
    try {
      // trust: false — \href, \url et \includegraphics restent désactivés.
      html = katex.renderToString(scope.parsed.rest, { displayMode, throwOnError: true, trust: false, macros: { ...scope.macros }, maxExpand: 500 })
    } catch (e: any) {
      error = String(e?.message ?? e)
    }
  }
  return { html, error, scope }
}

function sameSource(scope: Scope, latex: string) {
  return sources.get(scope) === latex
}
