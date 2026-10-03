import { Extension } from '@tiptap/core'
import Suggestion from '@tiptap/suggestion'
import { PluginKey } from '@tiptap/pm/state'
import { allEmojis } from '~/utils/emojis'
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
        items: ({ query }) => {
          if (query.length < 2) return []
          const q = query.toLowerCase()
          return (allEmojis as { title: string; emoji: string }[])
            .filter((e) => e.title.includes(q))
            .sort((a, b) => Number(!a.title.startsWith(q)) - Number(!b.title.startsWith(q)))
            .slice(0, 8)
            .map((e) => ({ id: e.title, label: `:${e.title}:`, emoji: e.emoji }))
        },
        command: ({ editor, range, props }) => editor.chain().focus().insertContentAt(range, (props as any).emoji).run(),
        render: suggestionRenderer(),
      }),
    ]
  },
})
