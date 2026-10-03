import { Node, mergeAttributes, InputRule, nodePasteRule } from '@tiptap/core'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import Image from '@tiptap/extension-image'
import CodeBlockLowlight from '@tiptap/extension-code-block-lowlight'
import { createLowlight, common } from 'lowlight'
import MathBlockView from '~/components/editor/MathBlockView.vue'
import MathInlineView from '~/components/editor/MathInlineView.vue'
import ImageView from '~/components/editor/ImageView.vue'
import CodeBlockView from '~/components/editor/CodeBlockView.vue'
import ToggleView from '~/components/editor/ToggleView.vue'

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    callout: { setCallout: (attrs?: { type: string }) => ReturnType; toggleCallout: (attrs?: { type: string }) => ReturnType }
    toggleBlock: { setToggle: () => ReturnType }
    math: { insertMathBlock: (latex?: string) => ReturnType; insertMathInline: (latex?: string) => ReturnType }
  }
}

export const lowlight = createLowlight(common)

/** Bloc de code avec coloration syntaxique et sélecteur de langage. */
export const CodeBlock = CodeBlockLowlight.extend({
  addNodeView() {
    return VueNodeViewRenderer(CodeBlockView)
  },
}).configure({ lowlight, defaultLanguage: null })

/** Formule mathématique en bloc ($$...$$). Compatible avec le contenu existant. */
export const MathBlock = Node.create({
  name: 'mathBlock',
  group: 'block',
  atom: true,
  draggable: true,
  addAttributes() {
    return { latex: { default: '', parseHTML: (el) => el.getAttribute('latex') ?? '' } }
  },
  parseHTML() {
    return [{ tag: 'div[data-type="math-block"]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'math-block' })]
  },
  addNodeView() {
    return VueNodeViewRenderer(MathBlockView)
  },
  addCommands() {
    return {
      insertMathBlock: (latex = '') => ({ commands }) => commands.insertContent({ type: this.name, attrs: { latex } }),
    }
  },
  addInputRules() {
    return [
      new InputRule({
        find: /^\$\$\s$/,
        handler: ({ state, range }) => {
          state.tr.replaceRangeWith(range.from - 1, range.to, this.type.create({ latex: '' }))
        },
      }),
    ]
  },
  addPasteRules() {
    return [nodePasteRule({ find: /\$\$([^$]+)\$\$/g, type: this.type, getAttributes: (m) => ({ latex: m[1].trim() }) })]
  },
})

/** Formule en ligne ($...$). */
export const MathInline = Node.create({
  name: 'mathInline',
  group: 'inline',
  inline: true,
  atom: true,
  addAttributes() {
    return { latex: { default: '', parseHTML: (el) => el.getAttribute('latex') ?? '' } }
  },
  parseHTML() {
    return [{ tag: 'span[data-type="math-inline"]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(HTMLAttributes, { 'data-type': 'math-inline' })]
  },
  addNodeView() {
    return VueNodeViewRenderer(MathInlineView)
  },
  addCommands() {
    return {
      insertMathInline: (latex = '') => ({ commands }) => commands.insertContent({ type: this.name, attrs: { latex } }),
    }
  },
  addInputRules() {
    return [
      new InputRule({
        find: /(?:^|[^$])\$([^$\s][^$]*?)\$$/,
        handler: ({ state, range, match }) => {
          const start = range.to - match[1].length - 2
          state.tr.replaceWith(start, range.to, this.type.create({ latex: match[1] }))
        },
      }),
    ]
  },
})

/** Image redimensionnable (largeur en %) avec légende alternative. */
export const ResizableImage = Image.extend({
  draggable: true,
  addAttributes() {
    return {
      ...this.parent?.(),
      width: { default: null, parseHTML: (el) => el.getAttribute('width'), renderHTML: (a) => (a.width ? { width: a.width } : {}) },
      align: { default: 'center', parseHTML: (el) => el.getAttribute('data-align') ?? 'center', renderHTML: (a) => ({ 'data-align': a.align }) },
    }
  },
  addNodeView() {
    return VueNodeViewRenderer(ImageView)
  },
}).configure({ inline: false, allowBase64: false })

/**
 * Encadré (info / succès / attention / astuce). Le nom de nœud `notice` et le
 * balisage `blockquote[data-type=notice]` sont conservés pour le contenu existant.
 */
export const Callout = Node.create({
  name: 'notice',
  // Avant le blockquote standard, sinon le HTML est analysé comme une citation.
  priority: 101,
  group: 'block',
  content: 'block+',
  defining: true,
  addAttributes() {
    return {
      type: {
        default: 'info',
        parseHTML: (el) => el.getAttribute('data-notice-type') ?? 'info',
        renderHTML: (a) => ({ 'data-notice-type': a.type }),
      },
    }
  },
  parseHTML() {
    return [{ tag: 'blockquote[data-type="notice"]' }, { tag: 'div[data-type="notice"]' }]
  },
  renderHTML({ HTMLAttributes, node }) {
    return ['blockquote', mergeAttributes(HTMLAttributes, { 'data-type': 'notice', class: `callout callout-${node.attrs.type}` }), 0]
  },
  addCommands() {
    return {
      setCallout: (attrs) => ({ commands }) => commands.wrapIn(this.name, attrs),
      toggleCallout: (attrs) => ({ commands }) => commands.toggleWrap(this.name, attrs),
    }
  },
  addInputRules() {
    // ":::info " / ":::warning " ... en début de ligne
    return [
      new InputRule({
        find: /^:::(info|success|warning|tip)\s$/,
        handler: ({ state, range, match, chain }) => {
          state.tr.delete(range.from, range.to)
          chain().wrapIn(this.name, { type: match[1] }).run()
        },
      }),
    ]
  },
})

/** Bloc dépliable (toggle) : un résumé éditable + un contenu masquable. */
export const ToggleSummary = Node.create({
  name: 'toggleSummary',
  content: 'inline*',
  defining: true,
  selectable: false,
  parseHTML() {
    return [{ tag: 'summary' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['summary', HTMLAttributes, 0]
  },
})

export const ToggleContent = Node.create({
  name: 'toggleContent',
  content: 'block+',
  defining: true,
  selectable: false,
  parseHTML() {
    return [{ tag: 'div[data-type="toggle-content"]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['div', mergeAttributes(HTMLAttributes, { 'data-type': 'toggle-content' }), 0]
  },
})

export const Toggle = Node.create({
  name: 'toggle',
  group: 'block',
  content: 'toggleSummary toggleContent',
  defining: true,
  draggable: true,
  addAttributes() {
    return { open: { default: true, parseHTML: (el) => el.hasAttribute('open'), renderHTML: (a) => (a.open ? { open: '' } : {}) } }
  },
  parseHTML() {
    return [{ tag: 'details[data-type="toggle"]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['details', mergeAttributes(HTMLAttributes, { 'data-type': 'toggle' }), 0]
  },
  addNodeView() {
    return VueNodeViewRenderer(ToggleView)
  },
  addCommands() {
    return {
      setToggle: () => ({ commands }) =>
        commands.insertContent({
          type: this.name,
          attrs: { open: true },
          content: [{ type: 'toggleSummary' }, { type: 'toggleContent', content: [{ type: 'paragraph' }] }],
        }),
    }
  },
  addKeyboardShortcuts() {
    return {
      // Entrée dans le résumé : descend dans le contenu.
      Enter: ({ editor }) => {
        const { $from } = editor.state.selection
        if ($from.parent.type.name !== 'toggleSummary') return false
        const after = $from.after()
        return editor.chain().setTextSelection(after + 2).run()
      },
    }
  },
})
