import { Node, mergeAttributes } from '@tiptap/core'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import MathInline from './MathInline.vue'
import { InputRule } from '@tiptap/core'

export const CustomMathInlineExtension = Node.create({
  name: 'mathInline',
  group: 'inline',
  inline: true,
  atom: true,

  addAttributes() {
    return {
      latex: {
        default: '',
      },
    }
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-type="math-inline"]',
      },
    ]
  },

  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(HTMLAttributes, { 'data-type': 'math-inline' })]
  },

  addNodeView() {
    return VueNodeViewRenderer(MathInline)
  },

  addInputRules() {
    return [
      new InputRule({
        find: /\$([^$]+)\$$/,
        handler: ({ state, range, match }) => {
          const { tr } = state
          const start = range.from
          const end = range.to
          const latex = match[1]

          tr.replaceWith(start, end, this.type.create({ latex }))
        },
      }),
    ]
  },

  addCommands() {
    return {
      insertMathInline: () => ({ commands }) => {
        return commands.insertContent({
          type: this.name,
          attrs: {
            latex: 'i \\in N'
          }
        })
      },
    }
  }
})
