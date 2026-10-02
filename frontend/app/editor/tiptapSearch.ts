import { Extension } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'

export const searchPluginKey = new PluginKey('search')

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    search: {
      setSearchTerm: (searchTerm: string) => ReturnType
    }
  }
}

export const SearchExtension = Extension.create({
  name: 'search',

  addCommands() {
    return {
      setSearchTerm: (searchTerm: string) => ({ tr, dispatch }) => {
        if (dispatch) {
          tr.setMeta(searchPluginKey, searchTerm)
        }
        return true
      },
    }
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: searchPluginKey,
        state: {
          init() {
            return {
              searchTerm: '',
              decorations: DecorationSet.empty,
            }
          },
          apply(tr, oldState) {
            const newSearchTerm = tr.getMeta(searchPluginKey)
            const searchTerm = newSearchTerm !== undefined ? newSearchTerm : oldState.searchTerm

            if (searchTerm === oldState.searchTerm && !tr.docChanged) {
              return {
                searchTerm,
                decorations: oldState.decorations.map(tr.mapping, tr.doc),
              }
            }

            if (!searchTerm) {
              return {
                searchTerm,
                decorations: DecorationSet.empty,
              }
            }

            const decorations: Decoration[] = []
            const search = searchTerm.toLowerCase()
            
            tr.doc.descendants((node, pos) => {
              if (node.isText) {
                const nodeText = node.text?.toLowerCase() || ''
                let idx = nodeText.indexOf(search)
                while (idx !== -1) {
                  decorations.push(
                    Decoration.inline(pos + Math.max(0, idx), pos + Math.max(0, idx) + search.length, {
                      class: 'search-highlight',
                    })
                  )
                  idx = nodeText.indexOf(search, idx + search.length)
                }
              }
            })
            return {
              searchTerm,
              decorations: DecorationSet.create(tr.doc, decorations),
            }
          },
        },
        props: {
          decorations(state) {
            return this.getState(state).decorations
          },
        },
      }),
    ]
  },
})
