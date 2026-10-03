import { Extension } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'
import type { Node as PMNode } from '@tiptap/pm/model'

export interface SearchState {
  term: string
  matches: { from: number; to: number }[]
  index: number
  decorations: DecorationSet
}

export const searchKey = new PluginKey<SearchState>('findInPage')

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    findInPage: {
      setSearchTerm: (term: string) => ReturnType
      gotoMatch: (dir: 1 | -1) => ReturnType
    }
  }
}

function findAll(doc: PMNode, term: string) {
  const out: { from: number; to: number }[] = []
  if (!term) return out
  const q = term.toLowerCase()
  doc.descendants((node, pos) => {
    if (!node.isText) return
    const text = node.text!.toLowerCase()
    let i = text.indexOf(q)
    while (i !== -1) {
      out.push({ from: pos + i, to: pos + i + q.length })
      i = text.indexOf(q, i + q.length)
    }
  })
  return out
}

function decorate(doc: PMNode, matches: SearchState['matches'], index: number) {
  return DecorationSet.create(
    doc,
    matches.map((m, i) => Decoration.inline(m.from, m.to, { class: i === index ? 'search-highlight search-highlight-current' : 'search-highlight' })),
  )
}

/** Recherche dans la page avec navigation entre les occurrences (Ctrl+F). */
export const FindInPage = Extension.create({
  name: 'findInPage',
  addCommands() {
    return {
      setSearchTerm: (term) => ({ tr, dispatch }) => {
        dispatch?.(tr.setMeta(searchKey, { term }))
        return true
      },
      gotoMatch: (dir) => ({ tr, state, dispatch, view }) => {
        const s = searchKey.getState(state)
        if (!s?.matches.length) return false
        const index = (s.index + dir + s.matches.length) % s.matches.length
        dispatch?.(tr.setMeta(searchKey, { index }))
        requestAnimationFrame(() => {
          const el = view.dom.querySelector('.search-highlight-current')
          el?.scrollIntoView({ block: 'center', behavior: 'smooth' })
        })
        return true
      },
    }
  },
  addProseMirrorPlugins() {
    return [
      new Plugin<SearchState>({
        key: searchKey,
        state: {
          init: () => ({ term: '', matches: [], index: 0, decorations: DecorationSet.empty }),
          apply(tr, prev) {
            const meta = tr.getMeta(searchKey) as Partial<SearchState> | undefined
            const term = meta?.term ?? prev.term
            if (!meta && !tr.docChanged) return prev
            const matches = meta?.term !== undefined || tr.docChanged ? findAll(tr.doc, term) : prev.matches
            let index = meta?.index ?? (meta?.term !== undefined ? 0 : prev.index)
            if (index >= matches.length) index = 0
            return { term, matches, index, decorations: decorate(tr.doc, matches, index) }
          },
        },
        props: { decorations: (state) => searchKey.getState(state)?.decorations },
      }),
    ]
  },
})
