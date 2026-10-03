import { VueRenderer } from '@tiptap/vue-3'
import tippy, { type Instance } from 'tippy.js'
import type { SuggestionOptions } from '@tiptap/suggestion'
import SuggestionList from '~/components/editor/SuggestionList.vue'

/**
 * Rendu standard des menus de suggestion TipTap (commande /, mention @, émoji :)
 * dans une popup tippy contenant `SuggestionList`.
 */
export function suggestionRenderer(extraProps: Record<string, any> = {}): SuggestionOptions['render'] {
  return () => {
    let component: VueRenderer | null = null
    let popup: Instance | null = null

    return {
      onStart(props) {
        component = new VueRenderer(SuggestionList, { props: { ...props, ...extraProps }, editor: props.editor })
        if (!props.clientRect) return
        popup = tippy(document.body, {
          getReferenceClientRect: props.clientRect as () => DOMRect,
          appendTo: () => document.body,
          content: component.element as Element,
          showOnCreate: true,
          interactive: true,
          trigger: 'manual',
          placement: 'bottom-start',
          offset: [0, 6],
          zIndex: 1500,
          popperOptions: { modifiers: [{ name: 'flip', options: { fallbackPlacements: ['top-start'] } }] },
        })
      },
      onUpdate(props) {
        component?.updateProps({ ...props, ...extraProps })
        if (props.clientRect && popup) popup.setProps({ getReferenceClientRect: props.clientRect as () => DOMRect })
      },
      onKeyDown(props) {
        if (props.event.key === 'Escape') {
          popup?.hide()
          return true
        }
        return (component?.ref as any)?.onKeyDown(props) ?? false
      },
      onExit() {
        popup?.destroy()
        component?.destroy()
        popup = null
        component = null
      },
    }
  }
}
