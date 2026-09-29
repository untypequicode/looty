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
    { title: 'thinking', emoji: '🤔' },
    { title: 'shrug', emoji: '🤷' },
    { title: 'facepalm', emoji: '🤦' },
    { title: 'pray', emoji: '🙏' },
    { title: 'tada', emoji: '🎉' },
    { title: 'sparkles', emoji: '✨' },
    { title: 'clap', emoji: '👏' },
    { title: 'handshake', emoji: '🤝' },
    { title: 'ok', emoji: '👌' },
    { title: 'muscle', emoji: '💪' },
    { title: 'brain', emoji: '🧠' },
    { title: 'alien', emoji: '👽' },
    { title: 'ghost', emoji: '👻' },
    { title: 'skull', emoji: '💀' },
    { title: 'robot', emoji: '🤖' },
    { title: 'poop', emoji: '💩' },
    { title: 'money', emoji: '💰' },
    { title: 'chart', emoji: '📈' },
    { title: 'computer', emoji: '💻' },
    { title: 'phone', emoji: '📱' },
    { title: 'clock', emoji: '🕒' },
    { title: 'calendar', emoji: '📅' },
    { title: 'book', emoji: '📖' },
    { title: 'pencil', emoji: '✏️' },
    { title: 'mag', emoji: '🔍' },
    { title: 'lock', emoji: '🔒' },
    { title: 'key', emoji: '🔑' },
    { title: 'tools', emoji: '🛠️' },
    { title: 'gear', emoji: '⚙️' },
    { title: 'link', emoji: '🔗' },
    { title: 'bug', emoji: '🐛' },
    { title: 'wave', emoji: '👋' },
    { title: 'sad', emoji: '😔' },
    { title: 'angry', emoji: '😠' },
    { title: 'cool', emoji: '😎' },
    { title: 'zany', emoji: '🤪' },
    { title: 'exploding_head', emoji: '🤯' },
    { title: 'tired', emoji: '😫' },
    { title: 'sleeping', emoji: '😴' },
    { title: 'mask', emoji: '😷' }
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
