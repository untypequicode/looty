import { Marked } from 'marked'
import DOMPurify from 'dompurify'
import katex from 'katex'
import { extractMath, injectMath } from './chatMathParse'

// Instance dédiée : le HTML brut du modèle est échappé, jamais interprété.
const md = new Marked({ gfm: true, breaks: true })
md.use({ renderer: { html: ({ text }) => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;') } })

/** Réponse de l'assistant (Markdown) → HTML nettoyé : ni script, ni style, ni image, liens externes sûrs. */
export function renderChatMarkdown(src: string): string {
  // Sans DOM (rendu serveur) DOMPurify ne nettoie rien : on n'émet alors que du texte échappé.
  if (typeof window === 'undefined') return src.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  // Formules fermées mises de côté (jetons) ; le reste est nettoyé, puis KaTeX les rend (trust: false).
  const nonce = 'LTYMATH' + Math.random().toString(36).slice(2, 8).toUpperCase()
  const { text, parts } = extractMath(src, nonce)
  const html = md.parse(text, { async: false }) as string
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
  return injectMath(doc.body.innerHTML, parts, (tex, display) => katex.renderToString(tex, { displayMode: display, throwOnError: true, trust: false, maxExpand: 500 }))
}
