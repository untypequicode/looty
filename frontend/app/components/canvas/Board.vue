<script setup lang="ts">
import { h, render, getCurrentInstance } from 'vue'
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import { useDebounceFn } from '@vueuse/core'
import CanvasCard from './Card.vue'
import { api as http } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { socketBase, taskKey, boardKey } from '~/composables/useRealtime'
import { useQueryClient } from '@tanstack/vue-query'
import { parseLootyLink, lootyPath } from '~/utils/lootyLinks'

/**
 * Canevas d'une page (façon AFFiNE) : tableau blanc infini Excalidraw, édité à plusieurs.
 * - Les éléments sont dans le document Yjs « canvas:<page> » (Y.Map « elements », un
 *   élément par identifiant, la version la plus récente l'emporte ; images dans « files »).
 * - Un lien Looty (tâche, document, canevas, tableau, événement, projet) devient une carte
 *   vivante, rendue par Vue dans l'élément intégré d'Excalidraw.
 * - Curseurs des autres personnes par l'awareness du fournisseur Yjs.
 * - Aperçu SVG enregistré pour la liste des canevas et les cartes.
 */
const props = defineProps<{ pageId: string; wsId: string; canEdit: boolean; user: { name: string; color: string } }>()
const emit = defineEmits<{
  (e: 'status', s: 'connecting' | 'synced' | 'offline' | 'denied'): void
  (e: 'collaborators', users: { name: string; color: string }[]): void
}>()
const { t } = useI18n()
const config = useRuntimeConfig()
const host = ref<HTMLElement | null>(null)
const insertOpen = ref(false)
const loading = ref(true)
const instance = getCurrentInstance()!
const qc = useQueryClient()

const ydoc = new Y.Doc()
const yElements = ydoc.getMap<any>('elements')
const yFiles = ydoc.getMap<any>('files')
const collabUrl = (config.public.collabUrl as string) || `${socketBase()}/collab`
const provider = new HocuspocusProvider({ url: collabUrl, name: `canvas:${props.pageId}`, document: ydoc })
const LOCAL = 'looty-canvas-local'

let ex: any = null
let React: any = null
let root: any = null
let xapi: any = null
let alive = true
/** Dernière version connue de chaque élément (envoyée ou reçue) : pas d'écho. */
const versions = new Map<string, number>()
const isDark = () => !document.documentElement.classList.contains('light')

// --- Local → Yjs (regroupé par image) ---
let pending: readonly any[] | null = null
let pendingFiles: Record<string, any> = {}
let frame = 0
function onChange(elements: readonly any[], _state: any, files: Record<string, any>) {
  if (!props.canEdit) return
  pending = elements
  pendingFiles = files
  if (!frame) frame = requestAnimationFrame(flushLocal)
}
function flushLocal() {
  frame = 0
  const elements = pending
  pending = null
  if (!elements) return
  let changed = false
  ydoc.transact(() => {
    for (const el of elements) {
      if ((versions.get(el.id) ?? -1) >= el.version) continue
      versions.set(el.id, el.version)
      yElements.set(el.id, JSON.parse(JSON.stringify(el)))
      changed = true
    }
    for (const [id, f] of Object.entries(pendingFiles)) if (!yFiles.has(id)) yFiles.set(id, f)
  }, LOCAL)
  if (changed) savePreview()
}

// --- Yjs → Excalidraw ---
let remoteFrame = 0
function applyRemote() {
  remoteFrame = 0
  if (!xapi || !ex) return
  const remote = ex.restoreElements([...yElements.values()], null)
  for (const r of remote) versions.set(r.id, Math.max(versions.get(r.id) ?? -1, r.version))
  const merged = ex.reconcileElements(xapi.getSceneElementsIncludingDeleted(), remote, xapi.getAppState())
  xapi.updateScene({ elements: merged, captureUpdate: ex.CaptureUpdateAction.NEVER })
}
yElements.observe((_e, tr) => {
  if (tr.origin === LOCAL) return
  if (!remoteFrame) remoteFrame = requestAnimationFrame(applyRemote)
})
yFiles.observe((e, tr) => {
  if (tr.origin === LOCAL || !xapi) return
  const added = [...e.keysChanged].map((k) => yFiles.get(k)).filter(Boolean)
  if (added.length) xapi.addFiles(added)
})

// --- Curseurs ---
let pointerAt = 0
function onPointerUpdate(p: { pointer: { x: number; y: number; tool: string }; button: string }) {
  const now = Date.now()
  if (now - pointerAt < 40) return
  pointerAt = now
  provider.setAwarenessField('pointer', { x: p.pointer.x, y: p.pointer.y, tool: p.pointer.tool === 'laser' ? 'laser' : 'pointer' })
  provider.setAwarenessField('button', p.button)
}
provider.setAwarenessField('user', props.user)
provider.on('awarenessUpdate', ({ states }: { states: any[] }) => {
  const mine = provider.awareness?.clientID
  const people: { name: string; color: string }[] = []
  const collaborators = new Map<string, any>()
  provider.awareness?.getStates().forEach((s: any, id: number) => {
    if (!s.user) return
    if (id !== mine && !people.some((x) => x.name === s.user.name)) people.push(s.user)
    if (id === mine || !s.pointer) return
    collaborators.set(String(id), { username: s.user.name, pointer: s.pointer, button: s.button ?? 'up', color: { background: s.user.color, stroke: s.user.color } })
  })
  void states
  emit('collaborators', people)
  xapi?.updateScene({ collaborators })
})

// --- Aperçu (liste des canevas, cartes) ---
/** Titre d'une carte d'après les données déjà chargées (aperçu). */
function cardTitle(url: string): string {
  const r = parseLootyLink(url)
  if (!r) return url
  const data: any = r.kind === 'task' ? qc.getQueryData(taskKey(r.id))
    : r.kind === 'page' || r.kind === 'canvas' ? qc.getQueryData(['page', r.id])
    : r.kind === 'board' ? qc.getQueryData(boardKey(r.id))
    : r.kind === 'event' ? qc.getQueryData(['ws', r.ws, 'event', r.id])
    : qc.getQueryData(['project', r.id])
  const title = data?.task?.title ?? data?.title ?? data?.name ?? data?.project?.name
  return title || t(`canvas.kinds.${r.kind === 'canvas' ? 'page' : r.kind === 'task' ? 'tasks' : r.kind === 'event' ? 'events' : r.kind === 'project' ? 'projects' : r.kind === 'board' ? 'board' : 'page'}`)
}
/** Éléments pour l'aperçu : les cartes deviennent des rectangles blancs portant leur titre. */
function previewElements(elements: readonly any[]) {
  const cards = elements.filter((e) => e.type === 'embeddable' && e.link && parseLootyLink(e.link))
  const others = elements.filter((e) => !cards.includes(e))
  const boxes = ex.convertToExcalidrawElements(cards.map((c: any) => ({
    type: 'rectangle', x: c.x, y: c.y, width: c.width, height: c.height, angle: c.angle,
    backgroundColor: '#ffffff', strokeColor: '#d4d4dc', strokeWidth: 1, roughness: 0, roundness: { type: 3 },
    label: { text: cardTitle(c.link), fontSize: 18, fontFamily: 6, strokeColor: '#1b1b1f' },
  })))
  return [...others, ...boxes]
}
const savePreview = useDebounceFn(async () => {
  if (!xapi || !ex || !props.canEdit || !alive) return
  const elements = xapi.getSceneElements()
  let svg = ''
  if (elements.length) {
    const out = await ex.exportToSvg({ elements: previewElements(elements), appState: { exportBackground: false, exportWithDarkMode: false, viewBackgroundColor: '#ffffff' }, files: xapi.getFiles(), exportPadding: 24 })
    svg = out.outerHTML
    if (svg.length > 550_000) svg = ''
  }
  await http(`/api/workspaces/${props.wsId}/pages/${props.pageId}/canvas-preview`, { method: 'PUT', body: { svg }, silent: true }).catch(() => {})
}, 4000)

// --- Cartes Looty (Vue dans React) ---
function makeIsland() {
  return function LootyIsland(p: { url: string; editable: boolean }) {
    const el = React.useRef(null)
    React.useEffect(() => {
      const target = el.current as HTMLElement | null
      if (!target) return
      const vnode = h(CanvasCard, { url: p.url, editable: p.editable })
      vnode.appContext = instance.appContext
      render(vnode, target)
      return () => render(null, target)
    }, [p.url, p.editable])
    return React.createElement('div', { ref: el, className: 'cv-island' })
  }
}

/** Taille des cartes selon l'élément. */
const CARD = { task: [320, 112], page: [320, 132], canvas: [340, 250], board: [320, 170], event: [300, 104], project: [300, 96] } as const
function insertCards(urls: string[]) {
  if (!xapi || !ex) return
  const st = xapi.getAppState()
  const cx = st.width / 2 / st.zoom.value - st.scrollX
  const cy = st.height / 2 / st.zoom.value - st.scrollY
  const skeletons = urls.map((u, i) => {
    const [w, hgt] = CARD[parseLootyLink(u)?.kind ?? 'page']
    // Côte à côte, centrées sur l'écran.
    const total = urls.length * 340 - 20
    return { type: 'rectangle', x: cx - total / 2 + i * 340, y: cy - hgt / 2, width: w, height: hgt }
  })
  const rects = ex.convertToExcalidrawElements(skeletons)
  const cards = rects.map((r: any, i: number) => ({ ...r, type: 'embeddable', link: urls[i], strokeColor: 'transparent', backgroundColor: 'transparent', roundness: { type: 3 } }))
  xapi.updateScene({ elements: [...xapi.getSceneElementsIncludingDeleted(), ...cards], captureUpdate: ex.CaptureUpdateAction.IMMEDIATELY })
  xapi.setActiveTool({ type: 'selection' })
}

async function mount() {
  if (root || !alive || !host.value) return
  const [r, dom, mod] = await Promise.all([import('react'), import('react-dom/client'), import('@excalidraw/excalidraw')])
  await import('@excalidraw/excalidraw/index.css')
  if (!alive || !host.value) return
  React = r
  ex = mod
  const elements = ex.restoreElements([...yElements.values()], null)
  for (const e of elements) versions.set(e.id, e.version)
  Island = makeIsland()
  initial = {
    elements,
    files: Object.fromEntries(yFiles.entries()),
    // Style épuré par défaut (trait net, police Nunito), grille.
    appState: { currentItemRoughness: 0, currentItemFontFamily: 6, currentItemStrokeWidth: 1, currentItemRoundness: 'round', gridModeEnabled: true, viewBackgroundColor: 'transparent' },
    scrollToContent: true,
  }
  root = dom.createRoot(host.value)
  renderApp()
  loading.value = false
}

// Propriétés de l'éditeur (droit de modifier, thème) : nouveau rendu quand elles changent.
let Island: any = null
let initial: any = null
function renderApp() {
  if (!root) return
  root.render(React.createElement(ex.Excalidraw, {
    initialData: initial,
    theme: isDark() ? 'dark' : 'light',
    langCode: document.documentElement.lang === 'fr' ? 'fr-FR' : 'en',
    viewModeEnabled: !props.canEdit,
    isCollaborating: true,
    excalidrawAPI: (a: any) => { xapi = a },
    onChange,
    onPointerUpdate,
    validateEmbeddable: (link: string) => (parseLootyLink(link) ? true : undefined),
    renderEmbeddable: (el: any) => (el.link && parseLootyLink(el.link) ? React.createElement(Island, { url: el.link, editable: props.canEdit }) : null),
    renderTopRightUI: () => (props.canEdit
      ? React.createElement('button', { className: 'cv-insert', type: 'button', onClick: () => { insertOpen.value = true } }, t('canvas.insert'))
      : null),
    // Icône de lien d'une carte : ouverte dans Looty, pas dans un nouvel onglet.
    onLinkOpen: (el: any, ev: CustomEvent<{ nativeEvent: MouseEvent }>) => {
      const r = el.link ? parseLootyLink(el.link) : null
      if (!r) return
      ev.preventDefault()
      navigateTo(lootyPath(r))
    },
    UIOptions: { canvasActions: { loadScene: false, saveToActiveFile: false, export: { saveFileToDisk: true } } },
  }))
}
watch(() => props.canEdit, renderApp)

provider.on('synced', () => { emit('status', 'synced'); mount() })
provider.on('authenticationFailed', () => { emit('status', 'denied'); loading.value = false })
provider.on('status', ({ status }: { status: string }) => { if (status === 'disconnected') emit('status', 'offline') })
onMounted(() => emit('status', 'connecting'))
onBeforeUnmount(() => {
  alive = false
  if (frame) { cancelAnimationFrame(frame); flushLocal() }
  if (remoteFrame) cancelAnimationFrame(remoteFrame)
  root?.unmount()
  provider.destroy()
  ydoc.destroy()
})
</script>

<template>
  <div class="cv">
    <div ref="host" class="cv-host" />
    <div v-if="loading" class="cv-loading"><UiSpinner /></div>
    <CanvasInsertDialog v-model:open="insertOpen" @pick="insertCards" />
  </div>
</template>

<style scoped>
.cv { position: relative; flex: 1; min-height: 0; display: flex; }
.cv-host { flex: 1; min-width: 0; }
.cv-loading { position: absolute; inset: 0; display: grid; place-items: center; }
.cv :deep(.cv-island) { width: 100%; height: 100%; }
.cv :deep(.cv-insert) {
  height: 36px; padding: 0 14px; border-radius: 10px; border: 0; cursor: pointer;
  background: var(--accent); color: #fff; font: 600 13px var(--font-sans, Inter, system-ui, sans-serif);
  box-shadow: 0 1px 2px rgb(0 0 0 / .12);
}
.cv :deep(.cv-insert:hover) { filter: brightness(1.07); }
/* Les cartes portent leur propre fond : pas de cadre gris autour des éléments intégrés. */
.cv :deep(.excalidraw__embeddable-container) { border-radius: 14px; }
</style>
