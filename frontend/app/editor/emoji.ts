import { Extension } from '@tiptap/core'
import Suggestion from '@tiptap/suggestion'
import { PluginKey } from '@tiptap/pm/state'
import { loadEmojis, searchEmojis, pushRecentEmoji } from '~/utils/emojiData'
import { currentLocale } from '~/composables/useI18n'
import { suggestionRenderer } from './suggestion'

/** Émojis « :smile » (au moins 2 caractères pour ne pas gêner la saisie de « : »). */
export const EmojiSuggestion = Extension.create({
  name: 'emojiSuggestion',
  addProseMirrorPlugins() {
    return [
      Suggestion({
        editor: this.editor,
        char: ':',
        pluginKey: new PluginKey('emojiSuggestion'),
        allowSpaces: false,
        allow: ({ state, range }) => state.doc.resolve(range.from).parent.type.name !== 'codeBlock',
        items: async ({ query }) => {
          if (query.length < 2) return []
          const fr = currentLocale() === 'fr'
          return searchEmojis(await loadEmojis(), query.replace(/_/g, ' '), 8).map((e) => ({ id: e.emoji, label: fr ? e.fr : e.en, emoji: e.emoji }))
        },
        command: ({ editor, range, props }) => {
          pushRecentEmoji((props as any).emoji)
          editor.chain().focus().insertContentAt(range, (props as any).emoji).run()
        },
        render: suggestionRenderer(),
      }),
    ]
  },
})
