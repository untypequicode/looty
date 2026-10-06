<script setup lang="ts">
import { useEditorSpellcheck } from '~/composables/usePrefs'
import { useEditor, EditorContent } from '@tiptap/vue-3'
import type { Editor as TEditor } from '@tiptap/core'
import * as Y from 'yjs'
import { HocuspocusProvider } from '@hocuspocus/provider'
import { IndexeddbPersistence } from 'y-indexeddb'
import { isDesktop } from '~/composables/useDesktop'
import { useDebounceFn } from '@vueuse/core'
import { docExtensions } from '~/editor/extensions'
import { headingsKey, type HeadingInfo } from '~/editor/headings'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { sendMention, socketBase } from '~/composables/useRealtime'
import { uploadFile } from '~/composables/api'
import { motion } from '~/composables/useMotion'
import { resolveEmbed } from '~/editor/embeds'
import { toast } from '~/composables/useToast'
import { translate } from '~/composables/useI18n'

/**
 * Éditeur de document collaboratif (Yjs + Hocuspocus).
 * Le contenu de référence vit dans l'état Yjs ; une copie HTML est remontée
 * (debounce) pour la recherche, l'export et les liens publics.
 */
const props = defineProps<{
  pageId: string
  initialHtml?: string | null
  canEdit: boolean
  canComment?: boolean
  user: { name: string; color: string }
}>()
const emit = defineEmits<{
  (e: 'html', html: string): void
  (e: 'headings', h: HeadingInfo[]): void
  (e: 'collaborators', users: { name: string; color: string }[]): void
  (e: 'status', s: 'connecting' | 'synced' | 'offline' | 'denied'): void
  (e: 'ready', editor: TEditor): void
}>()

const { members, wsId, pages } = useWorkspace()
const config = useRuntimeConfig()
const router = useRouter()

const ydoc = new Y.Doc()
const collabUrl = (config.public.collabUrl as string) || `${socketBase()}/collab`
const provider = new HocuspocusProvider({ url: collabUrl, name: props.pageId, document: ydoc })
// Application de bureau : copie locale du document (édition hors ligne, fusion au retour).
const local = isDesktop ? new IndexeddbPersistence(`looty-page:${props.pageId}`, ydoc) : null
const synced = ref(false)
const serverSynced = ref(false)
/** Hors ligne, page jamais ouverte sur cet appareil : lecture seule du HTML enregistré. */
const offlineReadonly = ref(false)
const denied = ref(false)
const wrapper = ref<HTMLElement | null>(null)

// Sélecteur de document (lien interne inséré depuis « / »)
const docPickerOpen = ref(false)
const docPickerPos = ref<{ x: number; y: number } | null>(null)
const docPickerValue = ref<string | null>(null)
// Intégration par URL (YouTube, Figma...) depuis « / »
const embedOpen = ref(false)
const embedPos = ref<{ x: number; y: number } | null>(null)
const embedUrl = ref('')
function submitEmbed() {
  const url = embedUrl.value.trim()
  if (!url) return
  if (!resolveEmbed(url)) { toast.error(translate('embeds.unsupported')); return }
  editor.value?.chain().focus().insertEmbed(url).run()
  embedOpen.value = false
}

const docItems = computed(() => (pages.data.value ?? []).filter((p) => p.id !== props.pageId).map((p) => ({ value: p.id, label: `${p.icon ? `${p.icon} ` : ''}${p.title || '…'}` })))

const editor = useEditor({
  editable: false,
  extensions: docExtensions({
    ydoc,
    provider,
    user: props.user,
    getMembers: () => members.data.value ?? [],
    pages: { wsId: () => wsId.value, pages: () => (pages.data.value ?? []).filter((p) => p.id !== props.pageId) },
    onMention: (id) => sendMention(id, `page:${props.pageId}`),
    slash: {
      pickDoc: (e) => {
        const c = e.view.coordsAtPos(e.state.selection.from)
        docPickerPos.value = { x: c.left, y: c.bottom + 4 }
        docPickerValue.value = null
        docPickerOpen.value = true
      },
      askEmbed: (e) => {
        const c = e.view.coordsAtPos(e.state.selection.from)
        embedPos.value = { x: c.left, y: c.bottom + 4 }
        embedUrl.value = ''
        embedOpen.value = true
      },
    },
  }),
  editorProps: {
    attributes: { class: 'prose doc-prose', spellcheck: 'true' },
    handlePaste(view, event) {
      const files = [...(event.clipboardData?.files ?? [])].filter((f) => f.type.startsWith('image/'))
      if (!files.length) return false
      files.forEach((f) => insertImage(f))
      return true
    },
    handleDrop(view, event, _slice, moved) {
      if (moved) return false
      const files = [...(event.dataTransfer?.files ?? [])].filter((f) => f.type.startsWith('image/'))
      if (!files.length) return false
      const pos = view.posAtCoords({ left: event.clientX, top: event.clientY })?.pos
      files.forEach((f) => insertImage(f, pos))
      return true
    },
    handleClick(view, _pos, event) {
      const a = (event.target as HTMLElement).closest('a[href]') as HTMLAnchorElement | null
      if (!a) return false
      // En édition, un clic simple place le curseur ; Ctrl/⌘ + clic ouvre le lien.
      if (view.editable && !(event.metaKey || event.ctrlKey)) return false
      event.preventDefault()
      openHref(a.getAttribute('href')!)
      return true
    },
  },
  onUpdate: ({ editor: e, transaction }) => {
    emitHeadings(e)
    // Copie HTML : seulement pour les modifications faites ici (pas celles reçues d'un
    // collaborateur ni le chargement de la copie locale), sinon chacun la réenregistre.
    if (synced.value && props.canEdit && !fromYjs(transaction)) pushHtml()
  },
})
useEditorSpellcheck(editor)

/** Transaction appliquée par la synchronisation Yjs (clé « y-sync… », quelle que soit
 * la copie de y-prosemirror qui l'a produite). */
function fromYjs(tr: { meta?: Record<string, any> }): boolean {
  return Object.entries((tr as any).meta ?? {}).some(([k, v]) => k.startsWith('y-sync') && (v as any)?.isChangeOrigin)
}

function openHref(href: string) {
  if (href.startsWith('#')) return scrollToAnchor(href.slice(1))
  if (href.startsWith('/')) return router.push(href)
  window.open(href, '_blank', 'noopener')
}

async function insertImage(file: File, pos?: number) {
  const r = await uploadFile(file)
  const e = editor.value
  if (!e) return
  const node = e.schema.nodes.image.create({ src: r.url, alt: r.name })
  const tr = e.state.tr
  if (pos != null) tr.insert(pos, node)
  else tr.replaceSelectionWith(node)
  e.view.dispatch(tr)
}

function emitHeadings(e: TEditor) {
  emit('headings', headingsKey.getState(e.state)?.headings ?? [])
}
// Dernière copie HTML connue du serveur : inutile de renvoyer un contenu identique
// (ajustements automatiques au chargement, page rouverte hors ligne…).
let lastHtml = props.initialHtml ?? ''
const pushHtml = useDebounceFn(() => {
  if (!editor.value) return
  const html = editor.value.getHTML()
  if (html === lastHtml) return
  lastHtml = html
  emit('html', html)
}, 1500)

function scrollToAnchor(id: string) {
  const el = document.getElementById(decodeURIComponent(id))
  if (!el) return
  el.scrollIntoView({ block: 'start', behavior: 'smooth' })
  setTimeout(() => motion.flash(el), 350)
}

function insertDocLink(id: string | null) {
  docPickerOpen.value = false
  const p = pages.data.value?.find((x) => x.id === id)
  if (!p || !editor.value) return
  editor.value.chain().focus().insertContent([
    { type: 'text', text: `${p.icon ? `${p.icon} ` : ''}${p.title || '…'}`, marks: [{ type: 'link', attrs: { href: links.doc(wsId.value, p.id) } }] },
    { type: 'text', text: ' ' },
  ]).run()
}

function ready(e: TEditor) {
  synced.value = true
  e.setEditable(props.canEdit && !denied.value)
  emitHeadings(e)
  emit('ready', e)
  if (location.hash) nextTick(() => setTimeout(() => scrollToAnchor(location.hash.slice(1)), 120))
}

// Copie locale chargée : si elle a du contenu, on peut éditer sans attendre le serveur.
local?.on('synced', () => {
  const e = editor.value
  if (!e || synced.value) return
  if (ydoc.getXmlFragment('default').length > 0) {
    ready(e)
    if (!serverSynced.value) emit('status', 'offline')
    return
  }
  // Rien en local : on attend le serveur ; s'il ne répond pas, page neuve (éditable)
  // ou page jamais ouverte ici (lecture seule, pour ne pas dupliquer son contenu).
  setTimeout(() => {
    if (serverSynced.value || synced.value || !editor.value) return
    emit('status', 'offline')
    if (props.initialHtml && props.initialHtml !== '<p></p>') offlineReadonly.value = true
    else ready(editor.value)
  }, 3500)
})

provider.on('synced', () => {
  serverSynced.value = true
  offlineReadonly.value = false
  emit('status', 'synced')
  const e = editor.value
  if (!e) return
  // Migration : document Yjs vide mais contenu HTML existant (anciennes pages, duplication).
  if (e.isEmpty && props.initialHtml && props.initialHtml !== '<p></p>' && props.canEdit) {
    e.commands.setContent(props.initialHtml)
  }
  if (synced.value) {
    e.setEditable(props.canEdit && !denied.value)
    emitHeadings(e)
  } else ready(e)
})
provider.on('authenticationFailed', () => {
  denied.value = true
  emit('status', 'denied')
  // Lecture seule de secours à partir du HTML enregistré
  if (editor.value && props.initialHtml) {
    editor.value.commands.setContent(props.initialHtml)
    synced.value = true
  }
})
provider.on('status', ({ status }: { status: string }) => {
  if (status === 'disconnected' && synced.value) emit('status', 'offline')
  else if (status === 'connected' && serverSynced.value) emit('status', 'synced')
})
provider.on('awarenessUpdate', ({ states }: { states: { user?: { name: string; color: string } }[] }) => {
  const seen = new Set<string>()
  emit('collaborators', states.map((s) => s.user).filter((u): u is { name: string; color: string } => !!u && !seen.has(u.name) && !!seen.add(u.name)))
})

watch(() => props.canEdit, (v) => editor.value?.setEditable(v && synced.value && !denied.value))
onMounted(() => emit('status', 'connecting'))
onBeforeUnmount(() => {
  local?.destroy()
  provider.destroy()
  editor.value?.destroy()
  ydoc.destroy()
})

defineExpose({ editor, scrollToAnchor, getHTML: () => editor.value?.getHTML() ?? '' })
</script>

<template>
  <div ref="wrapper" class="doc-editor" :class="{ loading: !synced }">
    <div v-if="offlineReadonly" class="doc-offline">
      <p class="doc-offline-note">{{ $t('offline.readonly') }}</p>
      <DocViewer :html="initialHtml || ''" />
    </div>
    <div v-else-if="!synced" class="doc-loading">
      <div class="skeleton" style="height: 18px; width: 70%" />
      <div class="skeleton" style="height: 18px; width: 92%" />
      <div class="skeleton" style="height: 18px; width: 54%" />
    </div>
    <EditorContent v-show="synced && !offlineReadonly" :editor="editor" />
    <template v-if="editor && synced">
      <DocBlockHandle :editor="editor" :container="wrapper" />
      <DocBubble :editor="editor" :can-comment="canComment" />
    </template>
    <UiPopover v-model:open="embedOpen" :virtual-rect="embedPos" :width="360">
      <form class="embed-ask" @submit.prevent="submitEmbed">
        <input v-model="embedUrl" autofocus :placeholder="$t('embeds.urlPlaceholder')" />
        <UiButton size="sm" variant="primary" type="submit" :disabled="!embedUrl.trim()">{{ $t('embeds.insert') }}</UiButton>
      </form>
      <p class="embed-hint">{{ $t('embeds.supported') }}</p>
    </UiPopover>
    <UiCombobox v-model="docPickerValue" v-model:open="docPickerOpen" :items="docItems" :virtual-rect="docPickerPos" :placeholder="$t('editor.searchDocs')" :width="300" @update:model-value="insertDocLink" />
  </div>
</template>

<style scoped>
.doc-offline-note { margin: 0 0 16px; padding: 8px 12px; border-radius: var(--r-md); background: var(--bg-code); color: var(--text-2); font-size: var(--fs-sm); }
.doc-editor { position: relative; }
.embed-ask { display: flex; gap: 6px; padding: 8px; }
.embed-ask input { flex: 1; min-width: 0; height: 28px; padding: 0 8px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); outline: none; }
.embed-hint { margin: 0; padding: 0 10px 10px; font-size: var(--fs-xs); color: var(--text-3); }
.doc-loading { display: flex; flex-direction: column; gap: 14px; padding: 8px 0; }
.doc-editor :deep(.ProseMirror) { outline: none; min-height: 50vh; padding-bottom: 30vh; }
.doc-editor :deep(.ProseMirror .is-empty:not([data-placeholder=''])::before) {
  content: attr(data-placeholder);
  color: var(--text-4);
  float: left;
  height: 0;
  pointer-events: none;
}
.doc-editor :deep(.ProseMirror-selectednode:not(.img-node):not(.math-block)) { outline: 2px solid var(--border-focus); border-radius: var(--r-sm); }
.doc-editor :deep(.ProseMirror .selectedCell::after) { content: ''; position: absolute; inset: 0; background: var(--accent-soft); pointer-events: none; }
.doc-editor :deep(.column-resize-handle) { position: absolute; right: -2px; top: 0; bottom: 0; width: 4px; background: var(--accent); pointer-events: none; }
.doc-editor :deep(.tableWrapper) { overflow-x: auto; }
.doc-editor :deep(a) { cursor: text; }
.doc-editor :deep(.ProseMirror[contenteditable='false'] a) { cursor: pointer; }
</style>

<style>
/* Curseurs des collaborateurs */
.collaboration-cursor__caret { position: relative; margin: 0 -1px; border-left: 1.5px solid; border-right: none; pointer-events: none; word-break: normal; }
.collaboration-cursor__label {
  position: absolute; top: -1.45em; left: -1.5px; padding: 1px 6px; border-radius: 4px 4px 4px 0;
  font-size: 11px; font-weight: 600; line-height: 1.4; color: #fff; white-space: nowrap; user-select: none; font-style: normal;
}
</style>
