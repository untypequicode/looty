<script setup lang="ts">
import type { Editor as TEditor } from '@tiptap/core'
import { FileText, Star, Share2, MoreHorizontal, Link2, Copy, Download, FileArchive, Upload, Trash2, AppWindow, Search, Maximize2, Minimize2, Plus, Smile, Lock, CloudOff, Check, Loader2, FileQuestion, MessageSquare, History, X, BookOpenText, Printer } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { useDebounceFn, useLocalStorage } from '@vueuse/core'
import type { Page } from '~/types'
import type { MenuItem } from '~/components/ui/Menu.vue'
import type { HeadingInfo } from '~/editor/headings'
import { api } from '~/composables/api'
import { useAccess } from '~/composables/useAccess'
import { useWorkspace, links, wsKeys } from '~/composables/useWorkspace'
import { usePages, pagePath } from '~/composables/usePages'
import { useAuth } from '~/composables/useAuth'
import { useShortcuts } from '~/composables/useShortcuts'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { motion } from '~/composables/useMotion'
import { exportMarkdown } from '~/editor/export'
import { isDesktop, openInDesktop } from '~/composables/useDesktop'
import { useComments, type ItemComment } from '~/composables/useComments'
import { replaceCommentMark, findCommentMark } from '~/editor/commentMark'

definePageMeta({ layout: 'workspace' })

const route = useRoute()
const qc = useQueryClient()
const { t, relativeTime } = useI18n()
const { wsId, pages, member, isGuest } = useWorkspace()
const { editing, allows } = useAccess(wsId)
const { user } = useAuth()
const { updatePage, toggleFavorite, deletePage, duplicatePage, createPage } = usePages()

const pageId = computed(() => route.params.page as string)
const { data: page, isLoading, error } = useQuery({
  queryKey: computed(() => ['page', pageId.value]),
  queryFn: () => api<Page>(`/api/workspaces/${wsId.value}/pages/${pageId.value}`, { silent: true }),
  enabled: computed(() => !!pageId.value && !!wsId.value),
  staleTime: 0,
})
// Les métadonnées temps réel (titre, icône...) proviennent aussi de la liste des pages.
const live = computed<Page | undefined>(() => {
  const fromList = pages.data.value?.find((p) => p.id === pageId.value)
  return page.value ? { ...page.value, ...(fromList ?? {}), content: page.value.content } : undefined
})
// Droit sur la page (rôle, permissions de page) + mode d'édition personnel des documents.
// Invité : seul le partage de la page compte (pas de mode d'édition d'espace).
const canEdit = computed(() => live.value?.can_edit !== false && editing('docs'))
/** Créer, dupliquer, importer, supprimer : réservé aux membres (un invité modifie seulement). */
const canManage = computed(() => canEdit.value && !isGuest.value)
// Sous-document : rôle qui crée des documents, ou partage « peut créer des sous-pages ».
const canAddChild = computed(() => (canManage.value && allows('docs', 'edit')) || (!!live.value?.can_add_child && editing('docs')))
const path = computed(() => pagePath(pages.data.value ?? [], pageId.value))
const children = computed(() => (pages.data.value ?? []).filter((p) => p.parent_id === pageId.value).sort((a, b) => a.position - b.position))

// Enregistre la consultation
watch(pageId, (id) => { if (id) api(`/api/workspaces/${wsId.value}/pages/${id}/view`, { method: 'POST', silent: true }).catch(() => {}) }, { immediate: true })

// --- Titre & icône ---
const title = ref('')
const titleEl = ref<HTMLTextAreaElement | null>(null)
watch(() => live.value?.title, (v) => { if (v !== undefined && document.activeElement !== titleEl.value) title.value = v; nextTick(autosize) }, { immediate: true })
function autosize() {
  const el = titleEl.value
  if (!el) return
  el.style.height = 'auto'
  // Mesure faite avant la mise en page (données déjà en cache) : on réessaie plus tard
  // plutôt que de figer une hauteur nulle.
  if (!el.scrollHeight) return void requestAnimationFrame(() => el.isConnected && el.scrollHeight && autosize())
  el.style.height = `${el.scrollHeight}px`
}
watch(titleEl, (el) => { if (el) nextTick(autosize) })
const saveTitle = useDebounceFn(() => {
  if (live.value && title.value !== live.value.title) updatePage(pageId.value, { title: title.value })
}, 500)
function onTitleKey(e: KeyboardEvent) {
  if (e.key === 'Enter' || (e.key === 'ArrowDown' && (e.target as HTMLTextAreaElement).selectionStart === title.value.length)) {
    e.preventDefault()
    editorInst.value?.commands.focus('start')
  }
}
const iconOpen = ref(false)
function setIcon(e: string | null) { updatePage(pageId.value, { icon: e }) }

// --- Éditeur ---
const editorInst = shallowRef<TEditor | null>(null)
const headings = ref<HeadingInfo[]>([])
const collaborators = ref<{ name: string; color: string }[]>([])
const status = ref<'connecting' | 'synced' | 'offline' | 'denied'>('connecting')
const saving = ref(false)
const PALETTE = ['#7170ff', '#4cb782', '#f2994a', '#eb5757', '#4ea7fc', '#b18cff', '#e06ba8', '#26b5ce']
const cursorUser = computed(() => {
  const name = user.value?.name ?? '?'
  let h = 0
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return { name, color: PALETTE[h % PALETTE.length] }
})
async function saveHtml(html: string, id: string = pageId.value) {
  // Identifiant fourni par l'éditeur : un éditeur qui se ferme après un changement de page
  // enregistre bien sa propre page.
  saving.value = true
  try {
    await api(`/api/workspaces/${wsId.value}/pages/${id}`, { method: 'PUT', body: { content: html }, silent: true })
    qc.setQueryData<Page>(['page', id], (old) => (old ? { ...old, content: html } : old))
  } finally {
    saving.value = false
  }
}

// --- Commentaires ---
const commentsOpen = useLocalStorage<boolean>('looty.doc.comments', false)
const pendingQuote = ref<string | null>(null)
const focusThread = ref<string | null>(null)
const { threads: commentThreads, openCount: openComments } = useComments(wsId, ref('page'), pageId)
const canComment = computed(() => !!live.value && allows('docs', 'comment'))
function onCommentSelection(e: Event) {
  pendingQuote.value = (e as CustomEvent).detail.quote
  commentsOpen.value = true
}
function onCommentOpen(e: Event) {
  focusThread.value = (e as CustomEvent).detail.id
  commentsOpen.value = true
}
onMounted(() => {
  window.addEventListener('looty:comment-selection', onCommentSelection)
  window.addEventListener('looty:comment-open', onCommentOpen)
})
onBeforeUnmount(() => {
  window.removeEventListener('looty:comment-selection', onCommentSelection)
  window.removeEventListener('looty:comment-open', onCommentOpen)
})
// Lien vers un commentaire (notification) : ouvrir la discussion sur ce fil.
watch(() => route.query.comment as string | undefined, (id) => { if (id) { commentsOpen.value = true; focusThread.value = id } }, { immediate: true })
// Passage cité abandonné : retirer la marque provisoire.
watch(pendingQuote, (q) => { if (!q && editorInst.value && canEdit.value) replaceCommentMark(editorInst.value, 'pending', null) })
watch(commentsOpen, (o) => { if (!o) pendingQuote.value = null })
function onCommentCreated(c: ItemComment) {
  if (c.quote && editorInst.value && canEdit.value) replaceCommentMark(editorInst.value, 'pending', c.id)
}
function onCommentRemoved(c: ItemComment) {
  if (editorInst.value && canEdit.value) replaceCommentMark(editorInst.value, c.id, null)
}
function onCommentSelect(c: ItemComment) {
  focusThread.value = c.id
  const e = editorInst.value
  const pos = e ? findCommentMark(e, c.id) : null
  if (e && pos !== null) {
    e.commands.setTextSelection(pos)
    const el = e.view.domAtPos(pos).node as HTMLElement
    ;(el.nodeType === 1 ? el : el.parentElement)?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }
}
/** Surlignage des passages des fils ouverts (les fils résolus restent discrets). */
const commentCss = computed(() => commentThreads.value
  .filter((th) => !th.root.resolved_at)
  .map((th) => `.doc-prose .comment-mark[data-comment-id="${th.root.id}"]`)
  .join(',') + '{background:color-mix(in srgb,var(--yellow,#e2a03f) 22%,transparent);border-bottom:2px solid color-mix(in srgb,var(--yellow,#e2a03f) 70%,transparent);cursor:pointer}'
  + (focusThread.value ? `.doc-prose .comment-mark[data-comment-id="${focusThread.value}"]{background:color-mix(in srgb,var(--yellow,#e2a03f) 45%,transparent)}` : ''))

// --- Historique ---
const historyOpen = ref(false)
// ?history=1 (lien de l’application Android) : ouvre l’historique des versions.
watch(() => route.query.history, (h) => { if (h) historyOpen.value = true }, { immediate: true })

// --- Interface ---
const fullWidth = useLocalStorage<boolean>('looty.doc.fullWidth', false)
const findOpen = ref(false)
const shareOpen = ref(false)
const exportOpen = ref(false)
const importOpen = ref(false)
const menuOpen = ref(false)
const scroller = ref<HTMLElement | null>(null)

const menu = computed<MenuItem[]>(() => {
  const p = live.value
  if (!p) return []
  return [
    { label: t('common.copyLink'), icon: Link2, onSelect: () => { navigator.clipboard.writeText(location.origin + links.doc(wsId.value, p.id)); toast.success(t('common.linkCopied')) } },
    ...(isDesktop ? [] : [{ label: t('desktop.openInApp'), icon: AppWindow, onSelect: () => openInDesktop(links.doc(wsId.value, p.id)) }]),
    { label: t('reader.open'), icon: BookOpenText, shortcut: ['mod', 'alt', 'R'], onSelect: () => navigateTo(links.read(wsId.value, p.id)) },
    { label: t('history.title'), icon: History, onSelect: () => (historyOpen.value = true) },
    { label: t('editor.findInPage'), icon: Search, shortcut: ['mod', 'F'], onSelect: () => (findOpen.value = true) },
    { label: fullWidth.value ? t('editor.normalWidth') : t('editor.fullWidthPage'), icon: fullWidth.value ? Minimize2 : Maximize2, onSelect: () => (fullWidth.value = !fullWidth.value) },
    { divider: true },
    ...(canManage.value ? [{ label: t('common.duplicate'), icon: Copy, onSelect: () => duplicatePage(p.id) }] : []),
    { label: t('editor.exportMarkdown'), icon: Download, onSelect: () => exportMarkdown(title.value, editorInst.value?.getHTML() ?? p.content ?? '') },
    { label: t('exportFormats.printPdf'), icon: Printer, onSelect: async () => { const { printToPdf } = await import('~/editor/exportDoc'); printToPdf(title.value, [{ title: title.value || t('docs.untitled'), html: editorInst.value?.getHTML() ?? p.content ?? '' }]) } },
    { label: t('transfer.exportMenu'), icon: FileArchive, onSelect: () => (exportOpen.value = true) },
    ...(canManage.value ? [{ label: t('transfer.importHere'), icon: Upload, onSelect: () => (importOpen.value = true) }] : []),
    ...(canManage.value ? [{ divider: true }, { label: t('common.delete'), icon: Trash2, danger: true, onSelect: async () => { if (await deletePage(p)) navigateTo(p.parent_id ? links.doc(wsId.value, p.parent_id) : links.docs(wsId.value)) } }] : []),
  ]
})

useShortcuts([
  { keys: 'mod+f', global: true, handler: () => { findOpen.value = true } },
  { keys: 'mod+shift+l', global: true, handler: () => { fullWidth.value = !fullWidth.value } },
  { keys: 'mod+alt+r', global: true, handler: () => { navigateTo(links.read(wsId.value, pageId.value)) } },
])

async function newChild() {
  const p = await createPage({ parentId: pageId.value })
  navigateTo(links.doc(wsId.value, p.id))
}

// Animation d'entrée du document
const head = ref<HTMLElement | null>(null)
watch([live, head], ([p, h], [old]) => { if (p && h && p.id !== old?.id) nextTick(() => motion.stagger(h.children, { y: 8, each: 0.05 })) })

useHead({ title: () => live.value ? `${live.value.icon ? `${live.value.icon} ` : ''}${live.value.title || t('docs.untitled')}` : t('nav.docs') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('nav.docs'), icon: FileText, to: links.docs(wsId) }, ...path.map((p) => ({ label: p.title || t('docs.untitled'), emoji: p.icon, to: p.id === pageId ? undefined : links.doc(wsId, p.id) }))]">
      <span v-if="live" class="status">
        <template v-if="status === 'denied' || !canEdit"><Lock :size="13" /> {{ t('editor.readOnly') }}</template>
        <template v-else-if="status === 'offline'"><CloudOff :size="13" /> {{ t('editor.offline') }}</template>
        <template v-else-if="status === 'connecting' || saving"><Loader2 :size="13" class="spin" /> {{ status === 'connecting' ? t('editor.connecting') : t('editor.saving') }}</template>
        <template v-else><Check :size="13" /> {{ t('common.saved') }}</template>
      </span>
      <span class="collabs">
        <UiTooltip v-for="c in collaborators.filter((c) => c.name !== cursorUser.name).slice(0, 5)" :key="c.name" :text="c.name">
          <span class="collab" :style="{ background: c.color }">{{ c.name[0]?.toUpperCase() }}</span>
        </UiTooltip>
      </span>
      <template v-if="live">
        <UiTooltip v-if="!isGuest" :text="live.is_favorite ? t('docs.unfavorite') : t('docs.favorite')">
          <UiButton size="sm" variant="ghost" :icon="Star" :class="{ fav: live.is_favorite }" @click="toggleFavorite(live)" />
        </UiTooltip>
        <AppEditToggle v-if="live.can_edit !== false" area="docs" />
        <UiTooltip :text="t('reader.open')" :shortcut="['mod', 'alt', 'R']">
          <UiButton size="sm" variant="ghost" :icon="BookOpenText" :to="links.read(wsId, live.id)" :aria-label="t('reader.open')" />
        </UiTooltip>
        <UiTooltip :text="t('itemComments.title')">
          <UiButton size="sm" variant="ghost" :icon="MessageSquare" :class="{ 'cm-on': commentsOpen }" :aria-pressed="commentsOpen" @click="commentsOpen = !commentsOpen">{{ openComments || '' }}</UiButton>
        </UiTooltip>
        <UiButton v-if="!isGuest" size="sm" :icon="Share2" @click="shareOpen = true">{{ t('share.share') }}</UiButton>
        <UiMenu v-model:open="menuOpen" :items="menu" placement="bottom-end" :width="250"><UiButton size="sm" variant="ghost" :icon="MoreHorizontal" /></UiMenu>
      </template>
    </AppPageHeader>

    <UiEmpty v-if="error && !isLoading" :icon="FileQuestion" :title="t('docs.notFound')" :description="t('docs.notFoundHint')">
      <UiButton :to="links.docs(wsId)">{{ t('nav.allDocs') }}</UiButton>
    </UiEmpty>
    <div v-else class="doc-body">
    <component :is="'style'">{{ commentCss }}</component>
    <div ref="scroller" class="doc-scroll">
      <DocFindBar v-model:open="findOpen" :editor="editorInst ?? undefined" />
      <div class="doc-layout" :class="{ wide: fullWidth }">
        <article class="doc-col">
          <div v-if="isLoading || !live" class="doc-skel"><div class="skeleton" style="height: 44px; width: 60%" /><div class="skeleton" style="height: 18px; width: 30%; margin-top: 16px" /></div>
          <template v-else>
            <header ref="head" class="doc-head">
              <div class="icon-row">
                <DocIconPicker v-if="canEdit" v-model:open="iconOpen" @select="setIcon">
                  <button v-if="live.icon" class="doc-icon">{{ live.icon }}</button>
                  <button v-else class="add-icon"><Smile :size="14" /> {{ t('editor.addIcon') }}</button>
                </DocIconPicker>
                <span v-else-if="live.icon" class="doc-icon static">{{ live.icon }}</span>
              </div>
              <textarea
                ref="titleEl"
                v-model="title"
                class="doc-title"
                rows="1"
                :readonly="!canEdit"
                :placeholder="t('docs.untitled')"
                @input="autosize(); saveTitle()"
                @keydown="onTitleKey"
              />
              <div class="doc-meta">
                <UiAvatar v-if="live.updated_by" :name="member(live.updated_by)?.name" :src="member(live.updated_by)?.avatar_url" :size="18" />
                <span>{{ t('editor.lastEdited', { name: member(live.updated_by ?? live.created_by)?.name ?? '?', time: relativeTime(live.updated_at) }) }}</span>
              </div>
            </header>
            <DocEditor
              :key="pageId"
              :page-id="pageId"
              :initial-html="page?.content"
              :can-edit="canEdit"
              :can-migrate="live.can_edit !== false"
              :can-comment="canComment"
              :user="cursorUser"
              @ready="(e) => (editorInst = e)"
              @html="saveHtml"
              @headings="(h) => (headings = h)"
              @collaborators="(c) => (collaborators = c)"
              @status="(s) => (status = s)"
            />

            <section v-if="children.length || canAddChild" class="children">
              <h3>{{ t('editor.subDocuments') }}</h3>
              <div class="child-grid">
                <NuxtLink v-for="c in children" :key="c.id" :to="links.doc(wsId, c.id)" class="child">
                  <span class="child-icon">{{ c.icon || '📄' }}</span>
                  <span class="child-body">
                    <span class="child-title truncate">{{ c.title || t('docs.untitled') }}</span>
                    <span class="child-meta">{{ relativeTime(c.updated_at) }}</span>
                  </span>
                </NuxtLink>
                <button v-if="canAddChild" class="child new" @click="newChild"><Plus :size="16" /> {{ t('docs.newChild') }}</button>
              </div>
            </section>
            <GraphRelatedPanel v-if="!isGuest" kind="page" :id="pageId" class="doc-related" />
          </template>
        </article>
        <aside class="toc-col"><DocToc :headings="headings" :scroller="scroller" /></aside>
      </div>
    </div>
    <Transition name="side">
      <aside v-if="commentsOpen && live" class="side-panel" :aria-label="t('itemComments.title')">
        <header class="side-head">
          <MessageSquare :size="14" />
          <span>{{ t('itemComments.title') }}</span>
          <span class="spacer" />
          <UiButton size="xs" variant="ghost" :icon="X" :aria-label="t('common.close')" @click="commentsOpen = false" />
        </header>
        <CommentsPanel
          v-model:quote="pendingQuote"
          kind="page"
          :target-id="pageId"
          :can-comment="canComment"
          :focus-id="focusThread"
          @created="onCommentCreated"
          @removed="onCommentRemoved"
          @select="onCommentSelect"
        />
      </aside>
    </Transition>
    </div>

    <DocShareDialog v-if="live" v-model:open="shareOpen" :page="live" />
    <DocExportDialog v-if="live" v-model:open="exportOpen" :page="{ ...live, title }" :get-html="() => editorInst?.getHTML() ?? ''" />
    <DocImportDialog v-if="live" v-model:open="importOpen" :parent="live" />
    <DocHistoryDialog v-if="live" v-model:open="historyOpen" :page-id="pageId" :editor="editorInst as any" :can-edit="canEdit" :current-title="title" @restore-title="(v) => { title = v; updatePage(pageId, { title: v }) }" />
  </div>
</template>

<style scoped>
.doc-related { margin-top: 32px; padding-top: 16px; border-top: 1px solid var(--border); }
.status { display: inline-flex; align-items: center; gap: 5px; font-size: var(--fs-sm); color: var(--text-3); margin-right: 6px; white-space: nowrap; }
.spin { animation: spin 1s linear infinite; }
.collabs { display: flex; margin-right: 4px; }
.collab { width: 22px; height: 22px; border-radius: 50%; display: grid; place-items: center; color: #fff; font-size: 10px; font-weight: 700; box-shadow: 0 0 0 2px var(--bg); margin-left: -4px; }
.fav :deep(svg) { fill: var(--yellow); color: var(--yellow); }
.doc-body { flex: 1; min-height: 0; display: flex; }
.doc-scroll { position: relative; flex: 1; min-width: 0; overflow-y: auto; scroll-behavior: smooth; }
.side-panel { width: 360px; flex-shrink: 0; display: flex; flex-direction: column; padding: 10px 14px 14px; border-left: 1px solid var(--border); background: var(--bg-panel); min-height: 0; }
.side-head { display: flex; align-items: center; gap: 8px; height: 30px; margin-bottom: 6px; font-weight: 600; }
.side-enter-active, .side-leave-active { transition: margin-right var(--dur) var(--ease-out), opacity var(--dur); }
.side-enter-from, .side-leave-to { margin-right: -360px; opacity: 0; }
.cm-on { background: var(--bg-active); }
@media (max-width: 900px) { .side-panel { position: fixed; z-index: 50; inset: 48px 0 0 auto; width: min(380px, 100vw); box-shadow: var(--shadow-lg); } }
.doc-layout { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 760px) minmax(0, 1fr); padding: 48px 24px 0; }
.doc-layout.wide { grid-template-columns: 0 minmax(0, 1fr) 240px; padding: 48px 80px 0; }
.doc-col { grid-column: 2; min-width: 0; }
.toc-col { grid-column: 3; padding-left: 40px; }
.toc-col > * { position: sticky; top: 24px; max-width: 240px; }
.doc-skel { padding-top: 40px; }
.doc-head { margin-bottom: 20px; }
.icon-row { min-height: 28px; margin-bottom: 6px; display: flex; }
.doc-icon { font-size: 52px; line-height: 1; padding: 4px; border-radius: var(--r-md); transition: background var(--dur-fast); }
.doc-icon:not(.static):hover { background: var(--bg-hover); }
.add-icon { display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 8px; border-radius: var(--r-sm); color: var(--text-3); font-size: var(--fs-sm); opacity: 0; transition: opacity var(--dur-fast), background var(--dur-fast); }
.doc-head:hover .add-icon, .add-icon:focus { opacity: 1; }
.add-icon:hover { background: var(--bg-hover); color: var(--text); }
.doc-title {
  width: 100%;
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  overflow: hidden;
  padding: 0;
  font-family: var(--font-display);
  font-size: 38px;
  font-weight: 700;
  letter-spacing: -0.028em;
  line-height: 1.2;
  color: var(--text);
}
.doc-title::placeholder { color: var(--text-4); }
.doc-meta { display: flex; align-items: center; gap: 8px; margin-top: 12px; color: var(--text-3); font-size: var(--fs-sm); }
.children { margin: 8px 0 80px; padding-top: 24px; border-top: 1px solid var(--border); }
.children h3 { margin: 0 0 12px; font-size: var(--fs-sm); color: var(--text-3); font-weight: 600; text-transform: uppercase; letter-spacing: 0.04em; }
.child-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 8px; }
.child { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: var(--r-md); background: var(--bg-panel); box-shadow: var(--shadow-card); transition: background var(--dur-fast), transform var(--dur) var(--ease-out); text-align: left; }
.child:hover { background: var(--bg-card-hover); transform: translateY(-1px); }
.child-icon { font-size: 18px; }
.child-body { display: flex; flex-direction: column; min-width: 0; }
.child-title { font-weight: 500; }
.child-meta { font-size: var(--fs-xs); color: var(--text-3); }
.child.new { color: var(--text-3); box-shadow: inset 0 0 0 1px var(--border-strong); background: transparent; justify-content: center; }
.child.new:hover { color: var(--text); }
:deep(.doc-prose) { font-size: 16px; }
@media (max-width: 1279px) {
  .doc-layout, .doc-layout.wide { grid-template-columns: minmax(0, 1fr); padding: 40px 72px 0; }
  .doc-col { grid-column: 1; max-width: 760px; margin: 0 auto; width: 100%; }
  .doc-layout.wide .doc-col { max-width: none; }
  .toc-col { display: none; }
}
@media (max-width: 639px) {
  .doc-layout, .doc-layout.wide { padding: 24px 18px 0; }
  .doc-title { font-size: 28px; }
  .doc-icon { font-size: 40px; }
  .add-icon { opacity: 1; }
  .status, .collabs { display: none; }
}
</style>
