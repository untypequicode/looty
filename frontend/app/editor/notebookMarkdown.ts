import { marked } from 'marked'
import DOMPurify from 'dompurify'
import katex from 'katex'

/** Pièces jointes Jupyter d'une cellule : nom → { type MIME → base64 }. */
export type NbAttachments = Record<string, Record<string, string | string[]>>

export interface MarkdownImages {
  /** Pièces jointes de la cellule (`![](attachment:image.png)`). */
  attachments?: NbAttachments | null
  /** Chemin relatif (image ou lien) → adresse, ou null s'il est inconnu (laissé tel quel). */
  resolve?: ((path: string) => string | null) | null
}

/** Adresse absolue (`https:`, `data:`, `//hôte`, `/chemin`) ou ancre : jamais réécrite. */
const isAbsolute = (u: string) => /^([a-z][a-z0-9+.-]*:|\/|#)/i.test(u)

function attachmentUrl(name: string, attachments: NbAttachments | null | undefined): string | null {
  const bundle = attachments?.[name]
  if (!bundle) return null
  const mime = Object.keys(bundle).find((k) => k.startsWith('image/')) ?? Object.keys(bundle)[0]
  if (!mime) return null
  const v = bundle[mime]
  return `data:${mime};base64,${(Array.isArray(v) ? v.join('') : String(v ?? '')).replace(/\s/g, '')}`
}

/**
 * Texte d'une cellule Markdown → HTML : Markdown (avec formules $…$ et $$…$$ rendues par
 * KaTeX), puis nettoyage (aucun script, aucun style global) ; les formules, produites par
 * KaTeX, sont insérées après le nettoyage. Les images `attachment:` et les chemins relatifs
 * (fichiers du projet) sont remplacés par leur adresse.
 */
export function renderMarkdown(src: string, images: MarkdownImages = {}): string {
  const math: string[] = []
  const keep = (tex: string, displayMode: boolean) => {
    math.push(katex.renderToString(tex, { displayMode, throwOnError: false, output: 'html' }))
    return `@@MATH${math.length - 1}@@`
  }
  let s = src.replace(/\$\$([\s\S]+?)\$\$/g, (_, m) => keep(m, true))
  s = s.replace(/(?<![\\$])\$(?!\s)([^$\n]+?)(?<!\s)\$(?!\d)/g, (_, m) => keep(m, false))
  const html = marked.parse(s, { async: false, gfm: true, breaks: false }) as string
  // Avant le contrôle des adresses : « attachment: » serait retiré, l'adresse finale est vérifiée.
  DOMPurify.addHook('beforeSanitizeAttributes', (node) => {
    const el = node as Element
    if (el.tagName === 'IMG') {
      const raw = el.getAttribute('src') ?? ''
      if (/^attachment:/i.test(raw)) {
        const url = attachmentUrl(decodeURIComponent(raw.slice('attachment:'.length)), images.attachments)
        if (url) el.setAttribute('src', url)
        else el.removeAttribute('src')
      } else if (raw && !isAbsolute(raw) && images.resolve) {
        const url = images.resolve(raw)
        if (url) el.setAttribute('src', url)
      }
    } else if (el.tagName === 'A') {
      const raw = el.getAttribute('href') ?? ''
      if (raw && !isAbsolute(raw) && images.resolve) {
        const url = images.resolve(raw)
        if (url) { el.setAttribute('href', url); el.setAttribute('target', '_blank'); el.setAttribute('rel', 'noopener') }
      }
    }
  })
  let clean: string
  try {
    clean = DOMPurify.sanitize(html, { USE_PROFILES: { html: true }, FORBID_TAGS: ['style', 'form', 'input', 'button'], ADD_ATTR: ['target'] })
  } finally {
    DOMPurify.removeHook('beforeSanitizeAttributes')
  }
  return clean.replace(/@@MATH(\d+)@@/g, (_, i) => math[Number(i)] ?? '')
}

/** Chemin normalisé (`a/./b/../c` → `a/c`), ou null s'il sort du projet. */
export function normalizePath(p: string): string | null {
  const out: string[] = []
  for (const seg of p.split('/')) {
    if (!seg || seg === '.') continue
    if (seg === '..') { if (!out.length) return null; out.pop() } else out.push(seg)
  }
  return out.length ? out.join('/') : null
}
export const dirOf = (path: string) => (path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '')

/**
 * Chemins candidats d'un lien relatif écrit dans un notebook : depuis le dossier du
 * notebook, puis depuis la racine du projet (comme Jupyter).
 */
export function candidatePaths(notebookPath: string, href: string): string[] {
  let rel = href.replace(/[?#].*$/, '')
  try { rel = decodeURIComponent(rel) } catch { /* adresse mal encodée : telle quelle */ }
  const dir = dirOf(notebookPath)
  const out = [normalizePath(dir ? `${dir}/${rel}` : rel), normalizePath(rel)].filter((x): x is string => !!x)
  return [...new Set(out)]
}

/**
 * Résolution des chemins relatifs d'un notebook. Avec la liste des fichiers du projet : le
 * premier candidat qui existe ; sans liste (lien public) : `url` reçoit tous les candidats.
 */
export function projectResolver(notebookPath: string, files: Iterable<string> | null, url: (paths: string[]) => string | null) {
  const known = files ? new Set(files) : null
  return (href: string) => {
    const cands = candidatePaths(notebookPath, href)
    if (!cands.length) return null
    if (!known) return url(cands)
    const hit = cands.find((c) => known.has(c))
    return hit ? url([hit]) : null
  }
}

/** Fichier image affichable (mêmes extensions que les images du lien public). */
export const IMAGE_EXT = /\.(png|jpe?g|gif|webp|svg)$/i

/** HTML produit par du code (tableaux pandas…) : nettoyé, sans style ni script. */
export function sanitizeOutputHtml(html: string): string {
  return DOMPurify.sanitize(html, { USE_PROFILES: { html: true }, FORBID_TAGS: ['style', 'form', 'input', 'button', 'iframe', 'object', 'embed'] })
}

/** Retire les séquences de couleur ANSI des messages d'erreur. */
export function stripAnsi(s: string): string {
  // eslint-disable-next-line no-control-regex
  return s.replace(/\u001b\[[0-9;]*m/g, '')
}
