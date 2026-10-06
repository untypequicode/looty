import { Mark, mergeAttributes, type Editor } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'

/**
 * Passage commenté dans une page : `span.comment-mark[data-comment-id]`. Le surlignage est
 * appliqué par la page pour les fils non résolus (les fils résolus ou supprimés laissent une
 * marque invisible). Un clic sur le passage ouvre le fil correspondant.
 */
export const CommentMark = Mark.create({
  name: 'comment',
  inclusive: false,
  excludes: '',
  addAttributes() {
    return {
      id: { default: null, parseHTML: (el) => el.getAttribute('data-comment-id'), renderHTML: (a) => ({ 'data-comment-id': a.id }) },
    }
  },
  parseHTML() {
    return [{ tag: 'span[data-comment-id]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(HTMLAttributes, { class: 'comment-mark' }), 0]
  },
  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey('commentClick'),
        props: {
          handleClick(_view, _pos, event) {
            const el = (event.target as HTMLElement | null)?.closest?.('[data-comment-id]')
            const id = el?.getAttribute('data-comment-id')
            if (id && id !== 'pending') window.dispatchEvent(new CustomEvent('looty:comment-open', { detail: { id } }))
            return false
          },
        },
      }),
    ]
  },
})

/** Remplace (ou retire si `to` est nul) la marque d'un fil dans tout le document. */
export function replaceCommentMark(editor: Editor, from: string, to: string | null) {
  const type = editor.schema.marks.comment
  if (!type) return
  const tr = editor.state.tr
  editor.state.doc.descendants((node, pos) => {
    if (!node.isText) return
    for (const m of node.marks) {
      if (m.type === type && m.attrs.id === from) {
        tr.removeMark(pos, pos + node.nodeSize, m)
        if (to) tr.addMark(pos, pos + node.nodeSize, type.create({ id: to }))
      }
    }
  })
  if (tr.docChanged) editor.view.dispatch(tr.setMeta('addToHistory', false))
}

/** Position du premier passage d'un fil (pour y faire défiler le document). */
export function findCommentMark(editor: Editor, id: string): number | null {
  let found: number | null = null
  editor.state.doc.descendants((node, pos) => {
    if (found !== null) return false
    if (node.isText && node.marks.some((m) => m.type.name === 'comment' && m.attrs.id === id)) found = pos
  })
  return found
}
