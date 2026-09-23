import { Extension } from '@tiptap/core'
import Suggestion from '@tiptap/suggestion'
import { VueRenderer } from '@tiptap/vue-3'
import tippy from 'tippy.js'
import CommandList from './CommandList.vue'

export const CommandsExtension = Extension.create({
  name: 'commands',

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
      title: 'Titre 1',
      subtitle: 'Grand titre de section',
      icon: 'H1',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).setNode('heading', { level: 1 }).run()
      },
    },
    {
      title: 'Titre 2',
      subtitle: 'Moyen titre',
      icon: 'H2',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).setNode('heading', { level: 2 }).run()
      },
    },
    {
      title: 'Liste à puces',
      subtitle: 'Créer une liste',
      icon: '•',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).toggleBulletList().run()
      },
    },
    {
      title: 'Math en ligne',
      subtitle: 'Formule LaTeX dans le texte',
      icon: '$',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertContent({ type: 'mathInline', attrs: { latex: '' } }).run()
      },
    },
    {
      title: 'Liste de tâches',
      subtitle: 'Créer une checklist',
      icon: '☐',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).toggleTaskList().run()
      },
    },
    {
      title: 'Code source',
      subtitle: 'Bloc de code',
      icon: '<>',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).toggleCodeBlock().run()
      },
    },
    {
      title: 'Citation',
      subtitle: 'Mettre en exergue',
      icon: '""',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).toggleBlockquote().run()
      },
    },
    {
      title: 'Séparateur',
      subtitle: 'Ligne horizontale',
      icon: '—',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).setHorizontalRule().run()
      },
    },
    {
      title: 'Math en bloc',
      subtitle: 'Formule LaTeX centrée',
      icon: '∑',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertContent({ type: 'mathBlock', attrs: { latex: '' } }).run()
      },
    },
  ].filter(item => item.title.toLowerCase().startsWith(query.toLowerCase()) || item.subtitle.toLowerCase().includes(query.toLowerCase()))
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
