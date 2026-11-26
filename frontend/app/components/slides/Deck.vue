<script setup lang="ts">
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import type { Editor as TEditor } from '@tiptap/vue-3'
import {
  Circle, ChevronDown, ChevronRight, ChevronUp, Copy, FolderPlus, Grid3x3, Image as ImageIcon, MessageSquare, MessageSquarePlus, Minus, MoveRight, Pencil, Plus, Redo2, Shapes, Square, Trash2, Type, Undo2, X, ZoomIn, ZoomOut,
} from 'lucide-vue-next'
import { useLocalStorage } from '@vueuse/core'
import { socketBase } from '~/composables/useRealtime'
import { uploadFile } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useCollabRecovery } from '~/composables/useCollabRecovery'
import { useComments } from '~/composables/useComments'
import { useWorkspace } from '~/composables/useWorkspace'
import { promptDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { HANDLES, angleFromCenter, boundsOf, marquee, normalizeAngle, resizeBox, rotatedBounds, snap, type Box, type Guide, type Handle, type Rect } from '~/slides/geometry'
import {
  SLIDES_SCHEMA, addElement, alignElements, createSlide, createUndo, deleteElements, deleteSlide, distributeElements, duplicateElements, duplicateSlide, expandGroups, groupElements, initDoc,
  groupSlides, isNewerThanClient, listSlides, metaOf, moveSlide, moveZ, restoreFrom, setSection, slideSize, slidesOf, setSlideBackground, textOf, ungroupElements, updateElement, updateElements, type ElementPatch, type SlideElement, type ZMove,
} from '~/slides/model'
import { anchorCounts, anchorTarget, elementAnchor, parseAnchor, slideAnchor } from '~/slides/comments'
import { GRID, midpoint, distance, pinchZoom, snapMove, snapRect, stepZoom, wheelZoom, clampZoom } from '~/slides/view'
import type { AlignMode } from '~/slides/geometry'
import SlideView from './SlideView.vue'
import PresentLauncher from './PresentLauncher.vue'
import PresentPanel from './PresentPanel.vue'
import TextEdit from './TextEdit.vue'
import Inspector from './Inspector.vue'
import LazyThumb from './LazyThumb.vue'
import SlidesHistory from './History.vue'
import IconPicker from './IconPicker.vue'

/**
 * Présentation d'une page (ADR 0023) : rail de miniatures, canevas et panneau de propriétés, édités
 * à plusieurs dans le document Yjs « slides:<page> ». Annulation par personne, présence (sélection et
 * curseur des autres personnes), lecture seule sans droit d'écriture.
 */
const props = defineProps<{ pageId: string; canEdit: boolean; canComment?: boolean; user: { name: string; color: string } }>()
const emit = defineEmits<{
  (e: 'status', s: 'connecting' | 'synced' | 'offline' | 'denied'): void
  (e: 'collaborators', users: { name: string; color: string }[]): void
  /** Plan des diapositives (identifiants, noms d'éléments) : libellés des commentaires ancrés, côté page. */
  (e: 'outline', outline: { id: string; elements: { id: string; label: string }[] }[]): void
}>()
/** Historique des versions (ouvert depuis le menu de la page). */
const historyOpen = defineModel<boolean>('historyOpen', { default: false })
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

/** Présentation neuve : sa première diapositive est créée par la première personne qui peut modifier. */
function ensureInit() {
  if (synced.value && props.canEdit && !newer.value && slidesOf(ydoc).size === 0) initDoc(ydoc)
}
watch(() => props.canEdit, ensureInit)
provider.on('synced', () => {
  synced.value = true
  emit('status', 'synced')
  if (isNewerThanClient(ydoc)) { newer.value = true; return }
  ensureInit()
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
interface Peer { id: number; name: string; color: string; slide: string; sel: string[]; x?: number; y?: number; editing?: string }
const peers = ref<Peer[]>([])
provider.setAwarenessField('user', props.user)
provider.on('awarenessUpdate', () => {
  const mine = provider.awareness?.clientID
  const list: Peer[] = []
  provider.awareness?.getStates().forEach((s: any, id: number) => {
    if (id === mine || !s.user) return
    const p = s.slides ?? {}
    list.push({ id, name: s.user.name, color: s.user.color, slide: p.slide ?? '', sel: Array.isArray(p.sel) ? p.sel : [], x: p.x, y: p.y, editing: typeof p.editing === 'string' ? p.editing : undefined })
  })
  peers.value = list
  const seen = new Set<string>()
  emit('collaborators', list.filter((p) => !seen.has(p.name) && seen.add(p.name)).map((p) => ({ name: p.name, color: p.color })))
})
let pointer: { x: number; y: number } | null = null
watch([cur, sel, editingId], () => { announceAt = 0; announce() }, { deep: true })
let announceAt = 0
function announce() {
  const now = Date.now()
  if (now - announceAt < 30 && pointer) return
  announceAt = now
  provider.setAwarenessField('slides', { slide: cur.value, sel: sel.value, editing: editingId.value ?? undefined, x: pointer?.x, y: pointer?.y })
}
const peersOn = (id: string) => peers.value.filter((p) => p.slide === id)
const peersHere = computed(() => (curSlide.value ? peersOn(curSlide.value.id) : []))
/** Cadres de sélection des autres personnes sur la diapositive affichée. */
const peerBoxes = computed(() => {
  const out: { key: string; name: string; color: string; editing: boolean; style: Record<string, string> }[] = []
  for (const p of peersHere.value) {
    for (const id of p.sel) {
      const e = curSlide.value?.elements.find((x) => x.id === id)
      if (e) out.push({ key: `${p.id}-${id}`, name: p.name, color: p.color, editing: p.editing === id, style: { left: `${e.x}px`, top: `${e.y}px`, width: `${e.w}px`, height: `${e.h}px`, transform: `rotate(${e.rotation}deg)`, outlineColor: p.color, outlineWidth: `${2 / Math.max(0.1, scale.value)}px` } })
    }
  }
  return out
})
const initials = (n: string) => n.trim().slice(0, 1).toUpperCase() || '?'
/** Zones de texte de la diapositive que d'autres personnes sont en train de modifier : affichées en direct, avec leur curseur. */
const liveIds = computed(() => [...new Set(peersHere.value.map((p) => p.editing).filter((id): id is string => !!id && !!curSlide.value?.elements.some((e) => e.id === id && e.type === 'text')))])
/** Identité d'un fragment de texte : change si la zone est remplacée (restauration d'une version), pour recréer l'éditeur. */
const fragKey = (f: Y.XmlFragment | null) => { const it = (f as unknown as { _item?: { id: { client: number; clock: number } } } | null)?._item; return it ? `${it.id.client}:${it.id.clock}` : 'root' }

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
/** Zoom : « fit » ajuste la diapositive à la zone ; sinon un nombre (1 = taille réelle). */
const zoom = ref<'fit' | number>('fit')
const fitScale = computed(() => {
  const margin = 24
  return Math.max(0.05, Math.min((box.w - margin * 2) / size.value.w, (box.h - margin * 2) / size.value.h))
})
const zoomValue = computed(() => (zoom.value === 'fit' ? fitScale.value : zoom.value))
const frameStyle = computed(() => ({ width: `${Math.floor(size.value.w * zoomValue.value)}px` }))
const setZoom = (z: number) => { zoom.value = clampZoom(z) }
const zoomBy = (dir: 1 | -1) => setZoom(stepZoom(zoomValue.value, dir))
const zoomPct = computed(() => `${Math.round(zoomValue.value * 100)}%`)
function onWheel(e: WheelEvent) {
  if (!(e.ctrlKey || e.metaKey)) return
  e.preventDefault()
  setZoom(wheelZoom(zoomValue.value, e.deltaY))
}
/** Grille magnétique (réglage personnel). */
const grid = useLocalStorage<boolean>('looty.slides.grid', false)

// Gestes tactiles : pincement pour zoomer, deux doigts pour déplacer la vue.
const touches = new Map<number, { x: number; y: number }>()
let gesture: { dist: number; zoom: number; mid: { x: number; y: number } } | null = null
const inGesture = () => gesture !== null
function onTouchDown(e: PointerEvent) {
  if (e.pointerType !== 'touch') return
  touches.set(e.pointerId, { x: e.clientX, y: e.clientY })
  if (touches.size === 2) {
    const [a, b] = [...touches.values()] as [{ x: number; y: number }, { x: number; y: number }]
    gesture = { dist: distance(a, b), zoom: zoomValue.value, mid: midpoint(a, b) }
    // Un geste à deux doigts n'est jamais un déplacement d'élément.
    drag = null
    guides.value = []
    marqueeRect.value = null
  }
}
function onTouchMove(e: PointerEvent) {
  if (e.pointerType !== 'touch' || !touches.has(e.pointerId)) return
  touches.set(e.pointerId, { x: e.clientX, y: e.clientY })
  if (!gesture || touches.size < 2) return
  e.stopPropagation()
  const [a, b] = [...touches.values()] as [{ x: number; y: number }, { x: number; y: number }]
  setZoom(pinchZoom(gesture.zoom, gesture.dist, distance(a, b)))
  const m = midpoint(a, b)
  if (stageBox.value) { stageBox.value.scrollLeft -= m.x - gesture.mid.x; stageBox.value.scrollTop -= m.y - gesture.mid.y }
  gesture.mid = m
}
function onTouchUp(e: PointerEvent) {
  if (!touches.delete(e.pointerId)) return
  if (touches.size < 2) gesture = null
}
const scale = computed(() => view.value?.scale ?? zoomValue.value)
function frameRect(): DOMRect | null { return (view.value?.root as HTMLElement | null)?.getBoundingClientRect() ?? null }
function toCanvas(e: { clientX: number; clientY: number }) {
  const r = frameRect()
  if (!r) return { x: 0, y: 0 }
  const s = r.width / size.value.w
  return { x: (e.clientX - r.left) / s, y: (e.clientY - r.top) / s }
}

// --- Commentaires ancrés (panneau de la page, table des commentaires de la page) ---
const { wsId } = useWorkspace()
const { threads } = useComments(wsId, ref('page'), computed(() => props.pageId))
const counts = computed(() => anchorCounts(threads.value.map((th) => ({ anchor: th.root.anchor, resolved: !!th.root.resolved_at }))))
const slideComments = (id: string) => counts.value.bySlide.get(id) ?? 0
const elementBubbles = computed(() => {
  const out: { id: string; n: number; style: Record<string, string> }[] = []
  for (const e of curSlide.value?.elements ?? []) {
    const n = counts.value.byElement.get(`${curSlide.value!.id}:${e.id}`) ?? 0
    if (n) out.push({ id: e.id, n, style: { left: `${e.x + e.w}px`, top: `${e.y}px`, '--k': String(1 / Math.max(0.1, scale.value)) } })
  }
  return out
})
const slideOnlyComments = computed(() => curSlide.value ? threads.value.filter((th) => !th.root.resolved_at && th.root.anchor === slideAnchor(curSlide.value!.id)).length : 0)
function commentTarget(): string | null {
  if (!curSlide.value) return null
  return single.value ? elementAnchor(curSlide.value.id, single.value.id) : slideAnchor(curSlide.value.id)
}
function startComment() {
  const a = commentTarget()
  if (a) window.dispatchEvent(new CustomEvent('looty:comment-anchor', { detail: { anchor: a } }))
}
function openThreadsAt(anchor: string) {
  const th = threads.value.find((x) => !x.root.resolved_at && x.root.anchor === anchor)
  if (th) window.dispatchEvent(new CustomEvent('looty:comment-open', { detail: { id: th.root.id } }))
}
/** Un fil est choisi dans le panneau : on affiche sa diapositive et on sélectionne son élément. */
function onGoto(e: Event) {
  const p = parseAnchor((e as CustomEvent).detail?.anchor)
  const t = anchorTarget((e as CustomEvent).detail?.anchor, slides.value)
  if (!p || t.state === 'missing') return
  cur.value = p.slide
  if (p.kind === 'element') nextTick(() => { sel.value = [p.element] })
}
onMounted(() => window.addEventListener('looty:slide-goto', onGoto))
onBeforeUnmount(() => window.removeEventListener('looty:slide-goto', onGoto))
const outline = computed(() => slides.value.map((s) => ({ id: s.id, elements: s.elements.map((e) => ({ id: e.id, label: e.alt || t(`slides.types.${e.type}`) })) })))
watch(() => JSON.stringify(outline.value), () => emit('outline', outline.value), { immediate: true })

// --- Sections du rail ---
const groups = computed(() => groupSlides(slides.value))
const indexOf = computed(() => new Map(slides.value.map((s, i) => [s.id, i])))
const collapsed = ref<Set<string>>(new Set())
function toggleSection(id: string) {
  const next = new Set(collapsed.value)
  if (!next.delete(id)) next.add(id)
  collapsed.value = next
}
watch(cur, (id) => {
  const g = groups.value.find((x) => x.ids.includes(id))
  if (g?.start && collapsed.value.has(g.start)) toggleSection(g.start)
})
async function addSection() {
  if (!canWrite.value || !curSlide.value) return
  const id = curSlide.value.id
  const name = await promptDialog({ title: t('slides.collab.sectionName'), initial: curSlide.value.section || '', confirmLabel: t('common.save') })
  if (name) setSection(ydoc, id, name)
}
async function renameSection(start: string, title: string) {
  if (!canWrite.value) return
  const name = await promptDialog({ title: t('slides.collab.sectionName'), initial: title, confirmLabel: t('common.save') })
  if (name !== null) setSection(ydoc, start, name)
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
const addArrow = () => insert({ type: 'arrow', ...place(600, 64), props: { stroke: 'default', strokeWidth: 8, head: 'end' } })
const iconOpen = ref(false)
const addIcon = (name: string) => insert({ type: 'icon', ...place(200, 200), alt: name.replace(/([a-z0-9])([A-Z])/g, '$1 $2'), props: { name, color: 'default', strokeWidth: 2 } })
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
  if (inGesture()) return
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
    if (grid.value && !e.altKey) {
      const g = snapMove(startBounds, dx, dy)
      dx = g.dx
      dy = g.dy
    } else if (!e.altKey) {
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
    const g = grid.value && !e.altKey && !el?.rotation ? snapRect(r) : r
    updateElement(ydoc, slideId, drag.id, { x: Math.round(g.x), y: Math.round(g.y), w: Math.round(g.w), h: Math.round(g.h) })
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
function onRestore(snapshot: Y.Doc) {
  if (!canWrite.value) return
  editingId.value = null
  undo.stopCapturing()
  if (!restoreFrom(ydoc, snapshot)) toast.error(t('slides.newerFormat'))
  undo.stopCapturing()
}
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
  if (mod && (e.key === '+' || e.key === '=')) { e.preventDefault(); zoomBy(1); return }
  if (mod && (e.key === '-' || e.key === '_')) { e.preventDefault(); zoomBy(-1); return }
  if (mod && e.key === '0') { e.preventDefault(); zoom.value = 'fit'; return }
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
        <button type="button" class="rb" :title="t('slides.collab.addSection')" :aria-label="t('slides.collab.addSection')" data-add-section @click="addSection"><FolderPlus :size="15" /></button>
        <button type="button" class="rb danger" :title="t('slides.deleteSlide')" :aria-label="t('slides.deleteSlide')" @click="removeSlide()"><Trash2 :size="15" /></button>
      </div>
      <ol class="rail-list" @dragover.prevent @drop="onRailDrop">
        <template v-for="g in groups" :key="g.start ?? 'first'">
          <li v-if="g.start" class="rail-sec" data-section>
            <button type="button" class="sec-toggle" :aria-expanded="!collapsed.has(g.start)" :aria-label="g.title" @click="toggleSection(g.start)">
              <component :is="collapsed.has(g.start) ? ChevronRight : ChevronDown" :size="14" />
              <span class="sec-title truncate">{{ g.title }}</span>
              <span class="sec-n">{{ g.ids.length }}</span>
            </button>
            <button v-if="canWrite" type="button" class="sec-edit" :title="t('slides.collab.renameSection')" :aria-label="t('slides.collab.renameSection')" @click="renameSection(g.start, g.title)"><Pencil :size="12" /></button>
            <button v-if="canWrite" type="button" class="sec-edit" :title="t('slides.collab.removeSection')" :aria-label="t('slides.collab.removeSection')" data-remove-section @click="setSection(ydoc, g.start, '')"><X :size="12" /></button>
          </li>
          <template v-if="!g.start || !collapsed.has(g.start)">
            <li v-for="s in g.ids.map((id) => slides[indexOf.get(id)!]!)" :key="s.id" class="rail-item" :class="{ on: s.id === cur, before: dropAt === indexOf.get(s.id) && dragId, after: dropAt === slides.length && indexOf.get(s.id) === slides.length - 1 && dragId }">
              <button
                type="button" class="thumb" :aria-current="s.id === cur ? 'true' : undefined" :aria-label="t('slides.slideN', { n: indexOf.get(s.id)! + 1 })"
                :draggable="canWrite" @click="cur = s.id" @keydown="onRailKey($event, s.id)"
                @dragstart="dragId = s.id; ($event.dataTransfer as DataTransfer).effectAllowed = 'move'; ($event.dataTransfer as DataTransfer).setData('text/plain', s.id)"
                @dragover="onRailOver($event, indexOf.get(s.id)!)" @dragend="dragId = null; dropAt = null"
              >
                <span class="num">{{ indexOf.get(s.id)! + 1 }}</span>
                <span class="mini"><LazyThumb :doc="ydoc" :slide="s" :size="size" :tick="tick" /></span>
                <span v-if="slideComments(s.id)" class="cbadge" data-slide-comments :title="t('slides.collab.openComments', { count: slideComments(s.id) })"><MessageSquare :size="10" /> {{ slideComments(s.id) }}</span>
                <span v-if="peersOn(s.id).length" class="who" data-slide-peers>
                  <span v-for="p in peersOn(s.id).slice(0, 3)" :key="p.id" class="dot" :style="{ background: p.color }" :title="p.name">{{ initials(p.name) }}</span>
                  <span v-if="peersOn(s.id).length > 3" class="dot more">+{{ peersOn(s.id).length - 3 }}</span>
                </span>
              </button>
            </li>
          </template>
        </template>
      </ol>
    </nav>

    <section class="main">
      <div class="pbar"><PresentLauncher :doc="ydoc" :provider="provider" :page-id="pageId" :current-id="cur" :can-edit="canEdit" :ready="synced" /></div>
      <div class="bar" role="toolbar" :aria-label="t('slides.tools')">
        <template v-if="canWrite">
        <button type="button" class="tb" :title="t('slides.addText')" @click="addText"><Type :size="16" /><span>{{ t('slides.text') }}</span></button>
        <button type="button" class="tb" :title="t('slides.addRect')" :aria-label="t('slides.addRect')" @click="addRect"><Square :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.addEllipse')" :aria-label="t('slides.addEllipse')" @click="addEllipse"><Circle :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.addLine')" :aria-label="t('slides.addLine')" @click="addLine"><Minus :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.collab.addArrow')" :aria-label="t('slides.collab.addArrow')" data-add-arrow @click="addArrow"><MoveRight :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.collab.addIcon')" :aria-label="t('slides.collab.addIcon')" data-add-icon @click="iconOpen = true"><Shapes :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.addImage')" @click="fileEl?.click()"><ImageIcon :size="16" /><span>{{ t('slides.image') }}</span></button>
        <input ref="fileEl" type="file" accept="image/*" hidden @change="onFile" />
        <span class="sep" />
        <button type="button" class="tb" :title="t('slides.undo')" :aria-label="t('slides.undo')" :disabled="!canUndo" @click="doUndo"><Undo2 :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.redo')" :aria-label="t('slides.redo')" :disabled="!canRedo" @click="doRedo"><Redo2 :size="16" /></button>
        <span class="sep" />
        </template>
        <button v-if="canComment" type="button" class="tb" :title="single ? t('slides.collab.commentElement') : t('slides.collab.commentSlide')" :aria-label="single ? t('slides.collab.commentElement') : t('slides.collab.commentSlide')" data-add-comment @click="startComment"><MessageSquarePlus :size="16" /></button>
        <span class="spacer" />
        <button type="button" class="tb" :class="{ on: grid }" :title="t('slides.collab.grid')" :aria-label="t('slides.collab.grid')" :aria-pressed="grid" data-grid @click="grid = !grid"><Grid3x3 :size="16" /></button>
        <button type="button" class="tb" :title="t('slides.collab.zoomOut')" :aria-label="t('slides.collab.zoomOut')" data-zoom-out @click="zoomBy(-1)"><ZoomOut :size="16" /></button>
        <button type="button" class="tb zoom" :title="t('slides.collab.fit')" :aria-label="t('slides.collab.fit')" data-zoom-fit @click="zoom = 'fit'">{{ zoom === 'fit' ? t('slides.collab.fitShort') : zoomPct }}</button>
        <button type="button" class="tb" :title="t('slides.collab.zoomIn')" :aria-label="t('slides.collab.zoomIn')" data-zoom-in @click="zoomBy(1)"><ZoomIn :size="16" /></button>
        <button type="button" class="tb zoom" :title="t('slides.collab.actualSize')" :aria-label="t('slides.collab.actualSize')" data-zoom-100 @click="setZoom(1)">100%</button>
      </div>
      <p v-if="newer" class="notice" role="status">{{ t('slides.newerFormat') }}</p>
      <p v-else-if="!canEdit" class="notice" role="status">{{ t('slides.readOnly') }}</p>
      <div ref="stageBox" class="stagebox" @wheel="onWheel" @pointerdown.capture="onTouchDown" @pointermove.capture="onTouchMove" @pointerup.capture="onTouchUp" @pointercancel.capture="onTouchUp">
        <div v-if="!synced" class="loading"><UiSpinner /></div>
        <p v-else-if="!curSlide" class="empty">{{ t('slides.empty') }}</p>
        <div
          v-if="curSlide" class="frame" :style="frameStyle" role="group" :aria-label="t('slides.canvasLabel', { n: slides.findIndex((s) => s.id === curSlide!.id) + 1 })"
          @pointerdown="onStageDown" @pointermove="onStageMove" @pointerup="onStageUp" @pointercancel="onStageUp" @pointerleave="onStageLeave" @dblclick="onDouble"
        >
          <SlideView ref="view" :doc="ydoc" :slide="curSlide" :size="size" :tick="tick" :editing-id="editingId" :live-ids="liveIds">
            <template #editor="{ el, textStyle }">
              <TextEdit
                v-if="frag(el.id)" :key="`${el.id}:${el.id === editingId ? 'w' : 'r'}:${fragKey(frag(el.id))}`" :fragment="frag(el.id)!" :provider="provider" :user="user" :readonly="el.id !== editingId"
                :text-style="{ ...textStyle, ...textStyleOf(el) }" @done="editingId = null" @editor="textEditor = $event"
              />
            </template>
            <div v-if="grid" class="grid-ov" :style="{ backgroundSize: `${GRID}px ${GRID}px`, '--lw': `${Math.max(1, 1 / Math.max(0.1, scale))}px` }" />
            <!-- Sélection des autres personnes -->
            <div v-for="b in peerBoxes" :key="b.key" class="peer-box" :style="b.style">
              <span class="peer-tag" :class="{ editing: b.editing }" :style="{ background: b.color, fontSize: `${12 / Math.max(0.1, scale)}px` }">{{ b.name }}<template v-if="b.editing"> · {{ t('slides.collab.editing') }}</template></span>
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
            <button v-for="b in elementBubbles" :key="`cb${b.id}`" type="button" class="cbubble" data-el-comments :style="b.style" :aria-label="t('slides.collab.openComments', { count: b.n })" @pointerdown.stop @click.stop="openThreadsAt(elementAnchor(curSlide.id, b.id))"><MessageSquare :size="12" /> {{ b.n }}</button>
            <button v-if="slideOnlyComments" type="button" class="cbubble slide" data-slide-bubble :style="{ left: '0px', top: '0px', '--k': String(1 / Math.max(0.1, scale)) }" :aria-label="t('slides.collab.openComments', { count: slideOnlyComments })" @pointerdown.stop @click.stop="openThreadsAt(slideAnchor(curSlide.id))"><MessageSquare :size="12" /> {{ slideOnlyComments }}</button>
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
    >
      <PresentPanel :doc="ydoc" :slide="curSlide" :selected="selected" :tick="tick" :can-write="canWrite" />
    </Inspector>
    <SlidesHistory v-model:open="historyOpen" :page-id="pageId" :doc="ydoc" :can-edit="canWrite" @restore="onRestore" />
    <IconPicker v-model:open="iconOpen" @pick="addIcon" />
  </div>
</template>

<style scoped>
.pbar { display: flex; padding: 6px 10px 0; }
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
.stagebox { position: relative; flex: 1; min-height: 0; display: flex; padding: 24px; overflow: auto; background: var(--bg-column); }
.stagebox > .frame { margin: auto; flex: none; }
.spacer { flex: 1; }
.tb.on { background: var(--bg-selected); color: var(--accent-text); }
.tb.zoom { min-width: 44px; font-variant-numeric: tabular-nums; }
.grid-ov { position: absolute; inset: 0; pointer-events: none; z-index: 9990; background-image: linear-gradient(to right, color-mix(in srgb, var(--text) 18%, transparent) var(--lw), transparent var(--lw)), linear-gradient(to bottom, color-mix(in srgb, var(--text) 18%, transparent) var(--lw), transparent var(--lw)); }
.cbubble { position: absolute; z-index: 9997; display: inline-flex; align-items: center; gap: 3px; padding: 2px 7px; border-radius: 999px; background: var(--yellow, #e2a03f); color: #1b1b1b; font-size: 12px; font-weight: 700; transform: translate(-100%, -50%) scale(var(--k)); transform-origin: 100% 50%; box-shadow: 0 1px 4px rgb(0 0 0 / 0.3); cursor: pointer; }
.cbubble.slide { transform: translate(0, 0) scale(var(--k)); transform-origin: 0 0; margin: 12px; }
.cbadge { position: absolute; right: 4px; top: 3px; z-index: 2; display: inline-flex; align-items: center; gap: 2px; padding: 0 5px; border-radius: 999px; background: var(--yellow, #e2a03f); color: #1b1b1b; font-size: 10px; font-weight: 700; }
.rail-sec { display: flex; align-items: center; gap: 2px; margin-top: 4px; }
.sec-toggle { flex: 1; min-width: 0; display: flex; align-items: center; gap: 4px; height: 24px; padding: 0 4px; border-radius: var(--r-sm); color: var(--text-2); font-size: var(--fs-xs); font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; text-align: left; }
.sec-toggle:hover, .sec-edit:hover { background: var(--bg-hover); color: var(--text); }
.sec-title { flex: 1; min-width: 0; }
.sec-n { font-weight: 500; color: var(--text-3); }
.sec-edit { width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); }
.dot.more { background: var(--bg-active); color: var(--text-2); }
.peer-tag.editing { font-weight: 700; }
.empty { margin: auto; color: var(--text-3); }
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
