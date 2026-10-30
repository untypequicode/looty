import { Node, mergeAttributes } from '@tiptap/core'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import BoardBlockView from '~/components/editor/BoardBlockView.vue'
import AgendaBlockView from '~/components/editor/AgendaBlockView.vue'

/**
 * Blocs reliés au reste de Looty (façon bases de données d'AFFiNE) :
 * - `boardBlock` : un vrai tableau affiché dans la page (kanban ou liste), modifiable sur place ;
 * - `agendaBlock` : les événements et échéances des prochains jours.
 * Seule la référence est dans le document ; le contenu vient de l'espace, à jour en direct.
 */
declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    lootyBlocks: {
      insertBoardBlock: (boardId?: string) => ReturnType
      insertAgendaBlock: () => ReturnType
    }
  }
}

export const BoardBlock = Node.create({
  name: 'boardBlock',
  group: 'block',
  atom: true,
  draggable: true,
  addAttributes() {
    return {
      boardId: { default: '' },
      view: { default: 'kanban' },
      filter: { default: 'all' },
    }
  },
  parseHTML() {
    return [{
      tag: 'div[data-type="board-block"]',
      getAttrs: (el) => ({
        boardId: (el as HTMLElement).getAttribute('data-board') ?? '',
        view: (el as HTMLElement).getAttribute('data-view') ?? 'kanban',
        filter: (el as HTMLElement).getAttribute('data-filter') ?? 'all',
      }),
    }]
  },
  renderHTML({ HTMLAttributes, node }) {
    return ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'board-block', 'data-board': node.attrs.boardId, 'data-view': node.attrs.view, 'data-filter': node.attrs.filter })]
  },
  addNodeView() {
    // Glisser-déposer et saisie dans le bloc : gérés par le tableau, pas par l'éditeur.
    return VueNodeViewRenderer(BoardBlockView, { stopEvent: ({ event }) => !(event.target as HTMLElement | null)?.closest?.('[data-drag-handle]') })
  },
  addCommands() {
    return {
      insertBoardBlock: (boardId = '') => ({ commands }) => commands.insertContent({ type: this.name, attrs: { boardId } }),
      insertAgendaBlock: () => ({ commands }) => commands.insertContent({ type: 'agendaBlock' }),
    }
  },
})

export const AgendaBlock = Node.create({
  name: 'agendaBlock',
  group: 'block',
  atom: true,
  draggable: true,
  addAttributes() {
    return {
      days: { default: 7 },
      scope: { default: 'all' },
    }
  },
  parseHTML() {
    return [{
      tag: 'div[data-type="agenda-block"]',
      getAttrs: (el) => ({
        days: Number((el as HTMLElement).getAttribute('data-days') ?? 7) || 7,
        scope: (el as HTMLElement).getAttribute('data-scope') ?? 'all',
      }),
    }]
  },
  renderHTML({ HTMLAttributes, node }) {
    return ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'agenda-block', 'data-days': String(node.attrs.days), 'data-scope': node.attrs.scope })]
  },
  addNodeView() {
    return VueNodeViewRenderer(AgendaBlockView, { stopEvent: ({ event }) => !(event.target as HTMLElement | null)?.closest?.('[data-drag-handle]') })
  },
})

export const lootyBlocks = [BoardBlock, AgendaBlock]
