import { renderMermaid } from './mermaid'
import { sanitizeSvg } from './sanitizeSvg'
import { renderGraphImage, MAX_GRAPH_SOURCE } from './graphCore'
import type { DiagramStyle } from './diagramStyles'

/**
 * Passe « riche » appliquée au HTML déjà nettoyé de `renderChatMarkdown`, une fois posé dans le
 * DOM : coloration du code, diagrammes Mermaid, graphes. Tout est chargé à la demande ; un
 * résultat déjà calculé est remis aussitôt (le HTML est reconstruit à chaque morceau du flux).
 * Un diagramme invalide garde sa source avec une petite note d'erreur.
 */

export interface EnhanceOptions {
  dark: boolean
  style: DiagramStyle
  /** Texte de la note quand un diagramme ne se dessine pas. */
  errorLabel: string
  /** Note quand la source d'un graphe dépasse la limite (elle reste affichée). */
  tooLargeLabel?: string
}

type Done = { kind: 'svg'; svg: string } | { kind: 'img'; src: string } | { kind: 'error'; message: string }
const done = new Map<string, Done>()
const MAX = 200
function remember(key: string, v: Done) {
  if (done.size >= MAX) done.delete(done.keys().next().value!)
  done.set(key, v)
}

function showError(box: HTMLElement, message: string, label: string) {
  box.querySelector('.rich-err')?.remove()
  const note = document.createElement('small')
  note.className = 'rich-err'
  note.textContent = `${label}${message ? ` — ${message}` : ''}`
  box.append(note)
}

function show(box: HTMLElement, d: Done, o: EnhanceOptions) {
  if (!box.isConnected) return
  if (d.kind === 'error') return showError(box, d.message, o.errorLabel)
  const pal = o.dark ? o.style.dark : o.style.light
  const out = document.createElement('div')
  out.className = 'rich-render'
  out.style.background = pal.background
  if (d.kind === 'svg') out.innerHTML = d.svg
  else {
    const img = document.createElement('img')
    img.src = d.src
    img.alt = ''
    out.append(img)
    // Image agrandissable au clic (voir Markdown.vue) ; la source sert à « copier la source ».
    out.dataset.graphSource = box.querySelector('code')?.textContent ?? ''
    img.tabIndex = 0
    img.setAttribute('role', 'button')
    img.classList.add('rich-zoom')
  }
  box.replaceChildren(out)
}

function diagram(box: HTMLElement, o: EnhanceOptions) {
  const kind = box.getAttribute('data-kind')
  const source = box.querySelector('code')?.textContent ?? ''
  if (!source.trim() || (kind !== 'mermaid' && kind !== 'graph')) return
  const key = `${kind}|${o.dark ? 'd' : 'l'}|${o.style.id}|${source}`
  const hit = done.get(key)
  if (hit) return show(box, hit, o)
  if (source.length > MAX_GRAPH_SOURCE) return showError(box, '', kind === 'graph' && o.tooLargeLabel ? o.tooLargeLabel : o.errorLabel)
  const job: Promise<Done> = kind === 'mermaid'
    ? renderMermaid(`rich-${Math.random().toString(36).slice(2)}`, source, o.dark, o.style).then((r) => ({ kind: 'svg' as const, svg: sanitizeSvg(r.svg) }))
    : renderGraphImage(source, o.dark, o.style).then((src) => ({ kind: 'img' as const, src }))
  job.then(
    (d) => { remember(key, d); show(box, d, o) },
    (e) => {
      const message = String(e?.message ?? e).split('\n').slice(0, 2).join(' ').slice(0, 160)
      // Mermaid laisse un élément d'erreur dans la page quand l'analyse échoue.
      document.querySelectorAll('[id^="drich-"]').forEach((n) => n.remove())
      const d: Done = { kind: 'error', message }
      remember(key, d)
      show(box, d, o)
    },
  )
}

const codeDone = new Map<string, string>()
type Lowlight = { highlight(lang: string, code: string): { children: any[] }; registered(lang: string): boolean }
let lowlight: Promise<Lowlight> | null = null
const loadLowlight = () => (lowlight ??= import('lowlight').then(({ createLowlight, common }) => createLowlight(common) as unknown as Lowlight))

/** Arbre hast de lowlight → DOM (texte et classes seulement : rien d'exécutable par construction). */
function hastToNodes(children: any[]): Node[] {
  return children.map((c) => {
    if (c.type === 'text') return document.createTextNode(String(c.value))
    const el = document.createElement('span')
    const cls = c.properties?.className
    if (Array.isArray(cls)) el.className = cls.filter((x: unknown) => typeof x === 'string' && /^[\w-]+$/.test(x)).join(' ')
    el.append(...hastToNodes(c.children ?? []))
    return el
  })
}

function code(el: HTMLElement) {
  const lang = /(?:^|\s)language-([\w+#.-]+)/.exec(el.className)?.[1]
  const text = el.textContent ?? ''
  if (!lang || !text.trim() || text.length > 20000) return
  const key = `${lang}\0${text}`
  const hit = codeDone.get(key)
  if (hit != null) { el.innerHTML = hit; el.classList.add('hljs'); return }
  loadLowlight().then((ll) => {
    if (!el.isConnected || !ll.registered(lang)) return
    const wrap = document.createElement('div')
    wrap.append(...hastToNodes(ll.highlight(lang, text).children))
    if (codeDone.size >= MAX) codeDone.delete(codeDone.keys().next().value!)
    codeDone.set(key, wrap.innerHTML)
    el.replaceChildren(...wrap.childNodes)
    el.classList.add('hljs')
  }).catch(() => { /* coloration indisponible : le code reste en texte */ })
}

/** Colore le code et dessine les diagrammes sous `root` (HTML issu de `renderChatMarkdown`). */
export function enhanceRich(root: HTMLElement, o: EnhanceOptions) {
  root.querySelectorAll<HTMLElement>('.rich-diagram').forEach((box) => diagram(box, o))
  root.querySelectorAll<HTMLElement>('pre > code').forEach((el) => { if (!el.closest('.rich-diagram')) code(el) })
}
