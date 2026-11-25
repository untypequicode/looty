import { Extension, Mark, mergeAttributes } from '@tiptap/core'

/** Couleurs de texte proposées : des clés (et non des valeurs), le thème clair ou sombre choisit la teinte. */
export const TEXT_COLORS = ['gray', 'red', 'orange', 'yellow', 'green', 'blue', 'purple'] as const
export type TextColorKey = (typeof TEXT_COLORS)[number]
export const TEXT_ALIGNS = ['left', 'center', 'right', 'justify'] as const
export type TextAlignKey = (typeof TEXT_ALIGNS)[number]

export function cleanColor(v: unknown): TextColorKey | null {
  return typeof v === 'string' && (TEXT_COLORS as readonly string[]).includes(v) ? (v as TextColorKey) : null
}
export function cleanAlign(v: unknown): TextAlignKey | null {
  return typeof v === 'string' && (TEXT_ALIGNS as readonly string[]).includes(v) && v !== 'left' ? (v as TextAlignKey) : null
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    textStyle: {
      setTextColor: (color: string) => ReturnType
      unsetTextColor: () => ReturnType
      setTextAlign: (align: string) => ReturnType
    }
  }
}

/** Couleur du texte : marque `span[data-text-color]` portant une clé de la palette. */
export const TextColor = Mark.create({
  name: 'textColor',
  addAttributes() {
    return {
      color: {
        default: null,
        parseHTML: (el: HTMLElement) => cleanColor(el.getAttribute('data-text-color')),
        renderHTML: (attrs: Record<string, unknown>) => (cleanColor(attrs.color) ? { 'data-text-color': attrs.color as string } : {}),
      },
    }
  },
  parseHTML() {
    return [{ tag: 'span[data-text-color]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(HTMLAttributes), 0]
  },
  addCommands() {
    return {
      setTextColor: (color) => ({ commands }) => (cleanColor(color) ? commands.setMark('textColor', { color }) : false),
      unsetTextColor: () => ({ commands }) => commands.unsetMark('textColor'),
    }
  },
})

/** Alignement des paragraphes et des titres (attribut `text-align`, « gauche » = absence d'attribut). */
export const TextAlign = Extension.create({
  name: 'textAlign',
  addGlobalAttributes() {
    return [{
      types: ['paragraph', 'heading'],
      attributes: {
        textAlign: {
          default: null,
          parseHTML: (el: HTMLElement) => cleanAlign(el.style.textAlign),
          renderHTML: (attrs: Record<string, unknown>) => {
            const a = cleanAlign(attrs.textAlign)
            return a ? { style: `text-align: ${a}` } : {}
          },
        },
      },
    }]
  },
  addCommands() {
    return {
      setTextAlign: (align) => ({ commands }) => {
        const v = cleanAlign(align)
        return ['paragraph', 'heading'].map((t) => commands.updateAttributes(t, { textAlign: v })).some(Boolean)
      },
    }
  },
})
