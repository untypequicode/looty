import { Node, mergeAttributes } from '@tiptap/core'

export interface NoticeOptions {
  HTMLAttributes: Record<string, any>
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    notice: {
      setNotice: (attributes?: { type: string }) => ReturnType
      toggleNotice: (attributes?: { type: string }) => ReturnType
    }
  }
}

export const NoticeExtension = Node.create<NoticeOptions>({
  name: 'notice',

  addOptions() {
    return {
      HTMLAttributes: {},
    }
  },

  content: 'block+',
  group: 'block',
  defining: true,

  addAttributes() {
    return {
      type: {
        default: 'info',
        parseHTML: element => element.getAttribute('data-notice-type') || 'info',
        renderHTML: attributes => {
          return {
            'data-notice-type': attributes.type,
          }
        },
      },
    }
  },

  parseHTML() {
    return [
      {
        tag: 'blockquote[data-type="notice"]',
      },
      {
        tag: 'div[data-type="notice"]',
      },
    ]
  },

  renderHTML({ HTMLAttributes }) {
    return ['blockquote', mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, { 'data-type': 'notice', 'class': `notice-block notice-${HTMLAttributes['data-notice-type'] || 'info'}` }), 0]
  },

  addCommands() {
    return {
      setNotice: attributes => ({ commands }) => {
        return commands.setNode(this.name, attributes)
      },
      toggleNotice: attributes => ({ commands }) => {
        return commands.toggleNode(this.name, 'paragraph', attributes)
      },
    }
  },
})
