<script setup lang="ts">
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import type { Editor as TEditor } from '@tiptap/vue-3'
import {
  Circle, ChevronDown, ChevronUp, Copy, Image as ImageIcon, Minus, Plus, Redo2, Square, Trash2, Type, Undo2,
} from 'lucide-vue-next'
import { socketBase } from '~/composables/useRealtime'
import { uploadFile } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useCollabRecovery } from '~/composables/useCollabRecovery'
import { toast } from '~/composables/useToast'
import { HANDLES, angleFromCenter, boundsOf, marquee, normalizeAngle, resizeBox, rotatedBounds, snap, type Box, type Guide, type Handle, type Rect } from '~/slides/geometry'
import {
  SLIDES_SCHEMA, addElement, alignElements, createSlide, createUndo, deleteElements, deleteSlide, distributeElements, duplicateElements, duplicateSlide, expandGroups, groupElements, initDoc,
  isNewerThanClient, listSlides, metaOf, moveSlide, moveZ, slideSize, slidesOf, setSlideBackground, textOf, ungroupElements, updateElement, updateElements, type ElementPatch, type SlideElement, type ZMove,
} from '~/slides/model'
import type { AlignMode } from '~/slides/geometry'
import SlideView from './SlideView.vue'
import TextEdit from './TextEdit.vue'
import Inspector from './Inspector.vue'

/**
 * Présentation d'une page (ADR 0022) : rail de miniatures, canevas et panneau de propriétés, édités
 * à plusieurs dans le document Yjs « slides:<page> ». Annulation par personne, présence (sélection et
 * curseur des autres personnes), lecture seule sans droit d'écriture.
 */
const props = defineProps<{ pageId: string; canEdit: boolean; user: { name: string; color: string } }>()
const emit = defineEmits<{
  (e: 'status', s: 'connecting' | 'synced' | 'offline' | 'denied'): void
  (e: 'collaborators', users: { name: string; color: string }[]): void
}>()
const { t } = useI18n()
const config = useRuntimeConfig()

const ydoc = new Y.Doc()
const collabUrl = (config.public.collabUrl as string) || `${socketBase()}/collab`
const provider = new HocuspocusProvider({ url: `${collabUrl}${collabUrl.includes('?') ? '&' : '?'}schema=${SLIDES_SCHEMA}`, name: `slides:${props.pageId}`, document: ydoc })
const undo = createUndo(ydoc)
const synced = ref(false)
const newer = ref(false)
const canWrite = computed(() => props.canEdit && synced.value && !newer.value)

// --- État du document, relu (au plus une fois par image) à chaque changement ---
const tick = ref(0)
let raf = 0
const bump = () => { if (!raf) raf = requestAnimationFrame(() => { raf = 0; tick.value++ }) }
slidesOf(ydoc).observeDeep(bump)
metaOf(ydoc).observe(bump)
const slides = computed(() => { void tick.value; return listSlides(ydoc) })
const size = computed(() => { void tick.value; return slideSize(ydoc) })
const cur = ref<string>('')
const sel = ref<string[]>([])
const editingId = ref<string | null>(null)
const textEditor = shallowRef<TEditor | null>(null)
const curSlide = computed(() => slides.value.find((s) => s.id === cur.value) ?? slides.value[0] ?? null)
const selected = computed(() => (curSlide.value ? curSlide.value.elements.filter((e) => sel.value.includes(e.id)) : []))
const canUndo = ref(false)
const canRedo = ref(false)
const syncUndo = () => { canUndo.value = undo.undoStack.length > 0; canRedo.value = undo.redoStack.length > 0 }
undo.on('stack-item-added', syncUndo)
undo.on('stack-item-popped', syncUndo)
undo.on('stack-cleared', syncUndo)

watch(slides, (list) => {
  if (!list.length) return
  if (!list.some((s) => s.id === cur.value)) { cur.value = list[0]!.id; sel.value = [] }
  const ids = new Set(curSlide.value?.elements.map((e) => e.id))
  const keep = sel.value.filter((i) => ids.has(i))
  if (keep.length !== sel.value.length) sel.value = keep
  if (editingId.value && !ids.has(editingId.value)) editingId.value = null
}, { immediate: true })
watch(cur, () => { sel.value = []; editingId.value = null })

provider.on('synced', () => {
  synced.value = true
  emit('status', 'synced')
  if (isNewerThanClient(ydoc)) { newer.value = true; return }
  if (props.canEdit && slidesOf(ydoc).size === 0) initDoc(ydoc)
  undo.clear()
  syncUndo()
})
const recovery = useCollabRecovery(provider, { onLost: () => emit('status', 'offline') })
provider.on('authenticationFailed', ({ reason }: { reason?: string }) => {
  if (recovery.classify(reason) === 'transient') { emit('status', 'offline'); return }
  emit('status', 'denied')
})
provider.on('status', ({ status }: { status: string }) => { if (status === 'disconnected') emit('status', 'offline') })
onMounted(() => emit('status', 'connecting'))

// --- Présence ---
interface Peer { id: number; name: string; color: string; slide: string; sel: string[]; x?: number; y?: number }
const peers = ref<Peer[]>([])
provider.setAwarenessField('user', props.user)
provider.on('awarenessUpdate', () => {
  const mine = provider.awareness?.clientID
  const list: Peer[] = []
  provider.awareness?.getStates().forEach((s: any, id: number) => {
    if (id === mine || !s.user) return
    const p = s.slides ?? {}
    list.push({ id, name: s.user.name, color: s.user.color, slide: p.slide ?? '', sel: Array.isArray(p.sel) ? p.sel : [], x: p.x, y: p.y })
  })
  peers.value = list
  const seen = new Set<string>()
  emit('collaborators', list.filter((p) => !seen.has(p.name) && seen.add(p.name)).map((p) => ({ name: p.name, color: p.color })))
})
let pointer: { x: number; y: number } | null = null
watch([cur, sel], () => announce(), { deep: true })
let announceAt = 0
function announce() {
  const now = Date.now()
  if (now - announceAt < 30 && pointer) return
  announceAt = now
  provider.setAwarenessField('slides', { slide: cur.value, sel: sel.value, x: pointer?.x, y: pointer?.y })
}
const peersOn = (id: string) => peers.value.filter((p) => p.slide === id)
const peersHere = computed(() => (curSlide.value ? peersOn(curSlide.value.id) : []))
/** Cadres de sélection des autres personnes sur la diapositive affichée. */
const peerBoxes = computed(() => {
  const out: { key: string; name: string; color: string; style: Record<string, string> }[] = []
  for (const p of peersHere.value) {
    for (const id of p.sel) {
      const e = curSlide.value?.elements.find((x) => x.id === id)
      if (e) out.push({ key: `${p.id}-${id}`, name: p.name, color: p.color, style: { left: `${e.x}px`, top: `${e.y}px`, width: `${e.w}px`, height: `${e.h}px`, transform: `rotate(${e.rotation}deg)`, outlineColor: p.color, outlineWidth: `${2 / Math.max(0.1, scale.value)}px` } })
    }
  }
  return out
})
const initials = (n: string) => n.trim().slice(0, 1).toUpperCase() || '?'

// --- Ajustement du canevas au conteneur ---
const stageBox = ref<HTMLElement | null>(null)
const view = ref<InstanceType<typeof SlideView> | null>(null)
const box = reactive({ w: 800, h: 450 })
let ro: ResizeObserver | null = null
onMounted(() => {
  const measure = () => { if (stageBox.value) { box.w = stageBox.value.clientWidth; box.h = stageBox.value.clientHeight } }
  measure()
  ro = new ResizeObserver(measure)
  if (stageBox.value) ro.observe(stageBox.value)
})
const frameStyle = computed(() => {
  const margin = 24
  const s = Math.max(0.05, Math.min((box.w - margin * 2) / size.value.w, (box.h - margin * 2) / size.value.h))
  return { width: `${Math.floor(size.value.w * s)}px` }
})
const scale = computed(() => view.value?.scale ?? 1)
function frameRect(): DOMRect | null { return (view.value?.root as HTMLElement | null)?.getBoundingClientRect() ?? null }
function toCanvas(e: { clientX: number; clientY: number }) {
  const r = frameRect()
  if (!r) return { x: 0, y: 0 }
  const s = r.width / size.value.w
  return { x: (e.clientX - r.left) / s, y: (e.clientY - r.top) / s }
}

// --- Actions sur les diapositives ---
function addSlide() {
  if (!canWrite.value) return
  const id = createSlide(ydoc, { afterId: curSlide.value?.id })
  if (id) cur.value = id
  else toastLimit()
}
function copySlide() {
  if (!canWrite.value || !curSlide.value) return
  const id = duplicateSlide(ydoc, curSlide.value.id)
  if (id) cur.value = id
  else toastLimit()
}
function removeSlide(id = curSlide.value?.id) {
  if (!canWrite.value || !id) return
  const next = deleteSlide(ydoc, id)
  if (id === cur.value && next) cur.value = next
}
function shiftSlide(id: string, delta: number) {
  if (!canWrite.value) return
  const at = slides.value.findIndex((s) => s.id === id)
  const to = at + delta
  if (at < 0 || to < 0 || to >= slides.value.length) return
  moveSlide(ydoc, id, to)
}
function toastLimit() { toast.error(t('slides.limit')) }
const dragId = ref<string | null>(null)
const dropAt = ref<number | null>(null)
function onRailOver(e: DragEvent, i: number) {
  if (!dragId.value) return
  e.preventDefault()
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
  dropAt.value = e.clientY > r.top + r.height / 2 ? i + 1 : i
}
function onRailDrop(e: DragEvent) {
  e.preventDefault()
  const id = dragId.value
  const at = dropAt.value
  dragId.value = null
  dropAt.value = null
  if (!id || at === null || !canWrite.value) return
  const from = slides.value.findIndex((s) => s.id === id)
  moveSlide(ydoc, id, from < at ? at - 1 : at)
}
function onRailKey(e: KeyboardEvent, id: string) {
  if (!canWrite.value) return
  if (e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) { e.preventDefault(); shiftSlide(id, e.key === 'ArrowUp' ? -1 : 1) }
  else if (e.key === 'Delete' || e.key === 'Backspace') { e.preventDefault(); removeSlide(id) }
}

// --- Ajout d'éléments ---
function place(w: number, h: number) {
  const n = (curSlide.value?.elements.length ?? 0) % 8
  return { x: Math.round((size.value.w - w) / 2) + n * 24, y: Math.round((size.value.h - h) / 2) + n * 24, w, h }
}
function insert(spec: Parameters<typeof addElement>[2]) {
  if (!canWrite.value || !curSlide.value) return null
  const id = addElement(ydoc, curSlide.value.id, spec)
  if (id) sel.value = [id]
  else toastLimit()
  undo.stopCapturing()
  return id
}
function addText() {
  const id = insert({ type: 'text', ...place(800, 140), props: { fontSize: 48, color: 'default', align: 'left' } })
  if (id) editingId.value = id
}
const addRect = () => insert({ type: 'rect', ...place(480, 270), props: { fill: 'indigo', stroke: 'none', strokeWidth: 0, radius: 16 } })
const addEllipse = () => insert({ type: 'ellipse', ...place(360, 360), props: { fill: 'teal', stroke: 'none', strokeWidth: 0 } })
const addLine = () => insert({ type: 'line', ...place(600, 24), props: { stroke: 'default', strokeWidth: 6 } })
const fileEl = ref<HTMLInputElement | null>(null)
async function onFile(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file || !file.type.startsWith('image/') || !canWrite.value) return
  const r = await uploadFile(file)
  const dims = await new Promise<{ w: number; h: number }>((res) => {
    const img = new window.Image()
    img.onload = () => res({ w: img.naturalWidth || 640, h: img.naturalHeight || 360 })
    img.onerror = () => res({ w: 640, h: 360 })
    img.src = r.url
  })
  const k = Math.min(1, 1000 / dims.w, 700 / dims.h)
  insert({ type: 'image', ...place(Math.round(dims.w * k), Math.round(dims.h * k)), alt: r.name, props: { src: r.url } })
}

// --- Sélection et manipulation au pointeur ---
type Drag =
  | { kind: 'move'; start: { x: number; y: number }; boxes: Map<string, Box>; others: Rect[]; sz: { w: number; h: number } }
  | { kind: 'resize'; handle: Handle; id: string; box: Box; start: { x: number; y: number } }
  | { kind: 'rotate'; id: string; center: { x: number; y: number } }
  | { kind: 'marquee'; start: { x: number; y: number }; base: string[] }
let drag: Drag | null = null
const guides = ref<Guide[]>([])
const marqueeRect = ref<Rect | null>(null)
const boxOf = (e: SlideElement): Box => ({ x: e.x, y: e.y, w: e.w, h: e.h, rotation: e.rotation })

function onStageDown(e: PointerEvent) {
  if (e.button !== 0 || !curSlide.value) return
  const target = e.target as HTMLElement
  if (target.closest('[data-handle]')) return
  const elNode = target.closest('[data-el]') as HTMLElement | null
  const id = elNode?.dataset.el
  if (editingId.value && id === editingId.value) return
  if (editingId.value) editingId.value = null
  ;(document.activeElement as HTMLElement | null)?.blur?.()
  const p = toCanvas(e)
  const slide = curSlide.value
  if (id) {
    const el = slide.elements.find((x) => x.id === id)
    if (!el) return
    let next = sel.value
    if (e.shiftKey || e.ctrlKey || e.metaKey) next = sel.value.includes(id) ? sel.value.filter((i) => i !== id) : [...sel.value, id]
    else if (!sel.value.includes(id)) next = [id]
    sel.value = expandGroups(slide.elements, next)
    if (!canWrite.value || !sel.value.length) return
    const moving = slide.elements.filter((x) => sel.value.includes(x.id) && !x.locked)
    if (!moving.length) return
    undo.stopCapturing()
    drag = {
      kind: 'move', start: p, boxes: new Map(moving.map((x) => [x.id, boxOf(x)])),
      others: slide.elements.filter((x) => !sel.value.includes(x.id)).map((x) => rotatedBounds(boxOf(x))), sz: size.value,
    }
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  } else {
    const base = e.shiftKey ? sel.value : []
    sel.value = base
    if (!canWrite.value) return
    drag = { kind: 'marquee', start: p, base }
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }
}
function onHandleDown(e: PointerEvent, handle: Handle | 'rotate') {
  const el = selected.value[0]
  if (!canWrite.value || selected.value.length !== 1 || !el || el.locked) return
  e.stopPropagation()
  undo.stopCapturing()
  const p = toCanvas(e)
  if (handle === 'rotate') drag = { kind: 'rotate', id: el.id, center: { x: el.x + el.w / 2, y: el.y + el.h / 2 } }
  else drag = { kind: 'resize', handle, id: el.id, box: boxOf(el), start: p }
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
}
function onStageMove(e: PointerEvent) {
  const p = toCanvas(e)
  pointer = { x: Math.round(p.x), y: Math.round(p.y) }
  announce()
  if (!drag || !curSlide.value) return
  const slideId = curSlide.value.id
  if (drag.kind === 'move') {
    let dx = p.x - drag.start.x
    let dy = p.y - drag.start.y
    const startBounds = boundsOf([...drag.boxes.values()])!
    guides.value = []
    if (!e.altKey) {
      const s = snap({ ...startBounds, x: startBounds.x + dx, y: startBounds.y + dy }, drag.others, drag.sz, 8 / Math.max(0.1, scale.value))
      dx += s.dx
      dy += s.dy
      guides.value = s.guides
    }
    updateElements(ydoc, slideId, new Map([...drag.boxes].map(([id, b]) => [id, { x: Math.round(b.x + dx), y: Math.round(b.y + dy) }])))
  } else if (drag.kind === 'resize') {
    const rid = drag.id
    const el = curSlide.value.elements.find((x) => x.id === rid)
    const r = resizeBox(drag.box, drag.handle, p.x - drag.start.x, p.y - drag.start.y, { keepRatio: e.shiftKey || el?.type === 'image', fromCenter: e.altKey })
    updateElement(ydoc, slideId, drag.id, { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.w), h: Math.round(r.h) })
  } else if (drag.kind === 'rotate') {
    updateElement(ydoc, slideId, drag.id, { rotation: normalizeAngle(angleFromCenter(drag.center, p), e.shiftKey ? 15 : 1) })
  } else {
    const rect = { x: drag.start.x, y: drag.start.y, w: p.x - drag.start.x, h: p.y - drag.start.y }
    marqueeRect.value = { x: Math.min(rect.x, rect.x + rect.w), y: Math.min(rect.y, rect.y + rect.h), w: Math.abs(rect.w), h: Math.abs(rect.h) }
    const hit = marquee(rect, curSlide.value.elements.map((x) => ({ id: x.id, box: boxOf(x) })))
    sel.value = expandGroups(curSlide.value.elements, [...new Set([...drag.base, ...hit])])
  }
}
function onStageUp() {
  drag = null
  guides.value = []
  marqueeRect.value = null
  undo.stopCapturing()
}
function onStageLeave() {
  pointer = null
  if (cur.value) provider.setAwarenessField('slides', { slide: cur.value, sel: sel.value })
}
function onDouble(e: MouseEvent) {
  if (!canWrite.value) return
  const id = (e.target as HTMLElement).closest('[data-el]')?.getAttribute('data-el')
  const el = curSlide.value?.elements.find((x) => x.id === id)
  if (el?.type === 'text' && !el.locked) { sel.value = [el.id]; editingId.value = el.id }
}

// --- Actions du panneau et du clavier ---
const slideId = () => curSlide.value?.id ?? ''
function patchSel(p: ElementPatch) {
  if (!canWrite.value || !curSlide.value) return
  const ids = p.locked !== undefined ? sel.value : selected.value.filter((e) => !e.locked).map((e) => e.id)
  undo.stopCapturing()
  updateElements(ydoc, slideId(), new Map(ids.map((i) => [i, p])))
}
function removeSel() {
  if (!canWrite.value || !sel.value.length) return
  deleteElements(ydoc, slideId(), sel.value.filter((i) => !selected.value.find((e) => e.id === i)?.locked))
  sel.value = []
  editingId.value = null
}
function dupSel() {
  if (!canWrite.value || !sel.value.length) return
  undo.stopCapturing()
  sel.value = duplicateElements(ydoc, slideId(), sel.value)
}
let clip: { slide: string; ids: string[] } | null = null
function copySel() { if (sel.value.length) clip = { slide: slideId(), ids: [...sel.value] } }
function pasteClip() {
  if (!canWrite.value || !clip || !slides.value.some((s) => s.id === clip!.slide)) return
  const sameSlide = clip.slide === slideId()
  sel.value = duplicateElements(ydoc, clip.slide, clip.ids, slideId(), sameSlide ? 24 : 0, sameSlide ? 24 : 0)
}
const onZ = (m: ZMove) => canWrite.value && moveZ(ydoc, slideId(), sel.value, m)
const onAlign = (m: AlignMode) => canWrite.value && alignElements(ydoc, slideId(), sel.value, m)
const onDistribute = (a: 'x' | 'y') => canWrite.value && distributeElements(ydoc, slideId(), sel.value, a)
const onGroup = () => canWrite.value && groupElements(ydoc, slideId(), sel.value)
const onUngroup = () => canWrite.value && ungroupElements(ydoc, slideId(), sel.value)
const onBackground = (v: string) => canWrite.value && curSlide.value && setSlideBackground(ydoc, curSlide.value.id, v)
function doUndo() { if (canWrite.value) { editingId.value = null; undo.undo() } }
function doRedo() { if (canWrite.value) { editingId.value = null; undo.redo() } }

function onKey(e: KeyboardEvent) {
  const t0 = e.target as HTMLElement | null
  if (t0?.closest('input, textarea, select, [contenteditable="true"]')) return
  if (!root.value?.contains(t0) && t0 !== document.body) return
  const mod = e.ctrlKey || e.metaKey
  const k = e.key.toLowerCase()
  if (mod && k === 'z') { e.preventDefault(); e.shiftKey ? doRedo() : doUndo(); return }
  if (mod && k === 'y') { e.preventDefault(); doRedo(); return }
  if (mod && k === 'a' && curSlide.value) { e.preventDefault(); sel.value = curSlide.value.elements.map((x) => x.id); return }
  if (mod && k === 'd') { e.preventDefault(); dupSel(); return }
  if (mod && k === 'c') { copySel(); return }
  if (mod && k === 'x') { copySel(); removeSel(); return }
  if (mod && k === 'v') { pasteClip(); return }
  if (mod && k === 'g') { e.preventDefault(); e.shiftKey ? onUngroup() : onGroup(); return }
  if (e.key === 'Escape') { sel.value = []; return }
  if (e.key === 'Delete' || e.key === 'Backspace') { if (sel.value.length) { e.preventDefault(); removeSel() } return }
  if (e.key === 'Enter' && selected.value.length === 1 && selected.value[0]!.type === 'text' && canWrite.value) { e.preventDefault(); editingId.value = selected.value[0]!.id; return }
  if (e.key.startsWith('Arrow') && sel.value.length && canWrite.value) {
    e.preventDefault()
    const step = e.shiftKey ? 10 : 1
    const dx = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0
    const dy = e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0
    updateElements(ydoc, slideId(), new Map(selected.value.filter((x) => !x.locked).map((x) => [x.id, { x: x.x + dx, y: x.y + dy }])))
    return
  }
  const idx = slides.value.findIndex((s) => s.id === cur.value)
  if (e.key === 'PageDown' || (e.key === 'ArrowDown' && !sel.value.length)) { const n = slides.value[idx + 1]; if (n) { e.preventDefault(); cur.value = n.id } }
  else if (e.key === 'PageUp' || (e.key === 'ArrowUp' && !sel.value.length)) { const n = slides.value[idx - 1]; if (n) { e.preventDefault(); cur.value = n.id } }
  else if (e.key === 'Home') { cur.value = slides.value[0]?.id ?? cur.value }
  else if (e.key === 'End') { cur.value = slides.value[slides.value.length - 1]?.id ?? cur.value }
}
const root = ref<HTMLElement | null>(null)
onMounted(() => window.addEventListener('keydown', onKey))

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
  ro?.disconnect()
  if (raf) cancelAnimationFrame(raf)
  recovery.stop()
  undo.destroy()
  provider.destroy()
  ydoc.destroy()
})

// --- Géométrie de la sélection (repère logique) ---
const single = computed(() => (selected.value.length === 1 ? selected.value[0]! : null))
const multiBounds = computed(() => (selected.value.length > 1 ? boundsOf(selected.value.map(boxOf)) : null))
const handleSize = computed(() => 14 / Math.max(0.1, scale.value))
const hpos = (h: Handle, el: SlideElement) => {
  const x = h.includes('w') ? 0 : h.includes('e') ? el.w : el.w / 2
  const y = h.includes('n') ? 0 : h.includes('s') ? el.h : el.h / 2
  return { left: `${x - handleSize.value / 2}px`, top: `${y - handleSize.value / 2}px`, width: `${handleSize.value}px`, height: `${handleSize.value}px` }
}
const cursors: Record<Handle, string> = { nw: 'nwse-resize', se: 'nwse-resize', ne: 'nesw-resize', sw: 'nesw-resize', n: 'ns-resize', s: 'ns-resize', e: 'ew-resize', w: 'ew-resize' }
const frag = (id: string) => textOf(ydoc, slideId(), id)
const elLabel = (e: SlideElement) => e.alt || t(`slides.types.${e.type}`)
const textStyleOf = (e: SlideElement) => ({
  fontSize: `${typeof e.props.fontSize === 'number' ? e.props.fontSize : 48}px`,
  textAlign: (typeof e.props.align === 'string' ? e.props.align : 'left') as string,
})
defineExpose({ undo: doUndo, redo: doRedo })
</script>

<template>
  <div ref="root" class="deck" :class="{ ro: !canWrite }" tabindex="-1">
    <nav class="rail" :aria-label="t('slides.slides')">
      <div v-if="canWrite" class="rail-head">
        <button type="button" class="rb primary" @click="addSlide"><Plus :size="15" /> {{ t('slides.newSlide') }}</button>
        <button type="button" class="rb" :title="t('slides.duplicateSlide')" :aria-label="t('slides.duplicateSlide')" @click="copySlide"><Copy :size="15" /></button>
        <button type="button" class="rb" :title="t('slides.moveUp')" :aria-label="t('slides.moveUp')" :disabled="slides[0]?.id === cur" @click="shiftSlide(cur, -1)"><ChevronUp :size="15" /></button>
        <button type="button" class="rb" :title="t('slides.moveDown')" :aria-label="t('slides.moveDown')" :disabled="slides[slides.length - 1]?.id === cur" @click="shiftSlide(cur, 1)"><ChevronDown :size="15" /></button>
        <button type="button" class="rb danger" :title="t('slides.deleteSlide')" :aria-label="t('slides.deleteSlide')" @click="removeSlide()"><Trash2 :size="15" /></button>
      </div>
      <ol class="rail-list" @dragover.prevent @drop="onRailDrop">
        <li v-for="(s, i) in slides" :key="s.id" class="rail-item" :class="{ on: s.id === cur, before: dropAt === i && dragId, after: dropAt === slides.length && i === slides.length - 1 && dragId }">
          <button
            type="button" class="thumb" :aria-current="s.id === cur ? 'true' : undefined" :aria-label="t('slides.slideN', { n: i + 1 })"
            :draggable="canWrite" @click="cur = s.id" @keydown="onRailKey($event, s.id)"
            @dragstart="dragId = s.id; ($event.dataTransfer as DataTransfer).effectAllowed = 'move'; ($event.dataTransfer as DataTransfer).setData('text/plain', s.id)"
            @dragover="onRailOver($event, i)" @dragend="dragId = null; dropAt = null"
          >
            <span class="num">{{ i + 1 }}</span>
            <span class="mini"><SlideView :doc="ydoc" :slide="s" :size="size" :tick="tick" /></span>
            <span v-if="peersOn(s.id).length" class="who">
              <span v-for="p in peersOn(s.id).slice(0, 3)" :key="p.id" class="dot" :style="{ background: p.color }" :title="p.name">{{ initials(p.name) }}</span>
            </span>
          </button>
        </li>
      </ol>
    </nav>

    <section class="main">
      <div v-if="canWrite" class="bar" role="toolbar" :aria-label="t('slides.tools')">
        <button type="button" class="tb" :title="t('slides.addText')" @click="addText"><Type :size="16" /><span>{{ t('slides.text') }}</span></button>
        <button type="button" class="tb" :title="t('slides.addRect')" :aria-label="t('slides.addRect')" @click="addRect"><Square :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.addEllipse')" :aria-label="t('slides.addEllipse')" @click="addEllipse"><Circle :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.addLine')" :aria-label="t('slides.addLine')" @click="addLine"><Minus :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.addImage')" @click="fileEl?.click()"><ImageIcon :size="16" /><span>{{ t('slides.image') }}</span></button>
        <input ref="fileEl" type="file" accept="image/*" hidden @change="onFile" />
        <span class="sep" />
        <button type="button" class="tb" :title="t('slides.undo')" :aria-label="t('slides.undo')" :disabled="!canUndo" @click="doUndo"><Undo2 :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.redo')" :aria-label="t('slides.redo')" :disabled="!canRedo" @click="doRedo"><Redo2 :size="16" /></button>
      </div>
      <p v-if="newer" class="notice" role="status">{{ t('slides.newerFormat') }}</p>
      <p v-else-if="!canEdit" class="notice" role="status">{{ t('slides.readOnly') }}</p>
      <div ref="stageBox" class="stagebox">
        <div v-if="!synced" class="loading"><UiSpinner /></div>
        <div
          v-if="curSlide" class="frame" :style="frameStyle" role="group" :aria-label="t('slides.canvasLabel', { n: slides.findIndex((s) => s.id === curSlide!.id) + 1 })"
          @pointerdown="onStageDown" @pointermove="onStageMove" @pointerup="onStageUp" @pointercancel="onStageUp" @pointerleave="onStageLeave" @dblclick="onDouble"
        >
          <SlideView ref="view" :doc="ydoc" :slide="curSlide" :size="size" :tick="tick" :editing-id="editingId">
            <template #editor="{ el, textStyle }">
              <TextEdit v-if="frag(el.id)" :fragment="frag(el.id)!" :text-style="{ ...textStyle, ...textStyleOf(el) }" @done="editingId = null" @editor="textEditor = $event" />
            </template>
            <!-- Sélection des autres personnes -->
            <div v-for="b in peerBoxes" :key="b.key" class="peer-box" :style="b.style">
              <span class="peer-tag" :style="{ background: b.color, fontSize: `${12 / Math.max(0.1, scale)}px` }">{{ b.name }}</span>
            </div>
            <template v-for="p in peersHere" :key="`c${p.id}`">
              <div v-if="p.x !== undefined && p.y !== undefined" class="peer-cursor" :style="{ left: `${p.x}px`, top: `${p.y}px`, '--c': p.color, '--k': 1 / Math.max(0.1, scale) }"><span>{{ p.name }}</span></div>
            </template>
            <!-- Ma sélection : cadre, poignées de redimensionnement et de rotation -->
            <template v-if="canWrite && !editingId">
              <div v-if="single" class="sel" :style="{ left: `${single.x}px`, top: `${single.y}px`, width: `${single.w}px`, height: `${single.h}px`, transform: `rotate(${single.rotation}deg)`, outlineWidth: `${2 / Math.max(0.1, scale)}px` }">
                <template v-if="!single.locked">
                  <span v-for="h in HANDLES" :key="h" class="handle" data-handle :style="{ ...hpos(h, single), cursor: cursors[h] }" @pointerdown="onHandleDown($event, h)" @pointermove.stop="onStageMove" @pointerup.stop="onStageUp" />
                  <span class="handle rot" data-handle :style="{ left: `${single.w / 2 - handleSize / 2}px`, top: `${-handleSize * 2.4}px`, width: `${handleSize}px`, height: `${handleSize}px` }" @pointerdown="onHandleDown($event, 'rotate')" @pointermove.stop="onStageMove" @pointerup.stop="onStageUp" />
                </template>
              </div>
              <template v-else-if="multiBounds">
                <div v-for="e in selected" :key="e.id" class="sel thin" :style="{ left: `${e.x}px`, top: `${e.y}px`, width: `${e.w}px`, height: `${e.h}px`, transform: `rotate(${e.rotation}deg)`, outlineWidth: `${1.5 / Math.max(0.1, scale)}px` }" />
                <div class="sel multi" :style="{ left: `${multiBounds.x}px`, top: `${multiBounds.y}px`, width: `${multiBounds.w}px`, height: `${multiBounds.h}px`, outlineWidth: `${2 / Math.max(0.1, scale)}px` }" />
              </template>
            </template>
            <div v-for="(g, i) in guides" :key="`g${i}`" class="guide" :style="g.axis === 'x' ? { left: `${g.pos}px`, top: `${g.from}px`, width: `${2 / Math.max(0.1, scale)}px`, height: `${g.to - g.from}px` } : { top: `${g.pos}px`, left: `${g.from}px`, height: `${2 / Math.max(0.1, scale)}px`, width: `${g.to - g.from}px` }" />
            <div v-if="marqueeRect" class="marquee" :style="{ left: `${marqueeRect.x}px`, top: `${marqueeRect.y}px`, width: `${marqueeRect.w}px`, height: `${marqueeRect.h}px`, outlineWidth: `${1.5 / Math.max(0.1, scale)}px` }" />
          </SlideView>
          <!-- Éléments atteignables au clavier : un bouton invisible par élément -->
          <ul class="kb">
            <li v-for="e in curSlide.elements" :key="e.id"><button type="button" tabindex="0" :aria-pressed="sel.includes(e.id)" @focus="canWrite && !editingId && (sel = [e.id])">{{ elLabel(e) }}</button></li>
          </ul>
        </div>
      </div>
    </section>

    <Inspector
      v-if="curSlide" :slide="curSlide" :selected="selected" :can-write="canWrite" :text-editor="editingId ? textEditor : null"
      @patch="patchSel" @background="onBackground" @z="onZ" @align="onAlign" @distribute="onDistribute" @group="onGroup" @ungroup="onUngroup" @duplicate="dupSel" @delete="removeSel"
    />
  </div>
</template>

<style scoped>
.deck { flex: 1; min-height: 0; display: grid; grid-template-columns: 188px minmax(0, 1fr) 244px; background: var(--bg); outline: none; }
.rail { display: flex; flex-direction: column; min-height: 0; border-right: 1px solid var(--border); background: var(--bg-sidebar); }
.rail-head { display: flex; flex-wrap: wrap; gap: 4px; padding: 8px; border-bottom: 1px solid var(--border); }
.rb { height: 28px; min-width: 28px; padding: 0 6px; display: inline-flex; align-items: center; justify-content: center; gap: 4px; border-radius: var(--r-sm); color: var(--text-2); font-size: var(--fs-sm); }
.rb:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.rb.primary { flex: 1 0 100%; background: var(--accent); color: var(--text-on-accent); }
.rb.primary:hover { background: var(--accent-hover); color: var(--text-on-accent); }
.rb.danger:hover { color: var(--red); }
.rb:disabled { opacity: 0.4; }
.rail-list { list-style: none; margin: 0; padding: 8px; display: flex; flex-direction: column; gap: 8px; overflow-y: auto; flex: 1; }
.rail-item { position: relative; content-visibility: auto; contain-intrinsic-size: auto 100px; }
.rail-item.before::before, .rail-item.after::after { content: ''; position: absolute; left: 0; right: 0; height: 3px; border-radius: 2px; background: var(--accent); }
.rail-item.before::before { top: -5px; } .rail-item.after::after { bottom: -5px; }
.thumb { position: relative; display: block; width: 100%; padding: 0; border-radius: var(--r-md); text-align: left; }
.thumb:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 2px; }
.mini { display: block; pointer-events: none; border-radius: var(--r-sm); overflow: hidden; box-shadow: 0 0 0 1px var(--border-strong); }
.rail-item.on .mini { box-shadow: 0 0 0 2px var(--accent); }
.num { position: absolute; left: 4px; top: 3px; z-index: 2; font-size: var(--fs-xs); color: var(--text-2); background: var(--bg-elevated); padding: 0 4px; border-radius: var(--r-sm); }
.who { position: absolute; right: 4px; bottom: 4px; z-index: 2; display: flex; }
.dot { width: 18px; height: 18px; margin-left: -4px; border-radius: 50%; display: grid; place-items: center; font-size: 10px; font-weight: 700; color: #fff; box-shadow: 0 0 0 2px var(--bg-sidebar); }
.main { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
.bar { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; padding: 6px 10px; border-bottom: 1px solid var(--border); }
.tb { height: 30px; min-width: 30px; padding: 0 8px; display: inline-flex; align-items: center; justify-content: center; gap: 6px; border-radius: var(--r-sm); color: var(--text-2); font-size: var(--fs-sm); }
.tb:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.tb:disabled { opacity: 0.4; }
.tb:focus-visible, .rb:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 1px; }
.sep { width: 1px; height: 18px; background: var(--border); margin: 0 4px; }
.notice { margin: 0; padding: 6px 12px; font-size: var(--fs-sm); color: var(--text-2); background: var(--bg-hover); }
.stagebox { position: relative; flex: 1; min-height: 0; display: grid; place-items: center; overflow: hidden; background: var(--bg-column); }
.loading { position: absolute; inset: 0; display: grid; place-items: center; }
.frame { position: relative; box-shadow: 0 0 0 1px var(--border-strong), 0 8px 30px rgb(0 0 0 / 0.25); touch-action: none; user-select: none; }
.ro .frame { touch-action: auto; }
.kb { position: absolute; left: 0; top: 0; margin: 0; padding: 0; list-style: none; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
.sel { position: absolute; outline: solid var(--accent); pointer-events: none; }
.sel.thin { outline-color: var(--accent-text); opacity: 0.7; }
.sel.multi { outline-style: dashed; }
.handle { position: absolute; pointer-events: auto; background: var(--bg-elevated); border: 2px solid var(--accent); border-radius: 3px; box-sizing: border-box; touch-action: none; }
.handle.rot { border-radius: 50%; cursor: grab; background: var(--accent); }
.guide { position: absolute; background: var(--pal-pink, #f27ab5); pointer-events: none; z-index: 9999; }
.marquee { position: absolute; outline: solid var(--accent); background: var(--accent-soft); pointer-events: none; }
.peer-box { position: absolute; outline-style: solid; pointer-events: none; }
.peer-tag { position: absolute; left: 0; top: 0; transform: translateY(-100%); padding: 1px 6px; border-radius: 3px; color: #fff; white-space: nowrap; }
.peer-cursor { position: absolute; width: 0; height: 0; pointer-events: none; z-index: 9998; transform: scale(var(--k)); transform-origin: 0 0; }
.peer-cursor::before { content: ''; position: absolute; left: 0; top: 0; width: 12px; height: 12px; background: var(--c); clip-path: polygon(0 0, 100% 45%, 45% 55%, 35% 100%); }
.peer-cursor span { position: absolute; left: 12px; top: 12px; padding: 1px 6px; border-radius: 3px; background: var(--c); color: #fff; font-size: 12px; white-space: nowrap; }
@media (max-width: 1100px) { .deck { grid-template-columns: 150px minmax(0, 1fr) 210px; } }
@media (max-width: 720px) {
  .deck { grid-template-columns: 1fr; grid-template-rows: minmax(0, 1fr) auto; }
  .deck :deep(.ins) { display: none; }
  .main { order: 1; }
  .rail { order: 2; flex-direction: row; border-right: 0; border-top: 1px solid var(--border); }
  .rail-head { border-bottom: 0; flex-wrap: nowrap; }
  .rail-list { flex-direction: row; overflow-x: auto; overflow-y: hidden; }
  .rail-item { width: 120px; flex: none; }
}
</style>
