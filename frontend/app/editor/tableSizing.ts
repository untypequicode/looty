import { TableRow, TableCell, TableHeader } from '@tiptap/extension-table'
import { Plugin, PluginKey } from '@tiptap/pm/state'

/**
 * Tableaux : hauteur de ligne ajustable. La largeur des colonnes est déjà portée par `colwidth`
 * (extension Table, `resizable`) ; la hauteur l'est ici par l'attribut `rowheight` de la ligne, donc
 * enregistré dans le document (synchronisé par Yjs) et rendu à l'identique en lecture seule, à l'export et à l'impression.
 */
export const MIN_ROW_HEIGHT = 24
export const MAX_ROW_HEIGHT = 1000
/** Zone sensible, en pixels, au bord bas d'une cellule. */
const GRAB = 4

/** Hauteur valide (entier de pixels) ou `null` si absente ou aberrante (ancien contenu, HTML importé). */
export function clampRowHeight(v: unknown): number | null {
  const n = typeof v === 'number' ? v : parseFloat(String(v ?? ''))
  if (!Number.isFinite(n) || n <= 0) return null
  return Math.round(Math.min(MAX_ROW_HEIGHT, Math.max(MIN_ROW_HEIGHT, n)))
}

const key = new PluginKey('tableRowResize')

export const ResizableTableRow = TableRow.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      rowheight: {
        default: null,
        parseHTML: (el: HTMLElement) => clampRowHeight(el.getAttribute('data-rowheight') ?? el.style.height.replace('px', '')),
        renderHTML: (attrs: Record<string, unknown>) => {
          const h = clampRowHeight(attrs.rowheight)
          return h ? { 'data-rowheight': String(h), style: `height: ${h}px` } : {}
        },
      },
    }
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key,
        props: {
          handleDOMEvents: {
            mousemove(view, e) {
              if (!view.editable) return false
              const cell = (e.target as HTMLElement | null)?.closest?.('td,th') as HTMLElement | null
              const r = cell?.getBoundingClientRect()
              // Le bord droit appartient à la poignée de colonne.
              const on = !!r && r.bottom - e.clientY <= GRAB && r.bottom - e.clientY >= -GRAB && r.right - e.clientX > GRAB + 2
              view.dom.classList.toggle('row-resize-ready', on)
              return false
            },
            mouseleave(view) {
              view.dom.classList.remove('row-resize-ready')
              return false
            },
            dblclick(view, e) {
              if (!view.dom.classList.contains('row-resize-ready')) return false
              const found = rowAt(view, e.target)
              if (!found) return false
              view.dispatch(view.state.tr.setNodeMarkup(found.pos, undefined, { ...found.node.attrs, rowheight: null }))
              e.preventDefault()
              return true
            },
            mousedown(view, e) {
              if (!view.editable || e.button !== 0 || !view.dom.classList.contains('row-resize-ready')) return false
              const tr = (e.target as HTMLElement).closest('tr') as HTMLElement | null
              const found = rowAt(view, e.target)
              if (!tr || !found) return false
              e.preventDefault()
              const startY = e.clientY
              const startH = tr.getBoundingClientRect().height
              let h = startH
              document.body.style.cursor = 'row-resize'
              const move = (ev: MouseEvent) => {
                h = clampRowHeight(startH + ev.clientY - startY) ?? startH
                tr.style.height = `${h}px`
              }
              const up = () => {
                window.removeEventListener('mousemove', move)
                window.removeEventListener('mouseup', up)
                document.body.style.cursor = ''
                const cur = rowAt(view, tr.firstElementChild)
                if (cur && h !== Math.round(startH)) view.dispatch(view.state.tr.setNodeMarkup(cur.pos, undefined, { ...cur.node.attrs, rowheight: h }))
                else tr.style.height = ''
              }
              window.addEventListener('mousemove', move)
              window.addEventListener('mouseup', up)
              return true
            },
          },
        },
      }),
    ]
  },
})

/** Ligne de tableau (nœud et position) contenant l'élément DOM. */
function rowAt(view: import('@tiptap/pm/view').EditorView, target: EventTarget | null) {
  const el = (target as HTMLElement | null)?.closest?.('tr')
  if (!el) return null
  const inner = view.posAtDOM(el, 0)
  const node = view.state.doc.nodeAt(inner - 1)
  return node?.type.name === 'tableRow' ? { pos: inner - 1, node } : null
}

/** Sens du texte d'une cellule : horizontal (absent), tourné de 90° vers le haut (« up ») ou vers le bas (« down »). */
export type CellDirection = 'up' | 'down'
export const cellDirection = (v: unknown): CellDirection | null => (v === 'up' || v === 'down' ? v : null)

const directionAttr = {
  default: null,
  parseHTML: (el: HTMLElement) => cellDirection(el.getAttribute('data-text-dir')),
  renderHTML: (attrs: Record<string, unknown>) => {
    const d = cellDirection(attrs.textDirection)
    // Style en ligne : le rendu reste correct dans les exports et l'impression. Un navigateur sans
    // `sideways-lr` ignore la déclaration et affiche le texte à l'horizontale.
    return d ? { 'data-text-dir': d, style: `writing-mode: ${d === 'up' ? 'sideways-lr' : 'vertical-rl'}; text-align: center; vertical-align: middle` } : {}
  },
}

export const DirectionalCell = TableCell.extend({ addAttributes() { return { ...this.parent?.(), textDirection: directionAttr } } })
export const DirectionalHeader = TableHeader.extend({ addAttributes() { return { ...this.parent?.(), textDirection: directionAttr } } })
