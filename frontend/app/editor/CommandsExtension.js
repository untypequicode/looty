import { Extension } from '@tiptap/core'
import Suggestion from '@tiptap/suggestion'
import { VueRenderer } from '@tiptap/vue-3'
import tippy from 'tippy.js'
import CommandList from './CommandList.vue'

export const CommandsExtension = Extension.create({
  name: 'slashCommands',

  addOptions() {
    return {
      suggestion: {
        char: '/',
        command: ({ editor, range, props }) => {
          props.command({ editor, range })
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

export const getSuggestionItems = ({ query }) => {
  return [
    {
      title: 'H1 En-tête',
      shortcut: '⇧ ⌘ 1',
      icon: 'H1',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).setNode('heading', { level: 1 }).run()
      },
    },
    {
      title: 'Titre moyen',
      shortcut: '⇧ ⌘ 2',
      icon: 'H2',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).setNode('heading', { level: 2 }).run()
      },
    },
    {
      title: 'Petit titre',
      shortcut: '⇧ ⌘ 3',
      icon: 'H3',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).setNode('heading', { level: 3 }).run()
      },
    },
    {
      title: 'En-tête extrêmement petit',
      shortcut: '⇧ ⌘ 4',
      icon: 'H4',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).setNode('heading', { level: 4 }).run()
      },
    },
    {
      title: 'Liste des tâches',
      shortcut: '⇧ ⌘ 7',
      icon: '✓',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).toggleTaskList().run()
      },
    },
    {
      title: 'Liste à puces',
      shortcut: '⇧ ⌘ 8',
      icon: '•',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).toggleBulletList().run()
      },
    },
    {
      title: 'Liste ordonnée',
      shortcut: '⇧ ⌘ 9',
      icon: '1.',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).toggleOrderedList().run()
      },
    },
    {
      title: 'Image',
      shortcut: '',
      icon: '🖼',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).run()
        const input = document.createElement('input')
        input.type = 'file'
        input.accept = 'image/*'
        input.onchange = async (e) => {
          if (e.target.files && e.target.files[0]) {
            const file = e.target.files[0]
            const formData = new FormData()
            formData.append('file', file)
            try {
              const response = await fetch('/api/upload', {
                method: 'POST',
                body: formData
              })
              const data = await response.json()
              if (data.url) {
                editor.chain().focus().setImage({ src: data.url }).run()
              }
            } catch (err) {
              console.error('Upload failed', err)
            }
          }
        }
        input.click()
      },
    },
    {
      title: 'Tableau',
      shortcut: '',
      icon: '▦',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()
      },
    },
    {
      title: 'Citation',
      shortcut: 'Ctrl ]',
      icon: '""',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).toggleBlockquote().run()
      },
    },
    {
      title: 'Bloc de code',
      shortcut: '⇧ ⌘ C',
      icon: '<>',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).toggleCodeBlock().run()
      },
    },
    {
      title: 'Bloc mathématique (LaTeX)',
      shortcut: '',
      icon: 'fx',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertContent({ type: 'mathInline', attrs: { latex: '' } }).run()
      },
    },
    {
      title: 'Séparateur',
      shortcut: 'Ctrl _',
      icon: '—',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).setHorizontalRule().run()
      },
    },
  ].filter(item => item.title.toLowerCase().startsWith(query.toLowerCase())).slice(0, 10)
}

export const renderItems = () => {
  let component
  let popup

  return {
    onStart: props => {
      component = new VueRenderer(CommandList, {
        props,
        editor: props.editor,
      })

      if (!props.clientRect) {
        return
      }

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

      if (!props.clientRect) {
        return
      }

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
      if (popup) {
        popup[0].destroy()
      }
      if (component) {
        component.destroy()
      }
    },
  }
}
