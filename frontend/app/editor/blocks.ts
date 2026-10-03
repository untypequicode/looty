import { Extension, type Editor } from '@tiptap/core'
import { NodeSelection, TextSelection } from '@tiptap/pm/state'
import type { Node as PMNode } from '@tiptap/pm/model'

/** Nœuds qui peuvent être saisis et déplacés individuellement par la poignée. */
const DRAGGABLE_INNER = new Set(['listItem', 'taskItem'])

export interface BlockRef {
  pos: number
  node: PMNode
  depth: number
}

/** Bloc « déplaçable » contenant la position donnée (élément de liste ou bloc de premier niveau). */
export function blockAt(editor: Editor, pos: number): BlockRef | null {
  const { doc } = editor.state
  if (pos < 0 || pos > doc.content.size) return null
  const $pos = doc.resolve(pos)
  for (let d = $pos.depth; d > 0; d--) {
    const node = $pos.node(d)
    if (DRAGGABLE_INNER.has(node.type.name) || d === 1) return { pos: $pos.before(d), node, depth: d }
  }
  // Position entre deux blocs de premier niveau
  const node = doc.nodeAt(pos)
  return node ? { pos, node, depth: 1 } : null
}

/** Libellé court du type de bloc affiché dans la poignée (H1, ¶, </>, ∑ ...). */
export function blockLabel(node: PMNode): string {
  switch (node.type.name) {
    case 'heading': return `H${node.attrs.level}`
    case 'paragraph': return '¶'
    case 'bulletList': case 'listItem': return '•'
    case 'orderedList': return '1.'
    case 'taskList': case 'taskItem': return '☐'
    case 'codeBlock': return '</>'
    case 'blockquote': return '❝'
    case 'notice': return 'ⓘ'
    case 'mathBlock': return '∑'
    case 'table': return '▦'
    case 'image': return '▣'
    case 'horizontalRule': return '—'
    case 'toggle': return '▸'
    default: return '◇'
  }
}

export function selectBlock(editor: Editor, b: BlockRef) {
  editor.view.dispatch(editor.state.tr.setSelection(NodeSelection.create(editor.state.doc, b.pos)))
}

export function deleteBlock(editor: Editor, b: BlockRef) {
  editor.chain().focus().deleteRange({ from: b.pos, to: b.pos + b.node.nodeSize }).run()
}

export function duplicateBlock(editor: Editor, b: BlockRef) {
  const end = b.pos + b.node.nodeSize
  editor.chain().focus().insertContentAt(end, b.node.toJSON()).run()
}

/** Échange le bloc avec son voisin (dir = -1 au-dessus, 1 en dessous). */
export function moveBlock(editor: Editor, b: BlockRef, dir: -1 | 1): boolean {
  const { state } = editor
  const $pos = state.doc.resolve(b.pos)
  const parent = $pos.parent
  const index = $pos.index()
  const sibling = parent.maybeChild(index + dir)
  if (!sibling) return false
  const tr = state.tr
  if (dir === -1) {
    const from = b.pos - sibling.nodeSize
    tr.delete(b.pos, b.pos + b.node.nodeSize).insert(from, b.node)
    tr.setSelection(NodeSelection.create(tr.doc, from))
  } else {
    const after = b.pos + b.node.nodeSize
    tr.delete(after, after + sibling.nodeSize).insert(b.pos, sibling)
    tr.setSelection(NodeSelection.create(tr.doc, b.pos + sibling.nodeSize))
  }
  editor.view.dispatch(tr.scrollIntoView())
  return true
}

/** « Transformer en » : change le type d'un bloc textuel. */
export function turnInto(editor: Editor, b: BlockRef, kind: string) {
  const inside = b.pos + 1
  const chain = editor.chain().focus().setTextSelection(TextSelection.near(editor.state.doc.resolve(inside)).from)
  switch (kind) {
    case 'paragraph': chain.setParagraph().run(); break
    case 'h1': chain.setHeading({ level: 1 }).run(); break
    case 'h2': chain.setHeading({ level: 2 }).run(); break
    case 'h3': chain.setHeading({ level: 3 }).run(); break
    case 'bullet': chain.toggleBulletList().run(); break
    case 'ordered': chain.toggleOrderedList().run(); break
    case 'todo': chain.toggleTaskList().run(); break
    case 'quote': chain.toggleBlockquote().run(); break
    case 'code': chain.toggleCodeBlock().run(); break
    case 'callout': chain.toggleCallout({ type: 'info' }).run(); break
  }
}

/** Raccourcis : Alt+Shift+↑/↓ déplacent le bloc courant, Mod+D le duplique. */
export const BlockKeymap = Extension.create({
  name: 'blockKeymap',
  addKeyboardShortcuts() {
    const current = () => blockAt(this.editor, this.editor.state.selection.from)
    return {
      'Alt-Shift-ArrowUp': () => { const b = current(); return b ? moveBlock(this.editor, b, -1) : false },
      'Alt-Shift-ArrowDown': () => { const b = current(); return b ? moveBlock(this.editor, b, 1) : false },
      'Mod-d': () => { const b = current(); if (!b) return false; duplicateBlock(this.editor, b); return true },
    }
  },
})
