import { Marked, type Tokens, type TokenizerAndRendererExtension } from 'marked'
import DOMPurify from 'dompurify'
import katex from 'katex'
import { extractMath, injectMath, type MathPart } from './chatMathParse.ts'

/**
 * Markdown riche partagé (chat de l'assistant, entraînements, aperçus) : Markdown GFM, formules
 * KaTeX, encadrés `:::info`, listes de tâches en lecture seule, tableaux, blocs de code. Les
 * blocs `mermaid` et `looty-graph` fermés sortent en emplacements `.rich-diagram` que le
 * composant `RichMarkdown` dessine ensuite, à la demande (voir `richMarkdown.ts`).
 */

export const CALLOUTS = ['info', 'success', 'warning', 'tip'] as const
export const DIAGRAM_LANGS = { mermaid: 'mermaid', 'looty-graph': 'graph' } as const
export interface RichOptions {
  /** Pas de bloc englobant (`<p>`) : choix de réponse, énoncé court. */
  inline?: boolean
}

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

// Formules à rendre trouvées pendant l'analyse en cours (l'analyse est synchrone).
let current: { nonce: string; parts: MathPart[] } | null = null

/** Un bloc de code est « fermé » quand sa clôture est présente : pendant le flux il reste du texte. */
function fenceClosed(raw: string): boolean {
  const lines = raw.replace(/\s+$/, '').split('\n')
  if (lines.length < 2) return false
  const open = /^ {0,3}(`{3,}|~{3,})/.exec(lines[0]!)
  const last = /^ {0,3}(`{3,}|~{3,})[ \t]*$/.exec(lines[lines.length - 1]!)
  return !!open && !!last && last[1]![0] === open[1]![0] && last[1]!.length >= open[1]!.length
}

/** Liens : seuls http(s), mailto, ancres et chemins relatifs survivent (jamais `javascript:`). */
export function safeHref(href: string): boolean {
  const h = href.trim().replace(/[\u0000- ]/g, '')
  return /^(https?:|mailto:|#|\/(?!\/)|\.{0,2}\/)/i.test(h) || !/^[a-z][a-z0-9+.-]*:/i.test(h)
}

const calloutExt: TokenizerAndRendererExtension = {
  name: 'callout',
  level: 'block',
  start: (src) => src.match(/(?:^|\n)(:::(?:info|success|warning|tip)[ \t]*\n)/)?.index,
  tokenizer(this: any, src: string) {
    const m = new RegExp(`^:::(${CALLOUTS.join('|')})[ \\t]*\\n((?:[\\s\\S]*?\\n)?):::[ \\t]*(?:\\n|$)`).exec(src)
    if (!m) return undefined
    const token = { type: 'callout', raw: m[0], kind: m[1]!, tokens: [] as unknown[] }
    this.lexer.blockTokens(m[2]!, token.tokens)
    return token
  },
  renderer(this: any, token: any) {
    return `<div class="callout callout-${token.kind}">${this.parser.parse(token.tokens)}</div>\n`
  },
}

// Instance dédiée : le HTML brut du modèle est échappé, jamais interprété.
const md = new Marked({ gfm: true, breaks: true })
md.use({
  extensions: [calloutExt],
  renderer: {
    html: ({ text }) => esc(text),
    image: ({ text }) => esc(text),
    checkbox: ({ checked }) => `<span class="rich-check" role="checkbox" aria-checked="${checked}" aria-readonly="true"></span> `,
    link(this: any, { href, title, tokens }: Tokens.Link) {
      const inner = this.parser.parseInline(tokens)
      if (!safeHref(href)) return inner
      return `<a href="${esc(href)}"${title ? ` title="${esc(title)}"` : ''}>${inner}</a>`
    },
    code({ text, lang, raw }: Tokens.Code) {
      const name = (lang ?? '').trim().split(/\s+/)[0]!.toLowerCase()
      if (fenceClosed(raw)) {
        const kind = DIAGRAM_LANGS[name as keyof typeof DIAGRAM_LANGS]
        if (kind) return `<div class="rich-diagram" data-kind="${kind}"><pre><code>${esc(text)}</code></pre></div>\n`
        if (name === 'math' && current) {
          const placeholder = `${current.nonce}${current.parts.length}X`
          current.parts.push({ placeholder, tex: text.trim(), display: true })
          return `<div class="rich-math">${placeholder}</div>\n`
        }
      }
      return `<pre><code${name ? ` class="language-${esc(name.replace(/[^a-z0-9_+#.-]/g, ''))}"` : ''}>${esc(text)}\n</code></pre>\n`
    },
  },
})

/** Markdown → HTML non nettoyé + formules à injecter. Sans DOM : testable seul. */
export function parseChatMarkdown(src: string, opts: RichOptions = {}): { html: string; parts: MathPart[] } {
  const nonce = 'LTYMATH' + Math.random().toString(36).slice(2, 8).toUpperCase()
  const { text, parts } = extractMath(src, nonce)
  const prev = current
  current = { nonce, parts }
  try {
    const html = (opts.inline ? md.parseInline(text, { async: false }) : md.parse(text, { async: false })) as string
    return { html, parts }
  } finally {
    current = prev
  }
}

export const renderKatex = (tex: string, display: boolean) => katex.renderToString(tex, { displayMode: display, throwOnError: true, trust: false, maxExpand: 500 })

/** Réponse de l'assistant (Markdown) → HTML nettoyé : ni script, ni style, ni image, liens externes sûrs. */
export function renderChatMarkdown(src: string, opts: RichOptions = {}): string {
  // Sans DOM (rendu serveur) DOMPurify ne nettoie rien : on n'émet alors que du texte échappé.
  if (typeof window === 'undefined') return src.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  // Formules fermées mises de côté (jetons) ; le reste est nettoyé, puis KaTeX les rend (trust: false).
  const { html, parts } = parseChatMarkdown(src, opts)
  const clean = DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['style', 'form', 'input', 'button', 'img', 'iframe'],
    FORBID_ATTR: ['style'],
    ADD_ATTR: ['target'],
  })
  const doc = new DOMParser().parseFromString(clean, 'text/html')
  doc.querySelectorAll('a').forEach((a) => {
    a.setAttribute('target', '_blank')
    a.setAttribute('rel', 'noopener noreferrer')
  })
  return injectMath(doc.body.innerHTML, parts, renderKatex)
}
