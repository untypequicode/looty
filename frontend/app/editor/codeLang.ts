import type { Extension } from '@codemirror/state'
import { StreamLanguage } from '@codemirror/language'
import { html } from '@codemirror/lang-html'
import { css } from '@codemirror/lang-css'
import { javascript } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { markdown } from '@codemirror/lang-markdown'
import { stex } from '@codemirror/legacy-modes/mode/stex'
import { yaml } from '@codemirror/legacy-modes/mode/yaml'
import { python } from '@codemirror/legacy-modes/mode/python'
import { lua } from '@codemirror/legacy-modes/mode/lua'
import { xml } from '@codemirror/legacy-modes/mode/xml'
import { autocompletion, snippetCompletion, type Completion, type CompletionContext } from '@codemirror/autocomplete'
import { COMMANDS } from './latexInput'

/** Coloration selon l'extension du fichier. */
export function languageFor(path: string): Extension[] {
  const ext = path.toLowerCase().split('.').pop() ?? ''
  switch (ext) {
    case 'html': case 'htm': return [html()]
    case 'css': return [css()]
    case 'js': case 'mjs': return [javascript()]
    case 'ts': return [javascript({ typescript: true })]
    case 'json': case 'webmanifest': return [json()]
    case 'md': return [markdown()]
    case 'tex': case 'sty': case 'cls': case 'ltx': case 'dtx': case 'bib': case 'bbx': case 'cbx': case 'tikz': case 'def':
      return [StreamLanguage.define(stex)]
    case 'yaml': case 'yml': return [StreamLanguage.define(yaml)]
    case 'py': return [StreamLanguage.define(python)]
    case 'lua': return [StreamLanguage.define(lua)]
    case 'xml': case 'svg': return [StreamLanguage.define(xml)]
    default: return []
  }
}

/** Commandes de document LaTeX (en plus des commandes mathématiques de l'éditeur). */
const DOC_COMMANDS = [
  '\\section{}', '\\subsection{}', '\\subsubsection{}', '\\chapter{}', '\\paragraph{}', '\\part{}', '\\section*{}', '\\subsection*{}',
  '\\textbf{}', '\\textit{}', '\\emph{}', '\\underline{}', '\\texttt{}', '\\textsc{}', '\\textsf{}', '\\footnote{}', '\\url{}', '\\href{}{}',
  '\\label{}', '\\ref{}', '\\eqref{}', '\\pageref{}', '\\cite{}', '\\citep{}', '\\citet{}', '\\autoref{}', '\\cref{}',
  '\\includegraphics[width=\\linewidth]{}', '\\input{}', '\\include{}', '\\caption{}', '\\centering', '\\usepackage{}', '\\usepackage[]{}',
  '\\documentclass{}', '\\title{}', '\\author{}', '\\date{}', '\\maketitle', '\\tableofcontents', '\\newpage', '\\clearpage', '\\item',
  '\\bibliography{}', '\\bibliographystyle{}', '\\printbibliography', '\\addbibresource{}', '\\newcommand{}{}', '\\renewcommand{}{}',
  '\\hline', '\\vspace{}', '\\hspace{}', '\\noindent', '\\today', '\\LaTeX', '\\textwidth', '\\linewidth', '\\frametitle{}',
]
const ENVIRONMENTS = ['document', 'itemize', 'enumerate', 'description', 'figure', 'table', 'tabular', 'equation', 'equation*', 'align', 'align*', 'gather', 'cases', 'matrix', 'pmatrix', 'bmatrix', 'center', 'abstract', 'quote', 'verbatim', 'minipage', 'frame', 'theorem', 'proof', 'lemma', 'definition', 'thebibliography', 'tikzpicture', 'lstlisting', 'columns', 'column']

/** « \frac{}{} » → modèle avec champs ({#{}}) parcourus avec Tab. */
const asSnippet = (cmd: string) => cmd.replace(/\{\}/g, '{#{}}').replace(/\[\]/g, '[#{}]')

const commandOptions: Completion[] = [...new Set([...DOC_COMMANDS, ...COMMANDS])].map((c) => {
  const label = c.replace(/[{[].*$/, '')
  return snippetCompletion(asSnippet(c), { label, detail: c.slice(label.length) || undefined, type: 'keyword' })
})
const envOptions: Completion[] = ENVIRONMENTS.map((env) =>
  snippetCompletion(`\\begin{${env}}\n\t#{}\n\\end{${env}}`, { label: `\\begin{${env}}`, type: 'type', boost: 1 }),
)

function latexSource(ctx: CompletionContext) {
  const word = ctx.matchBefore(/\\[a-zA-Z@]*\*?(\{[a-zA-Z*]*)?/)
  if (!word || (word.from === word.to && !ctx.explicit)) return null
  return { from: word.from, options: [...envOptions, ...commandOptions], validFor: /^\\[a-zA-Z@]*\*?(\{[a-zA-Z*]*)?$/ }
}

const latexAutocomplete = autocompletion({ override: [latexSource], activateOnTyping: true })

export const isTexPath = (path: string) => /\.(tex|sty|cls|ltx|dtx|bib|bbx|cbx|tikz|def)$/i.test(path)

/** Complétion : commandes et environnements LaTeX, sinon celle du langage. */
export const completionFor = (path: string): Extension => (isTexPath(path) ? latexAutocomplete : autocompletion())
