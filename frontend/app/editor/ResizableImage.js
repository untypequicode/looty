import Image from '@tiptap/extension-image'
import { VueNodeViewRenderer } from '@tiptap/vue-3'
import ImageNodeView from './ImageNodeView.vue'

export const ResizableImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      width: {
        default: '100%',
        parseHTML: element => element.getAttribute('width'),
        renderHTML: attributes => {
          return {
            width: attributes.width,
          }
        },
      },
    }
  },
  addNodeView() {
    return VueNodeViewRenderer(ImageNodeView)
  },
})
