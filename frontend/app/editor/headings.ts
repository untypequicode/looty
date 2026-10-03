import { Extension } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'
import type { Node as PMNode } from '@tiptap/pm/model'

export interface HeadingInfo {
  id: string
  level: number
  text: string
  pos: number
}

export function slugify(text: string): string {
  return (
    text
      .normalize('NFD')
      .replace(/[̀-ͯ]/g, '')
      .toLowerCase()
      .replace(/[^a-z0-9\s-]/g, '')
      .trim()
      .replace(/\s+/g, '-')
      .slice(0, 80) || 'section'
  )
}

/** Titres du document avec identifiants uniques (« intro », « intro-2 »...). */
export function collectHeadings(doc: PMNode): HeadingInfo[] {
  const out: HeadingInfo[] = []
  const seen = new Map<string, number>()
  doc.descendants((node, pos) => {
    if (node.type.name !== 'heading') return
    const base = slugify(node.textContent)
    const n = (seen.get(base) ?? 0) + 1
    seen.set(base, n)
    out.push({ id: n > 1 ? `${base}-${n}` : base, level: node.attrs.level, text: node.textContent, pos })
    return false
  })
  return out
}

export const headingsKey = new PluginKey<{ headings: HeadingInfo[]; decorations: DecorationSet }>('headingAnchors')

/**
 * Ancres de titres : les `id` sont posés par décoration (pas dans le document
 * collaboratif) pour permettre les liens `/doc/…#section`.
 */
export const HeadingAnchors = Extension.create({
  name: 'headingAnchors',
  addProseMirrorPlugins() {
    const build = (doc: PMNode) => {
      const headings = collectHeadings(doc)
      const decorations = DecorationSet.create(
        doc,
        headings.map((h) => {
          const node = doc.nodeAt(h.pos)!
          return Decoration.node(h.pos, h.pos + node.nodeSize, { id: h.id, 'data-anchor': h.id })
        }),
      )
      return { headings, decorations }
    }
    return [
      new Plugin({
        key: headingsKey,
        state: {
          init: (_, state) => build(state.doc),
          apply: (tr, value) => (tr.docChanged ? build(tr.doc) : value),
        },
        props: {
          decorations: (state) => headingsKey.getState(state)?.decorations,
        },
      }),
    ]
  },
})
