import { Extension } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { yUndoPluginKey } from '@tiptap/y-tiptap'
import type { UndoManager } from 'yjs'

/**
 * Annuler / rétablir en collaboration : un seul gestionnaire Yjs pour toute la vie de l'éditeur.
 *
 * y-tiptap détruit le gestionnaire chaque fois que les vues de plugins sont recréées (menus
 * enregistrés après le montage, vue créée deux fois…). Un gestionnaire détruit ne se suit plus
 * lui-même : Ctrl+Z marche encore mais l'étape annulée n'est jamais empilée, donc Ctrl+Y ne
 * rétablit rien. On neutralise ces destructions et on ne le détruit qu'avec l'éditeur.
 */
export const CollaborativeUndo = Extension.create({
  name: 'collaborativeUndo',

  addProseMirrorPlugins() {
    const editor = this.editor
    return [
      new Plugin({
        key: new PluginKey('collaborativeUndo'),
        view: (view) => {
          const um = yUndoPluginKey.getState(view.state)?.undoManager as (UndoManager & { lootyPinned?: boolean }) | undefined
          if (um && !um.lootyPinned) {
            um.lootyPinned = true
            const destroy = um.destroy
            um.destroy = () => {}
            editor.on('destroy', () => destroy())
          }
          // Remis en place même si une vue précédente l'avait déjà détruit.
          if (um) {
            um.trackedOrigins.add(um)
            um.doc.on('afterTransaction', um.afterTransactionHandler)
          }
          return {}
        },
      }),
    ]
  },
})
