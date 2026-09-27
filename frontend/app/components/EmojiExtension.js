import { Extension } from '@tiptap/core'
import Suggestion from '@tiptap/suggestion'
import { PluginKey } from '@tiptap/pm/state'
import tippy from 'tippy.js'
import { VueRenderer } from '@tiptap/vue-3'
import EmojiList from './EmojiList.vue'

export const EmojiExtension = Extension.create({
  name: 'emojiExtension',

  addOptions() {
    return {
      suggestion: {
        char: ':',
        pluginKey: new PluginKey('emojiExtension'),
        command: ({ editor, range, props }) => {
          editor
            .chain()
            .focus()
            .deleteRange(range)
            .insertContent(props.emoji)
            .run()
        },
      },
    }
  },

  addProseMirrorPlugins() {
    return [
      Suggestion({
        editor: this.editor,
        ...this.options.suggestion,
      }),
    ]
  },
})

export const getEmojiItems = ({ query }) => {
  const emojis = [
    { title: 'smile', emoji: '😄' },
    { title: 'joy', emoji: '😂' },
    { title: 'heart', emoji: '❤️' },
    { title: 'thumbsup', emoji: '👍' },
    { title: 'fire', emoji: '🔥' },
    { title: 'star', emoji: '⭐' },
    { title: 'rocket', emoji: '🚀' },
    { title: 'party', emoji: '🥳' },
    { title: 'check', emoji: '✅' },
    { title: 'cross', emoji: '❌' },
    { title: 'warning', emoji: '⚠️' },
    { title: 'info', emoji: 'ℹ️' },
    { title: 'idea', emoji: '💡' },
    { title: 'pin', emoji: '📌' },
    { title: 'eyes', emoji: '👀' },
    { title: 'sob', emoji: '😭' },
    { title: 'cry', emoji: '😢' },
    { title: 'sweat_smile', emoji: '😅' },
    { title: 'wink', emoji: '😉' },
    { title: 'sunglasses', emoji: '😎' },
  ]
  return emojis.filter(item => item.title.toLowerCase().startsWith(query.toLowerCase())).slice(0, 10)
}

export const renderEmojiItems = () => {
  let component
  let popup

  return {
    onStart: props => {
      component = new VueRenderer(EmojiList, {
        props,
        editor: props.editor,
      })

      if (!props.clientRect) return

      popup = tippy('body', {
        getReferenceClientRect: props.clientRect,
        appendTo: () => document.body,
        content: component.element,
        showOnCreate: true,
        interactive: true,
        trigger: 'manual',
        placement: 'bottom-start',
      })
    },

    onUpdate(props) {
      component.updateProps(props)

      if (!props.clientRect) return

      popup[0].setProps({
        getReferenceClientRect: props.clientRect,
      })
    },

    onKeyDown(props) {
      if (props.event.key === 'Escape') {
        popup[0].hide()
        return true
      }
      return component.ref?.onKeyDown(props)
    },

    onExit() {
      if (popup && popup[0]) {
        popup[0].destroy()
      }
      if (component) {
        component.destroy()
      }
    },
  }
}
