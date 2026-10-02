import { Node, mergeAttributes } from '@tiptap/core'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import { nodePasteRule } from '@tiptap/core'
import MathBlock from './MathBlock.vue'

export const CustomMathExtension = Node.create({
  name: 'mathBlock',
  group: 'block',
  atom: true,

  addAttributes() {
    return {
      latex: {
        default: '',
        parseHTML: element => element.getAttribute('latex'),
      },
    }
  },

  parseHTML() {
    return [
      {
        tag: 'div[data-type="math-block"]',
      },
    ]
  },

  renderHTML({ HTMLAttributes }) {
    return ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'math-block' })]
  },

  
  addPasteRules() {
    return [
      nodePasteRule({
        find: /\$\$([^$]+)\$\$/g,
        type: this.type,
        getAttributes: match => ({ latex: match[1].trim() }),
      }),
    ]
  },

  addNodeView() {
    return VueNodeViewRenderer(MathBlock)
  },

  addCommands() {
    return {
      insertMathBlock: () => ({ commands }) => {
        return commands.insertContent({
          type: this.name,
          attrs: {
            latex: '\\frac{1}{2}'
          }
        })
      },
    }
  }
})
