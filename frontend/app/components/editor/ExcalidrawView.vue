<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import { PenTool, Maximize2, X } from 'lucide-vue-next'
import { motion } from '~/composables/useMotion'
import { useLayer } from '~/composables/useLayer'

/**
 * Dessin Excalidraw : aperçu SVG dans le document, édition en plein écran.
 * La scène (JSON) est stockée dans l'attribut du nœud, donc synchronisée par Yjs.
 * React + Excalidraw ne sont chargés qu'à la première ouverture.
 */
const props = defineProps(nodeViewProps)
const preview = ref('')
const editing = ref(false)
const host = ref<HTMLElement | null>(null)
let reactRoot: any = null
let api: any = null

const isDark = () => !document.documentElement.classList.contains('light')

function scene() {
  try { return JSON.parse(props.node.attrs.scene || '{}') } catch { return {} }
}

async function renderPreview() {
  const s = scene()
  if (!s.elements?.length) { preview.value = ''; return }
  const { exportToSvg } = await import('@excalidraw/excalidraw')
  const svg = await exportToSvg({
    elements: s.elements,
    appState: { ...(s.appState ?? {}), exportBackground: false, exportWithDarkMode: isDark() },
    files: s.files ?? {},
  })
  preview.value = svg.outerHTML
}

async function open() {
  editing.value = true
  await nextTick()
  const [{ createElement }, { createRoot }, ex] = await Promise.all([import('react'), import('react-dom/client'), import('@excalidraw/excalidraw')])
  await import('@excalidraw/excalidraw/index.css')
  const s = scene()
  reactRoot = createRoot(host.value!)
  reactRoot.render(createElement(ex.Excalidraw, {
    initialData: { elements: s.elements ?? [], appState: { ...(s.appState ?? {}), collaborators: new Map() }, files: s.files ?? {}, scrollToContent: true },
    theme: isDark() ? 'dark' : 'light',
    viewModeEnabled: !props.editor.isEditable,
    langCode: document.documentElement.lang === 'fr' ? 'fr-FR' : 'en',
    excalidrawAPI: (a: any) => { api = a },
    UIOptions: { canvasActions: { loadScene: false, export: { saveFileToDisk: true } } },
  }))
}

function close() {
  if (api && props.editor.isEditable) {
    const elements = api.getSceneElements().filter((e: any) => !e.isDeleted)
    const st = api.getAppState()
    const data = { elements, appState: { viewBackgroundColor: st.viewBackgroundColor, gridSize: st.gridSize }, files: api.getFiles() }
    const json = JSON.stringify(data)
    if (json !== props.node.attrs.scene) props.updateAttributes({ scene: json })
  }
  reactRoot?.unmount()
  reactRoot = null
  api = null
  editing.value = false
}
useLayer(editing, close, { modal: true })

watch(() => props.node.attrs.scene, renderPreview)
onMounted(renderPreview)
onBeforeUnmount(() => reactRoot?.unmount())
</script>

<template>
  <NodeViewWrapper class="exc" :class="{ selected }" data-drag-handle>
    <div class="exc-preview" contenteditable="false" @dblclick="open">
      <div v-if="preview" class="exc-svg" v-html="preview" />
      <div v-else class="exc-empty"><PenTool :size="20" /> {{ $t('embeds.drawingEmpty') }}</div>
      <button class="exc-open" @click="open"><Maximize2 :size="14" /> {{ editor.isEditable ? $t('embeds.editDrawing') : $t('embeds.openDrawing') }}</button>
    </div>
    <Teleport to="body">
      <Transition :css="false" @enter="motion.dialog.onEnter" @leave="motion.dialog.onLeave">
        <div v-if="editing" class="exc-full">
          <div class="exc-full-bar">
            <PenTool :size="15" /> <span>Excalidraw</span>
            <span class="spacer" />
            <UiButton size="sm" variant="primary" :icon="X" @click="close">{{ $t('embeds.closeDrawing') }}</UiButton>
          </div>
          <div ref="host" class="exc-host" />
        </div>
      </Transition>
    </Teleport>
  </NodeViewWrapper>
</template>

<style scoped>
.exc { margin: 1em 0; }
.exc-preview { position: relative; border-radius: var(--r-lg); background: var(--bg-column); box-shadow: var(--shadow-card); min-height: 140px; display: flex; align-items: center; justify-content: center; padding: 16px; cursor: zoom-in; }
.exc.selected .exc-preview { box-shadow: 0 0 0 1.5px var(--border-focus); }
.exc-svg { width: 100%; max-height: 520px; display: flex; justify-content: center; }
.exc-svg :deep(svg) { max-width: 100%; max-height: 520px; height: auto; }
.exc-empty { display: flex; align-items: center; gap: 8px; color: var(--text-3); }
.exc-open {
  position: absolute; top: 10px; right: 10px; display: inline-flex; align-items: center; gap: 6px; height: 28px; padding: 0 10px;
  border-radius: var(--r-sm); background: var(--bg-elevated); box-shadow: var(--shadow-md); font-size: var(--fs-sm); font-weight: 500;
  opacity: 0; transition: opacity var(--dur-fast);
}
.exc-preview:hover .exc-open, .exc.selected .exc-open { opacity: 1; }
.exc-full { position: fixed; inset: 0; z-index: 2500; display: flex; flex-direction: column; background: var(--bg); }
.exc-full-bar { display: flex; align-items: center; gap: 8px; height: 48px; padding: 0 12px 0 16px; border-bottom: 1px solid var(--border); font-weight: 600; padding-top: var(--safe-top); }
.exc-host { flex: 1; min-height: 0; }
@media (pointer: coarse) { .exc-open { opacity: 1; } }
</style>
