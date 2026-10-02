import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { Node, mergeAttributes } from '@tiptap/core';

const CustomMathInlineExtension = Node.create({
  name: 'mathInline',
  group: 'inline',
  inline: true,
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
        tag: 'span[data-type="math-inline"]',
      },
    ]
  },

  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(HTMLAttributes, { 'data-type': 'math-inline' })]
  }
});

const editor = new Editor({
  extensions: [StarterKit, CustomMathInlineExtension],
  content: '<p><span data-type="math-inline" latex="x_i"></span> ; quantité d&#39;argent (<span data-type="math-inline" latex=") investi dans l\'actif"></span>i\in[[1,4]]$</p>'
});

console.log(editor.getHTML());
