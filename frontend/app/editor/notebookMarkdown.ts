import { marked } from 'marked'
import DOMPurify from 'dompurify'
import katex from 'katex'

/**
 * Texte d'une cellule Markdown → HTML : Markdown (avec formules $…$ et $$…$$ rendues par
 * KaTeX), puis nettoyage (aucun script, aucun style global) ; les formules, produites par
 * KaTeX, sont insérées après le nettoyage.
 */
export function renderMarkdown(src: string): string {
  const math: string[] = []
  const keep = (tex: string, displayMode: boolean) => {
    math.push(katex.renderToString(tex, { displayMode, throwOnError: false, output: 'html' }))
    return `@@MATH${math.length - 1}@@`
  }
  let s = src.replace(/\$\$([\s\S]+?)\$\$/g, (_, m) => keep(m, true))
  s = s.replace(/(?<![\\$])\$(?!\s)([^$\n]+?)(?<!\s)\$(?!\d)/g, (_, m) => keep(m, false))
  const html = marked.parse(s, { async: false, gfm: true, breaks: false }) as string
  const clean = DOMPurify.sanitize(html, { USE_PROFILES: { html: true }, FORBID_TAGS: ['style', 'form', 'input', 'button'], ADD_ATTR: ['target'] })
  return clean.replace(/@@MATH(\d+)@@/g, (_, i) => math[Number(i)] ?? '')
}

/** HTML produit par du code (tableaux pandas…) : nettoyé, sans style ni script. */
export function sanitizeOutputHtml(html: string): string {
  return DOMPurify.sanitize(html, { USE_PROFILES: { html: true }, FORBID_TAGS: ['style', 'form', 'input', 'button', 'iframe', 'object', 'embed'] })
}

/** Retire les séquences de couleur ANSI des messages d'erreur. */
export function stripAnsi(s: string): string {
  // eslint-disable-next-line no-control-regex
  return s.replace(/\u001b\[[0-9;]*m/g, '')
}
