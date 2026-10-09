import { Extension } from '@tiptap/core'
import type { EditorState, Plugin, PluginKey } from '@tiptap/pm/state'

/**
 * Ajout / retrait de plugins (menus flottants montés après l'éditeur) sans désynchroniser
 * l'état de Tiptap et celui de la vue.
 *
 * Pendant l'ajout, ProseMirror recrée les vues des plugins et celle de y-tiptap redessine le
 * document depuis Yjs en envoyant une transaction. L'éditeur Vue l'applique à son état mis en
 * cache (encore sans le nouveau plugin) : la vue perd le plugin, l'éditeur le garde. À la
 * modification suivante, les listes diffèrent, les vues sont recréées une fois de plus et
 * y-tiptap remplace le document par la version Yjs : la modification est perdue (restauration
 * d'une version, premier collage…). Ici, l'état de référence est toujours celui de la vue.
 */
export const StablePluginState = Extension.create({
  name: 'stablePluginState',

  onCreate() {
    const editor = this.editor as any
    const sync = (state: EditorState) => {
      if (editor.reactiveState) editor.reactiveState.value = state
    }
    const apply = (makePlugins: (current: readonly Plugin[]) => Plugin[] | null, fallback: () => EditorState | undefined) => {
      const view = editor.editorView
      if (!view || editor.isDestroyed) return fallback()
      const plugins = makePlugins(view.state.plugins)
      if (!plugins) return undefined
      const next = view.state.reconfigure({ plugins })
      // Les transactions envoyées pendant la reconfiguration partent du nouvel état.
      sync(next)
      view.updateState(next)
      sync(view.state)
      return view.state
    }
    const register = editor.registerPlugin.bind(editor)
    const unregister = editor.unregisterPlugin.bind(editor)
    editor.registerPlugin = (plugin: Plugin, handlePlugins?: (p: Plugin, list: Plugin[]) => Plugin[]) =>
      apply((cur) => (typeof handlePlugins === 'function' ? handlePlugins(plugin, [...cur]) : [...cur, plugin]), () => register(plugin, handlePlugins))
    editor.unregisterPlugin = (nameOrKey: string | PluginKey | (string | PluginKey)[]) =>
      apply((cur) => {
        let plugins = [...cur]
        for (const k of ([] as (string | PluginKey)[]).concat(nameOrKey)) {
          const name = typeof k === 'string' ? `${k}$` : (k as any).key
          plugins = plugins.filter((p) => !(p as any).key.startsWith(name))
        }
        return plugins.length === cur.length ? null : plugins
      }, () => unregister(nameOrKey))
  },
})
