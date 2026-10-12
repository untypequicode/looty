import type { Editor } from '@tiptap/core'
import { absolutePositionToRelativePosition, relativePositionToAbsolutePosition, ySyncPluginKey } from '@tiptap/y-tiptap'
import * as Y from 'yjs'
import { nextTick, onBeforeUnmount, ref, shallowRef, watch, type Ref, type ShallowRef } from 'vue'

/**
 * Curseurs des autres personnes dans les champs de formule et de diagramme.
 *
 * Ces champs sont de vrais champs de saisie (hors de la sélection ProseMirror) : le curseur
 * de y-tiptap ne les voit pas. Chacun publie donc sa position dans le champ (position relative
 * Yjs, qui reste juste pendant que le texte change) dans la présence de la collaboration ;
 * les autres la résolvent et dessinent un curseur nommé (et la sélection) par-dessus.
 */
export interface SourceCaret {
  key: string
  name: string
  color: string
  left: number
  top: number
  height: number
  rects: { left: number; top: number; width: number; height: number }[]
}

interface Remote { id: number; name: string; color: string; anchor: number; head: number }
type Field = HTMLInputElement | HTMLTextAreaElement
type Provider = { awareness: any }

const provider = (editor: Editor): Provider | undefined =>
  (editor.extensionManager.extensions.find((e) => e.name === 'collaborationCaret')?.options as any)?.provider

const binding = (editor: Editor) => (ySyncPluginKey.getState(editor.state) as any)?.binding

// Positions distantes, résolues une fois par éditeur (partagées par tous les champs).
const stores = new WeakMap<Editor, { remotes: ShallowRef<Remote[]>; users: number; stop: () => void }>()

function acquire(editor: Editor): ShallowRef<Remote[]> | null {
  const prov = provider(editor)
  if (!prov) return null
  let s = stores.get(editor)
  if (!s) {
    const remotes = shallowRef<Remote[]>([])
    let queued = false
    const resolve = () => {
      queued = false
      const b = binding(editor)
      if (!b) { remotes.value = []; return }
      const out: Remote[] = []
      const self = prov.awareness.clientID
      prov.awareness.getStates().forEach((st: any, id: number) => {
        if (id === self || !st?.sourceCursor) return
        try {
          const a = relativePositionToAbsolutePosition(b.type.doc, b.type, Y.createRelativePositionFromJSON(st.sourceCursor.anchor), b.mapping)
          const h = relativePositionToAbsolutePosition(b.type.doc, b.type, Y.createRelativePositionFromJSON(st.sourceCursor.head), b.mapping)
          if (a != null && h != null) out.push({ id, name: st.user?.name ?? '?', color: st.user?.color ?? '#7170ff', anchor: a, head: h })
        } catch { /* position pas encore connue */ }
      })
      remotes.value = out
    }
    const schedule = () => { if (!queued) { queued = true; requestAnimationFrame(resolve) } }
    prov.awareness.on('update', schedule)
    editor.on('transaction', schedule)
    s = { remotes, users: 0, stop: () => { prov.awareness.off('update', schedule); editor.off('transaction', schedule) } }
    stores.set(editor, s)
    schedule()
  }
  s.users++
  return s.remotes
}
function release(editor: Editor) {
  const s = stores.get(editor)
  if (!s || --s.users > 0) return
  s.stop()
  stores.delete(editor)
}

const COPIED = [
  'boxSizing', 'borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth', 'borderStyle',
  'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft', 'fontStyle', 'fontVariant', 'fontWeight', 'fontStretch',
  'fontSize', 'fontFamily', 'lineHeight', 'textAlign', 'textTransform', 'textIndent', 'letterSpacing', 'wordSpacing', 'tabSize',
] as const

/** Position (dans le repère du parent positionné du champ) du curseur et de la sélection. */
function measure(el: Field, from: number, to: number) {
  const cs = getComputedStyle(el)
  const area = el instanceof HTMLTextAreaElement
  const mirror = document.createElement('div')
  const st = mirror.style
  for (const p of COPIED) (st as any)[p] = (cs as any)[p]
  st.position = 'absolute'
  st.visibility = 'hidden'
  st.top = '0'
  st.left = '-9999px'
  st.width = `${el.offsetWidth}px`
  st.overflow = 'hidden'
  st.whiteSpace = area ? 'pre-wrap' : 'pre'
  st.overflowWrap = area ? 'break-word' : 'normal'
  document.body.appendChild(mirror)
  const value = el.value
  const a = Math.min(from, to), b = Math.max(from, to)
  mirror.appendChild(document.createTextNode(value.slice(0, a)))
  const sel = document.createElement('span')
  sel.textContent = value.slice(a, b)
  mirror.appendChild(sel)
  const caret = document.createElement('span')
  caret.textContent = value.slice(b) || '.'
  mirror.appendChild(caret)
  // Le curseur est au début de « caret » ; sa hauteur : celle d'une ligne.
  const fontSize = parseFloat(cs.fontSize) || 14
  const height = parseFloat(cs.lineHeight) || fontSize * 1.25
  const origin = { x: el.offsetLeft + el.clientLeft - el.scrollLeft, y: el.offsetTop + el.clientTop - el.scrollTop }
  const base = mirror.getBoundingClientRect()
  const inner = { x: mirror.clientLeft, y: mirror.clientTop }
  const rects = a === b ? [] : [...sel.getClientRects()].map((r) => ({ left: origin.x + r.left - base.left - inner.x, top: origin.y + r.top - base.top - inner.y, width: r.width, height: r.height }))
  const out = { left: origin.x + caret.offsetLeft, top: origin.y + caret.offsetTop, height, rects }
  mirror.remove()
  return out
}

/** Curseurs distants dans ce champ + publication du curseur local. */
export function useSourceCursors(
  field: Ref<Field | null>,
  props: { editor: Editor; getPos: () => number | undefined },
  source: Ref<string>,
) {
  const carets = ref<SourceCaret[]>([])
  /** Personnes qui sont dans ce bloc (même si je ne le modifie pas moi-même). */
  const present = ref<{ key: string; name: string; color: string }[]>([])
  const remotes = acquire(props.editor)
  const prov = provider(props.editor)

  /** Mon curseur dans le champ → présence. */
  function send() {
    const el = field.value
    const b = binding(props.editor)
    const pos = props.getPos()
    if (!prov || !b || pos == null) return
    if (!el || document.activeElement !== el) { prov.awareness.setLocalStateField('sourceCursor', null); return }
    const at = (i: number) => Y.relativePositionToJSON(absolutePositionToRelativePosition(pos + 1 + i, b.type, b.mapping))
    prov.awareness.setLocalStateField('sourceCursor', { anchor: at(el.selectionStart ?? 0), head: at(el.selectionEnd ?? 0) })
  }
  const soon = () => nextTick(send)

  /** Curseurs des autres : ceux dont la position tombe dans ce nœud. */
  function refresh() {
    const el = field.value
    const pos = props.getPos()
    const node = pos == null ? null : props.editor.state.doc.nodeAt(pos)
    if (!remotes || pos == null || !node) { carets.value = []; present.value = []; return }
    const start = pos + 1, end = start + node.content.size
    const inside = remotes.value.filter((r) => r.head >= start && r.head <= end)
    present.value = inside.map((r) => ({ key: String(r.id), name: r.name, color: r.color }))
    if (!el) { carets.value = []; return }
    const len = el.value.length
    carets.value = inside
      .map((r) => {
        const head = Math.min(len, r.head - start)
        const anchor = Math.min(len, Math.max(0, Math.min(r.anchor, end) - start))
        const m = measure(el, anchor, head)
        return { key: String(r.id), name: r.name, color: r.color, ...m }
      })
  }

  const events = ['select', 'keyup', 'mouseup', 'input', 'focus', 'blur'] as const
  const onBlur = () => prov?.awareness.setLocalStateField('sourceCursor', null)
  watch(field, (el, old) => {
    for (const e of events) { old?.removeEventListener(e, soon); el?.addEventListener(e, soon) }
    old?.removeEventListener('blur', onBlur)
    el?.addEventListener('blur', onBlur)
    if (!el) onBlur()
    nextTick(refresh)
  }, { immediate: true, flush: 'post' })
  if (remotes) watch([remotes, source], () => nextTick(refresh))
  const resize = () => refresh()
  window.addEventListener('resize', resize)
  onBeforeUnmount(() => {
    window.removeEventListener('resize', resize)
    onBlur()
    if (remotes) release(props.editor)
  })
  return { carets, present, refreshCarets: refresh }
}
