import { Node, Extension, mergeAttributes } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Fragment } from '@tiptap/pm/model'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import { nodeSource, sourceJSON, sourceNodeView, sourceSpec, sourceText } from './source'
import MermaidView from '~/components/editor/MermaidView.vue'
import ExcalidrawView from '~/components/editor/ExcalidrawView.vue'
import EmbedView from '~/components/editor/EmbedView.vue'
import PdfView from '~/components/editor/PdfView.vue'

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    embeds: {
      insertEmbed: (src: string) => ReturnType
      insertMermaid: (code?: string) => ReturnType
      insertExcalidraw: () => ReturnType
      insertPdf: (src: string, name?: string) => ReturnType
    }
  }
}

export interface EmbedInfo {
  provider: string
  embedUrl: string
  ratio: string
}

/**
 * Fournisseurs autorisés pour les iframes. Seules ces URL produisent une intégration :
 * aucune page arbitraire n'est chargée dans le document.
 */
const PROVIDERS: { name: string; match: RegExp; to: (m: RegExpMatchArray, url: URL) => string; ratio?: string }[] = [
  { name: 'YouTube', match: /^https:\/\/(?:www\.)?(?:youtube\.com\/watch\?.*v=|youtu\.be\/|youtube\.com\/shorts\/)([\w-]{11})/, to: (m) => `https://www.youtube-nocookie.com/embed/${m[1]}` },
  { name: 'Vimeo', match: /^https:\/\/(?:www\.)?vimeo\.com\/(\d+)/, to: (m) => `https://player.vimeo.com/video/${m[1]}` },
  { name: 'Loom', match: /^https:\/\/(?:www\.)?loom\.com\/share\/([\w]+)/, to: (m) => `https://www.loom.com/embed/${m[1]}` },
  { name: 'Figma', match: /^https:\/\/(?:www\.)?figma\.com\/(?:file|design|proto|board)\/[\w]+/, to: (_m, u) => `https://www.figma.com/embed?embed_host=looty&url=${encodeURIComponent(u.href)}`, ratio: '4 / 3' },
  { name: 'CodePen', match: /^https:\/\/codepen\.io\/([\w-]+)\/pen\/([\w]+)/, to: (m) => `https://codepen.io/${m[1]}/embed/${m[2]}?default-tab=result`, ratio: '4 / 3' },
  { name: 'Google Docs', match: /^https:\/\/docs\.google\.com\/(document|spreadsheets|presentation)\/d\/([\w-]+)/, to: (m) => `https://docs.google.com/${m[1]}/d/${m[2]}/${m[1] === 'presentation' ? 'embed' : 'preview'}`, ratio: '4 / 3' },
  { name: 'Miro', match: /^https:\/\/miro\.com\/app\/board\/([\w=-]+)/, to: (m) => `https://miro.com/app/live-embed/${m[1]}/`, ratio: '4 / 3' },
]

export function resolveEmbed(src: string): EmbedInfo | null {
  let url: URL
  try { url = new URL(src) } catch { return null }
  if (url.protocol !== 'https:') return null
  for (const p of PROVIDERS) {
    const m = src.match(p.match)
    if (m) return { provider: p.name, embedUrl: p.to(m, url), ratio: p.ratio ?? '16 / 9' }
  }
  return null
}

export const Embed = Node.create({
  name: 'embed',
  group: 'block',
  atom: true,
  draggable: true,
  addAttributes() {
    return { src: { default: '' } }
  },
  parseHTML() {
    return [{ tag: 'div[data-type="embed"]', getAttrs: (el) => ({ src: (el as HTMLElement).getAttribute('data-src') }) }]
  },
  renderHTML({ HTMLAttributes, node }) {
    return ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'embed', 'data-src': node.attrs.src }), ['a', { href: node.attrs.src }, node.attrs.src]]
  },
  addNodeView() {
    return VueNodeViewRenderer(EmbedView)
  },
  addCommands() {
    return {
      insertEmbed: (src) => ({ commands }) => (resolveEmbed(src) ? commands.insertContent({ type: this.name, attrs: { src } }) : false),
      insertMermaid: (code = '') => ({ commands }) => commands.insertContent({ type: 'mermaid', content: sourceJSON(code) }),
      insertExcalidraw: () => ({ commands }) => commands.insertContent({ type: 'excalidraw', attrs: { scene: '' } }),
      insertPdf: (src, name) => ({ commands }) => commands.insertContent({ type: 'pdfEmbed', attrs: { src, name: name ?? '' } }),
    }
  },
})

export const Mermaid = Node.create({
  name: 'mermaid',
  group: 'block',
  // Le code du diagramme est le texte du nœud : modifications fusionnées en collaboration.
  ...sourceSpec,
  draggable: true,
  addAttributes() {
    // Ancien attribut (documents pas encore convertis) : lu, jamais écrit dans le HTML.
    return { code: { default: '', parseHTML: () => '', rendered: false } }
  },
  parseHTML() {
    return [{ tag: 'div[data-type="mermaid"]', getContent: (el, schema) => Fragment.from(sourceText(schema, typeof el === 'string' ? '' : el.textContent ?? '')) }]
  },
  renderHTML({ HTMLAttributes, node }) {
    return ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'mermaid' }), ['pre', {}, nodeSource(node)]]
  },
  addNodeView() {
    return sourceNodeView(MermaidView)
  },
})

export const Excalidraw = Node.create({
  name: 'excalidraw',
  group: 'block',
  atom: true,
  draggable: true,
  addAttributes() {
    return { scene: { default: '' } }
  },
  parseHTML() {
    return [{ tag: 'div[data-type="excalidraw"]', getAttrs: (el) => ({ scene: (el as HTMLElement).getAttribute('data-scene') ?? '' }) }]
  },
  renderHTML({ HTMLAttributes, node }) {
    return ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'excalidraw', 'data-scene': node.attrs.scene })]
  },
  addNodeView() {
    return VueNodeViewRenderer(ExcalidrawView)
  },
})

export const PdfEmbed = Node.create({
  name: 'pdfEmbed',
  group: 'block',
  atom: true,
  draggable: true,
  addAttributes() {
    return { src: { default: '' }, name: { default: '' } }
  },
  parseHTML() {
    return [{ tag: 'div[data-type="pdf"]', getAttrs: (el) => ({ src: (el as HTMLElement).getAttribute('data-src'), name: (el as HTMLElement).getAttribute('data-name') }) }]
  },
  renderHTML({ HTMLAttributes, node }) {
    return ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'pdf', 'data-src': node.attrs.src, 'data-name': node.attrs.name }), ['a', { href: node.attrs.src }, node.attrs.name || 'PDF']]
  },
  addNodeView() {
    return VueNodeViewRenderer(PdfView)
  },
})

/** Coller un lien seul sur une ligne vide : intégration automatique s'il est reconnu. */
export const EmbedPaste = Extension.create({
  name: 'embedPaste',
  addProseMirrorPlugins() {
    const editor = this.editor
    return [
      new Plugin({
        key: new PluginKey('embedPaste'),
        props: {
          handlePaste(view, event) {
            const text = event.clipboardData?.getData('text/plain')?.trim()
            if (!text || /\s/.test(text)) return false
            const { $from, empty } = view.state.selection
            if (!empty || $from.parent.type.name !== 'paragraph' || $from.parent.content.size > 0) return false
            if (/^\/uploads\/[\w.-]+\.pdf$/i.test(text) || (/^https:\/\/.+\.pdf$/i.test(text) && text.startsWith(location.origin))) {
              return editor.commands.insertPdf(text)
            }
            if (!resolveEmbed(text)) return false
            return editor.commands.insertEmbed(text)
          },
        },
      }),
    ]
  },
})

export const embedExtensions = [Embed, Mermaid, Excalidraw, PdfEmbed]
