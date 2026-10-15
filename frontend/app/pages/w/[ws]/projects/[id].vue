<script setup lang="ts">
import {
  FolderCode, FilePlus, FolderPlus, Upload, Download, Globe, RefreshCw, ExternalLink, Play, Settings2, Monitor, Tablet, Smartphone,
  PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen, AlertCircle, AlertTriangle, CheckCircle2, Pencil, Trash2, Star,
  FileText, Laptop, Server, ScrollText, Copy, Lock, Users,
} from 'lucide-vue-next'
import { useLocalStorage } from '@vueuse/core'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useAuth } from '~/composables/useAuth'
import { useWorkspace } from '~/composables/useWorkspace'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { confirmDialog, promptDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { isDesktop } from '~/composables/useDesktop'
import {
  useProject, useProjectActions, buildFileTree, parseLatexLog, fileSize,
  type TreeNode, type CompileResult, type LatexIssue,
} from '~/composables/useProjects'

definePageMeta({ layout: 'workspace' })

/**
 * Projet (mini-site ou LaTeX) : fichiers à gauche, éditeur partagé au centre, aperçu du
 * site ou PDF compilé à droite.
 */
const { t, relativeTime } = useI18n()
const { user } = useAuth()
const { wsId } = useWorkspace()
const { isMobile } = useBreakpoint()
const route = useRoute()
const router = useRouter()
const id = computed(() => route.params.id as string)
const { data, error, isLoading } = useProject(id)
const actions = useProjectActions(id)
const project = computed(() => data.value?.project)
const files = computed(() => data.value?.files ?? [])
const tree = computed(() => buildFileTree(files.value))
const canWrite = computed(() => !!data.value?.can_write)
const isSite = computed(() => project.value?.kind === 'site')
const isNotebook = computed(() => project.value?.kind === 'notebook')
const collapsed = ref<string[]>([])

// --- Fichier ouvert ---
const fileId = computed<string | null>(() => (typeof route.query.file === 'string' ? route.query.file : null))
const current = computed(() => files.value.find((f) => f.id === fileId.value && !f.is_dir) ?? null)
const tabs = ref<string[]>([])
watch(data, (d) => {
  if (!d) return
  // Premier affichage : fichier principal (LaTeX) ou page d'entrée (site).
  if (!fileId.value) {
    const want = d.project.kind === 'latex' ? d.project.settings.main ?? 'main.tex' : d.project.kind === 'notebook' ? d.project.settings.main ?? 'notebook.ipynb' : d.project.settings.entry ?? 'index.html'
    const f = d.files.find((x) => x.path === want) ?? d.files.find((x) => !x.is_dir && x.text)
    if (f) open(f.id, true)
  }
  tabs.value = tabs.value.filter((tid) => d.files.some((f) => f.id === tid))
}, { immediate: true })
function open(fid: string, replace = false) {
  if (!tabs.value.includes(fid)) tabs.value = [...tabs.value, fid].slice(-8)
  const q = { ...route.query, file: fid }
  if (replace) router.replace({ query: q })
  else router.push({ query: q })
  if (isMobile.value) mobilePane.value = 'code'
}
function closeTab(fid: string) {
  tabs.value = tabs.value.filter((x) => x !== fid)
  if (fid === fileId.value) {
    const next = tabs.value[tabs.value.length - 1]
    router.replace({ query: { ...route.query, file: next } })
  }
}
const tabFiles = computed(() => tabs.value.map((tid) => files.value.find((f) => f.id === tid)).filter((f): f is NonNullable<typeof f> => !!f))
const editorRef = ref<{ getText: () => string; goToLine: (n: number) => void } | null>(null)
const status = ref<'connecting' | 'synced' | 'offline' | 'denied'>('connecting')
const collaborators = ref<{ name: string; color: string }[]>([])
const userColor = computed(() => {
  const palette = ['#e2a03f', '#4c9ad4', '#d4544c', '#56a85c', '#9c5cd4', '#d45c9c', '#3fb3a8']
  const s = user.value?.id ?? ''
  return palette[[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % palette.length]!
})
const cursorUser = computed(() => ({ name: user.value?.name ?? '?', color: userColor.value }))
/** Fichier .ipynb ouvert : éditeur de notebook (cellules, exécution Python). */
const isIpynb = computed(() => !!current.value?.text && /\.ipynb$/i.test(current.value.path))
const imagePreview = computed(() => current.value && !current.value.text && /^image\//.test(current.value.mime ?? '') ? `/api/projects/${id.value}/files/${current.value.id}/raw` : null)

// --- Actions sur les fichiers ---
const folderOf = (n: TreeNode | null) => (!n ? '' : n.dir ? n.path : n.path.includes('/') ? n.path.slice(0, n.path.lastIndexOf('/')) : '')
async function newFile(folder = '') {
  const name = await promptDialog({ title: t('projects.newFile'), placeholder: isSite.value ? 'page.html' : 'chapitre.tex', confirmLabel: t('common.create') })
  if (!name) return
  try { const r = await actions.createFile(folder ? `${folder}/${name}` : name); open(r.id) } catch { /* toast par api() */ }
}
async function newFolder(folder = '') {
  const name = await promptDialog({ title: t('projects.newFolder'), placeholder: isSite.value ? 'images' : 'chapitres', confirmLabel: t('common.create') })
  if (!name) return
  try { await actions.createFile(folder ? `${folder}/${name}` : name, true) } catch { /* toast */ }
}
const uploadInput = ref<HTMLInputElement | null>(null)
const uploadFolder = ref('')
function pickUpload(folder = '') { uploadFolder.value = folder; uploadInput.value?.click() }
async function doUpload(list: FileList | File[], folder = uploadFolder.value) {
  if (!list.length) return
  try {
    const r = await actions.upload(list, folder)
    toast.success(t('projects.uploaded', { count: r.length }))
  } catch { /* toast */ } finally { if (uploadInput.value) uploadInput.value.value = '' }
}
async function rename(n: TreeNode) {
  const next = await promptDialog({ title: t('projects.rename'), initial: n.name, confirmLabel: t('common.save') })
  if (!next || next === n.name || !n.file) return
  const parent = n.path.includes('/') ? n.path.slice(0, n.path.lastIndexOf('/')) : ''
  await actions.movePath(n.file.id, parent ? `${parent}/${next}` : next).catch(() => {})
}
async function moveTo(from: string, folder: string) {
  const f = files.value.find((x) => x.path === from)
  if (!f) return
  const name = from.split('/').pop()!
  await actions.movePath(f.id, folder ? `${folder}/${name}` : name).catch(() => {})
}
async function remove(n: TreeNode) {
  if (!n.file) return
  if (!(await confirmDialog({ title: t('projects.deleteTitle', { name: n.name }), message: n.dir ? t('projects.deleteFolderHint') : t('projects.deleteHint'), danger: true, confirmLabel: t('common.delete') }))) return
  await actions.remove(n.file.id)
  if (n.file.id === fileId.value || n.dir) closeTab(n.file.id)
}
async function setMain(n: TreeNode) {
  await actions.update({ settings: isSite.value ? { entry: n.path } : { main: n.path } })
  toast.success(isSite.value ? t('projects.entrySet', { path: n.path }) : t('projects.mainSet', { path: n.path }))
}
function nodeMenu(n: TreeNode): MenuItem[] {
  const items: MenuItem[] = []
  if (n.dir && canWrite.value) {
    items.push({ label: t('projects.newFile'), icon: FilePlus, onSelect: () => newFile(n.path) }, { label: t('projects.newFolder'), icon: FolderPlus, onSelect: () => newFolder(n.path) }, { label: t('projects.upload'), icon: Upload, onSelect: () => pickUpload(n.path) }, { divider: true })
  }
  if (!n.dir && n.file) {
    if (canWrite.value && !isNotebook.value && (isSite.value ? /\.html?$/i.test(n.path) : /\.tex$/i.test(n.path))) items.push({ label: isSite.value ? t('projects.setEntry') : t('projects.setMain'), icon: Star, onSelect: () => setMain(n) })
    items.push({ label: t('projects.download'), icon: Download, onSelect: () => window.open(`/api/projects/${id.value}/files/${n.file!.id}/raw`, '_blank') })
  }
  if (canWrite.value && n.file) items.push({ label: t('projects.rename'), icon: Pencil, onSelect: () => rename(n) }, { divider: true }, { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => remove(n) })
  return items
}

// --- Aperçu du site ---
const previewBase = ref('')
const previewNonce = ref(0)
const device = ref<'desktop' | 'tablet' | 'mobile'>('desktop')
async function loadPreview() {
  if (!isSite.value) return
  previewBase.value = (await api<{ base: string }>(`/api/projects/${id.value}/preview-token`, { method: 'POST', silent: true }).catch(() => ({ base: '' }))).base
}
watch([isSite, id], () => loadPreview(), { immediate: true })
/** Page HTML ouverte, sinon page d'entrée. */
const previewPath = computed(() => (current.value && /\.html?$/i.test(current.value.path) ? current.value.path : ''))
const previewSrc = computed(() => (previewBase.value ? `${previewBase.value}${previewPath.value}${previewPath.value.includes('?') ? '&' : '?'}v=${previewNonce.value}` : ''))
let reloadTimer: ReturnType<typeof setTimeout> | undefined
/** Après une modification : enregistrement par le serveur de collaboration (~2 s), puis rechargement. */
function onEdit(text: string) {
  lastText.value = text
  if (isSite.value) {
    clearTimeout(reloadTimer)
    reloadTimer = setTimeout(() => previewNonce.value++, 2600)
  }
  if (!isSite.value && !isNotebook.value && autoCompile.value) {
    clearTimeout(reloadTimer)
    reloadTimer = setTimeout(() => compile(), 2500)
  }
}
const lastText = ref('')
function openUrl(u: string) { if (u) window.open(u, '_blank', 'noopener') }
watch(() => data.value?.files.map((f) => f.updated_at).join(), () => { if (isSite.value) previewNonce.value++ })

// --- Publication ---
const publishOpen = ref(false)
const slugDraft = ref('')
const publishing = ref(false)
watch(publishOpen, (o) => { if (o) slugDraft.value = project.value?.slug ?? (project.value?.name ?? '').toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 48) })
async function setPublished(published: boolean) {
  publishing.value = true
  try {
    const r = await actions.update({ slug: slugDraft.value.trim(), published })
    toast.success(published ? t('projects.publishedToast') : t('projects.unpublishedToast'))
    if (!published || r.public_url) publishOpen.value = published
  } catch { /* toast */ } finally { publishing.value = false }
}
function copy(text: string) { navigator.clipboard.writeText(text); toast.success(t('common.linkCopied')) }

// --- LaTeX ---
const compiling = ref(false)
const result = ref<CompileResult | null>(null)
const pdfSrc = ref('')
const autoCompile = useLocalStorage('looty.latex.auto', false)
const where = useLocalStorage<'server' | 'local'>('looty.latex.where', 'server')
const showLog = ref(false)
const errors = computed(() => (result.value?.issues ?? []).filter((i) => i.level === 'error'))
const warnings = computed(() => (result.value?.issues ?? []).filter((i) => i.level === 'warning'))
watch(project, (p) => { if (p?.kind === 'latex' && p.last_build_at && !pdfSrc.value) pdfSrc.value = `/api/projects/${p.id}/output.pdf?v=${new Date(p.last_build_at).getTime()}` }, { immediate: true })
async function compile() {
  if (!project.value || isSite.value || isNotebook.value || compiling.value) return
  compiling.value = true
  try {
    const overrides: Record<string, string> = {}
    if (current.value?.text && editorRef.value) overrides[current.value.id] = editorRef.value.getText()
    if (isDesktop && where.value === 'local') {
      result.value = await compileLocal(overrides)
    } else {
      result.value = await api<CompileResult>(`/api/projects/${id.value}/compile`, { method: 'POST', body: { overrides }, silent: true })
      if (result.value.pdf) pdfSrc.value = `/api/projects/${id.value}/output.pdf?v=${Date.now()}`
    }
    if (!result.value.pdf) showLog.value = !result.value.issues.length
  } catch (e) {
    toast.error(errorMessage(e))
  } finally { compiling.value = false }
}
/** Compilation sur l'ordinateur (application de bureau) : TeX installé localement. */
async function compileLocal(overrides: Record<string, string>): Promise<CompileResult> {
  const bundle = await api<{ main: string; engine: string; files: { id: string; path: string; is_dir: boolean; text: boolean; data: string }[] }>(`/api/projects/${id.value}/bundle`)
  for (const f of bundle.files) {
    const o = overrides[f.id]
    if (o !== undefined) f.data = btoa(String.fromCharCode(...new TextEncoder().encode(o)))
  }
  const started = Date.now()
  const res = await fetch('/__looty/latex/compile', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(bundle) })
  const r = await res.json() as { ok: boolean; pdf: string | null; log: string; error?: string }
  if (r.error) throw { data: { error: r.error } }
  if (r.pdf) {
    const bytes = Uint8Array.from(atob(r.pdf), (c) => c.charCodeAt(0))
    if (pdfSrc.value.startsWith('blob:')) URL.revokeObjectURL(pdfSrc.value)
    pdfSrc.value = URL.createObjectURL(new Blob([bytes], { type: 'application/pdf' }))
  }
  const issues = parseLatexLog(r.log)
  return { ok: !!r.pdf && !issues.some((i) => i.level === 'error'), pdf: !!r.pdf, engine: bundle.engine, duration_ms: Date.now() - started, issues, log: r.log, local: true }
}
function goToIssue(i: LatexIssue) {
  if (!i.line) return
  const path = i.file ?? project.value?.settings.main ?? 'main.tex'
  const f = files.value.find((x) => x.path === path)
  if (!f) return
  if (f.id !== fileId.value) open(f.id)
  setTimeout(() => editorRef.value?.goToLine(i.line!), f.id === fileId.value ? 0 : 900)
}
const engineItems = [{ value: 'pdflatex', label: 'pdfLaTeX' }, { value: 'xelatex', label: 'XeLaTeX' }, { value: 'lualatex', label: 'LuaLaTeX' }]
const engineOpen = ref(false)
async function setEngine(engine: 'pdflatex' | 'xelatex' | 'lualatex') { await actions.update({ settings: { engine } }) }

// Raccourcis : Ctrl+S / Ctrl+Entrée compilent (LaTeX) ou rechargent l'aperçu (site).
function onKey(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && (e.key === 's' || e.key === 'Enter')) {
    e.preventDefault()
    if (isSite.value) previewNonce.value++
    else if (!isNotebook.value) compile()
  }
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => { window.removeEventListener('keydown', onKey); clearTimeout(reloadTimer); if (pdfSrc.value.startsWith('blob:')) URL.revokeObjectURL(pdfSrc.value) })

// --- Mise en page ---
const showTree = useLocalStorage('looty.project.tree', true)
const showPreview = useLocalStorage('looty.project.preview', true)
const mobilePane = ref<'files' | 'code' | 'preview'>('code')
/** Colonnes des seuls panneaux affichés (un panneau masqué ne laisse pas de colonne vide). */
const columns = computed(() => [showTree.value ? '220px' : '', 'minmax(0, 1fr)', showPreview.value && !isNotebook.value ? 'minmax(0, 1fr)' : ''].filter(Boolean).join(' '))
const moreOpen = ref(false)
const moreItems = computed<MenuItem[]>(() => [
  ...(isIpynb.value && current.value ? [{ label: t('notebook.download'), icon: Download, onSelect: () => window.open(`/api/projects/${id.value}/files/${current.value!.id}/raw`, '_blank') }] : []),
  { label: t('projects.downloadZip'), icon: Download, onSelect: () => (window.location.href = `/api/projects/${id.value}/download`) },
  ...(canWrite.value ? [{ label: t('projects.renameProject'), icon: Pencil, onSelect: renameProject }] : []),
  ...(data.value?.can_manage ? [{ divider: true }, { label: t('projects.deleteProject'), icon: Trash2, danger: true, onSelect: deleteProject }] : []),
])
async function renameProject() {
  const name = await promptDialog({ title: t('projects.renameProject'), initial: project.value?.name, confirmLabel: t('common.save') })
  if (name) await actions.update({ name })
}
async function deleteProject() {
  if (!(await confirmDialog({ title: t('projects.deleteProjectTitle', { name: project.value?.name ?? '' }), message: t('projects.deleteProjectHint'), danger: true, confirmLabel: t('common.delete'), typeToConfirm: project.value?.name }))) return
  await api(`/api/projects/${id.value}`, { method: 'DELETE' })
  navigateTo(`/w/${wsId.value}/projects`)
}
const editingIn = computed(() => (current.value?.updated_by_name ? t('projects.editedBy', { name: current.value.updated_by_name, time: relativeTime(current.value.updated_at) }) : ''))
const dropActive = ref(false)

useHead({ title: () => project.value?.name ?? t('projects.title') })
</script>

<template>
  <div class="pj" @dragover.prevent="dropActive = canWrite" @dragleave.self="dropActive = false" @drop.prevent="dropActive = false; canWrite && doUpload($event.dataTransfer?.files ?? [], folderOf(null))">
    <AppPageHeader :crumbs="[{ label: t('projects.title'), icon: FolderCode, to: `/w/${wsId}/projects` }, { label: project?.name ?? '…' }]">
      <template v-if="project">
        <span v-if="collaborators.length" class="pj-people" :title="collaborators.map((c) => c.name).join(', ')">
          <Users :size="13" /> <span v-for="c in collaborators.slice(0, 4)" :key="c.name" class="pj-dot" :style="{ background: c.color }" />
        </span>
        <span v-if="!canWrite" class="pj-ro"><Lock :size="12" /> {{ t('editor.readOnly') }}</span>
        <template v-if="isSite">
          <UiTooltip :text="t('projects.reload')" :shortcut="['mod', 'S']"><UiButton size="sm" variant="ghost" :icon="RefreshCw" :aria-label="t('projects.reload')" @click="previewNonce++" /></UiTooltip>
          <UiButton v-if="data?.can_publish" size="sm" :variant="project.published ? 'secondary' : 'primary'" :icon="Globe" @click="publishOpen = true">{{ project.published ? t('projects.published') : t('projects.publish') }}</UiButton>
        </template>
        <template v-else-if="!isNotebook">
          <UiButton size="sm" variant="primary" :icon="Play" :loading="compiling" class="pj-compile" @click="compile">{{ t('projects.compile') }} <UiKbd :keys="['mod', 'S']" /></UiButton>
        </template>
        <UiMenu v-model:open="moreOpen" :items="moreItems" placement="bottom-end">
          <UiButton size="sm" variant="ghost" :icon="Settings2" :aria-label="t('common.more')" />
        </UiMenu>
      </template>
    </AppPageHeader>

    <div v-if="isLoading" class="pj-center"><UiSpinner /></div>
    <UiEmpty v-else-if="error || !data" class="pj-center" :icon="FolderCode" :title="t('projects.notFound')" />
    <template v-else>
      <div v-if="isMobile" class="pj-mobile-tabs">
        <UiSegmented v-model="mobilePane" size="md" :options="[{ value: 'files', label: t('projects.files') }, { value: 'code', label: isNotebook ? 'Notebook' : t('projects.code') }, ...(isNotebook ? [] : [{ value: 'preview', label: isSite ? t('projects.preview') : 'PDF' }])]" />
      </div>
      <div class="pj-body" :class="`m-${mobilePane}`" :style="isMobile ? undefined : { gridTemplateColumns: columns }">
        <!-- Fichiers -->
        <aside v-show="showTree || isMobile" class="pj-tree">
          <div class="pj-tree-head">
            <span>{{ t('projects.files') }}</span>
            <template v-if="canWrite">
              <UiTooltip :text="t('projects.newFile')"><button class="pj-ic" :aria-label="t('projects.newFile')" @click="newFile()"><FilePlus :size="14" /></button></UiTooltip>
              <UiTooltip :text="t('projects.newFolder')"><button class="pj-ic" :aria-label="t('projects.newFolder')" @click="newFolder()"><FolderPlus :size="14" /></button></UiTooltip>
              <UiTooltip :text="t('projects.upload')"><button class="pj-ic" :aria-label="t('projects.upload')" @click="pickUpload()"><Upload :size="14" /></button></UiTooltip>
            </template>
          </div>
          <div class="pj-tree-body" @drop.prevent.stop="canWrite && doUpload($event.dataTransfer?.files ?? [], '')" @dragover.prevent>
            <CodeFileTree v-model:collapsed="collapsed" :nodes="tree" :selected="fileId" :main="isSite ? project?.settings.entry : project?.settings.main" :menu="nodeMenu" @open="(n) => n.file && open(n.file.id)" @drop="(folder, fl) => canWrite && doUpload(fl, folder)" @move="moveTo" />
            <p v-if="!files.length" class="pj-empty">{{ t('projects.noFiles') }}</p>
            <p v-if="canWrite" class="pj-drop-hint">{{ t('projects.dropHint') }}</p>
          </div>
          <input ref="uploadInput" type="file" multiple hidden @change="doUpload(($event.target as HTMLInputElement).files ?? [])" />
        </aside>

        <!-- Éditeur -->
        <section class="pj-code">
          <div class="pj-tabs">
            <UiButton v-if="!isMobile" size="xs" variant="ghost" :icon="showTree ? PanelLeftClose : PanelLeftOpen" :aria-label="t('projects.toggleFiles')" @click="showTree = !showTree" />
            <div class="pj-tablist" role="tablist">
              <button v-for="f in tabFiles" :key="f.id" role="tab" class="pj-tab" :class="{ on: f.id === fileId }" :aria-selected="f.id === fileId" :title="f.path" @click="open(f.id)" @auxclick.middle="closeTab(f.id)">
                <span class="truncate">{{ f.path.split('/').pop() }}</span>
                <span class="pj-tab-x" role="button" :aria-label="t('common.close')" @click.stop="closeTab(f.id)">×</span>
              </button>
            </div>
            <span class="pj-status">
              <template v-if="current?.text">
                <template v-if="status === 'synced'">{{ t('common.saved') }}</template>
                <template v-else-if="status === 'offline'">{{ t('projects.offline') }}</template>
                <template v-else-if="status === 'denied'">{{ t('editor.readOnly') }}</template>
                <template v-else>{{ t('editor.connecting') }}</template>
              </template>
            </span>
            <UiButton v-if="!isMobile && !isNotebook" size="xs" variant="ghost" :icon="showPreview ? PanelRightClose : PanelRightOpen" :aria-label="t('projects.togglePreview')" @click="showPreview = !showPreview" />
          </div>
          <div class="pj-editor">
            <NotebookEditor
              v-if="isIpynb && current" :key="current.id" ref="editorRef" :file-id="current.id" :path="current.path" :project-id="id" :project-files="files" :read-only="!canWrite" :user="cursorUser"
              @status="(s) => (status = s)" @collaborators="(c) => (collaborators = c)"
            />
            <CodeEditor
              v-else-if="current?.text" ref="editorRef" :key="current.id" :file-id="current.id" :path="current.path" :read-only="!canWrite" :user="cursorUser"
              @status="(s) => (status = s)" @change="onEdit" @collaborators="(c) => (collaborators = c)"
            />
            <div v-else-if="imagePreview" class="pj-binary"><img :src="imagePreview" :alt="current?.path" /></div>
            <div v-else-if="current" class="pj-binary">
              <FileText :size="28" />
              <p>{{ current.path }} · {{ fileSize(current.size) }}</p>
              <UiButton size="sm" :icon="Download" @click="openUrl(`/api/projects/${id}/files/${current.id}/raw`)">{{ t('projects.download') }}</UiButton>
            </div>
            <div v-else class="pj-binary muted">{{ t('projects.pickFile') }}</div>
          </div>
          <div v-if="editingIn && current" class="pj-foot">{{ current.path }} · {{ editingIn }}</div>
        </section>

        <!-- Aperçu -->
        <section v-if="!isNotebook" v-show="showPreview || isMobile" class="pj-preview">
          <template v-if="isSite">
            <div class="pv-bar">
              <UiSegmented v-model="device" :options="[{ value: 'desktop', icon: Monitor, title: t('projects.desktop') }, { value: 'tablet', icon: Tablet, title: t('projects.tablet') }, { value: 'mobile', icon: Smartphone, title: t('projects.mobile') }]" />
              <span class="pv-path truncate">/{{ previewPath || project?.settings.entry || 'index.html' }}</span>
              <UiTooltip :text="t('projects.openPreview')"><UiButton size="xs" variant="ghost" :icon="ExternalLink" :aria-label="t('projects.openPreview')" @click="openUrl(previewSrc)" /></UiTooltip>
            </div>
            <div class="pv-frame" :class="device">
              <iframe v-if="previewSrc" :src="previewSrc" :title="t('projects.preview')" sandbox="allow-scripts allow-forms allow-popups allow-modals" referrerpolicy="no-referrer" />
            </div>
          </template>
          <template v-else>
            <div class="pv-bar">
              <UiCombobox :model-value="project?.settings.engine ?? 'pdflatex'" v-model:open="engineOpen" :items="engineItems" :width="160" @update:model-value="(v) => setEngine(v as 'pdflatex')">
                <button class="pv-sel" :disabled="!canWrite">{{ engineItems.find((e) => e.value === (project?.settings.engine ?? 'pdflatex'))?.label }}</button>
              </UiCombobox>
              <UiSegmented v-if="isDesktop" v-model="where" :options="[{ value: 'server', icon: Server, title: t('projects.compileServer') }, { value: 'local', icon: Laptop, title: t('projects.compileLocal') }]" />
              <label class="pv-auto"><UiSwitch v-model="autoCompile" /> {{ t('projects.autoCompile') }}</label>
              <span class="pv-spacer" />
              <span v-if="result" class="pv-result" :class="{ ok: result.ok, bad: !result.pdf }">
                <component :is="result.ok ? CheckCircle2 : result.pdf ? AlertTriangle : AlertCircle" :size="13" />
                {{ result.ok ? t('projects.compiled', { s: (result.duration_ms / 1000).toFixed(1) }) : result.pdf ? t('projects.compiledWithErrors', { count: errors.length }) : t('projects.failed', { count: errors.length }) }}
                <template v-if="result.local"> · {{ t('projects.local') }}</template>
              </span>
              <UiTooltip :text="t('projects.log')"><UiButton size="xs" variant="ghost" :icon="ScrollText" :aria-label="t('projects.log')" :class="{ on: showLog }" @click="showLog = !showLog" /></UiTooltip>
              <UiTooltip v-if="pdfSrc" :text="t('projects.downloadPdf')"><UiButton size="xs" variant="ghost" :icon="Download" :aria-label="t('projects.downloadPdf')" @click="openUrl(pdfSrc)" /></UiTooltip>
            </div>
            <div v-if="result && (errors.length || warnings.length)" class="pv-issues">
              <button v-for="(i, k) in [...errors, ...warnings].slice(0, 30)" :key="k" class="pv-issue" :class="i.level" @click="goToIssue(i)">
                <component :is="i.level === 'error' ? AlertCircle : AlertTriangle" :size="12" />
                <span v-if="i.line" class="pv-loc">{{ i.file ?? project?.settings.main }}:{{ i.line }}</span>
                <span class="truncate">{{ i.message }}</span>
              </button>
            </div>
            <pre v-if="showLog && result" class="pv-log">{{ result.log }}</pre>
            <div v-else class="pv-pdf">
              <ViewerPdf v-if="pdfSrc" :key="pdfSrc" :src="pdfSrc" :name="`${project?.name}.pdf`" height="100%" />
              <div v-else class="pj-binary muted"><Play :size="22" /><span>{{ t('projects.compileHint') }}</span></div>
            </div>
          </template>
        </section>
      </div>
    </template>

    <UiDialog v-model:open="publishOpen" :title="t('projects.publishTitle')" :description="t('projects.publishHint')" :width="480">
      <div class="pub">
        <label class="pub-label">{{ t('projects.address') }}</label>
        <div class="pub-url"><span class="pub-base">{{ (data?.public_url?.replace(/\/pages\/.*$/, '') ?? '') || '' }}/pages/</span><UiInput v-model="slugDraft" :placeholder="t('projects.slugPlaceholder')" maxlength="64" /></div>
        <p class="pub-hint">{{ t('projects.slugHint') }}</p>
        <div v-if="project?.published && data?.public_url" class="pub-live">
          <Globe :size="14" /> <a :href="data.public_url" target="_blank" rel="noopener" class="truncate">{{ data.public_url }}</a>
          <UiButton size="xs" variant="ghost" :icon="Copy" :aria-label="t('common.copyLink')" @click="copy(data.public_url)" />
        </div>
        <p class="pub-hint">{{ t('projects.sandboxHint') }}</p>
      </div>
      <template #footer>
        <UiButton v-if="project?.published" variant="ghost" :loading="publishing" @click="setPublished(false)">{{ t('projects.unpublish') }}</UiButton>
        <span style="flex: 1" />
        <UiButton variant="primary" :icon="Globe" :loading="publishing" :disabled="slugDraft.trim().length < 2" @click="setPublished(true)">{{ project?.published ? t('common.save') : t('projects.publish') }}</UiButton>
      </template>
    </UiDialog>
    <div v-if="dropActive" class="pj-dropzone"><Upload :size="22" /> {{ t('projects.dropHere') }}</div>
  </div>
</template>

<style scoped>
.pj { display: flex; flex-direction: column; height: 100%; min-height: 0; position: relative; }
.pj-center { flex: 1; display: grid; place-items: center; }
.pj-people { display: inline-flex; align-items: center; gap: 3px; color: var(--text-3); margin-right: 4px; }
.pj-dot { width: 9px; height: 9px; border-radius: 50%; border: 1.5px solid var(--bg); }
.pj-ro { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-sm); color: var(--text-3); }
.pj-compile :deep(.kbd-group) { margin-left: 4px; opacity: .8; }
.pj-mobile-tabs { padding: 6px 10px; border-bottom: 1px solid var(--border); }
.pj-body { flex: 1; min-height: 0; display: grid; grid-template-columns: 220px minmax(0, 1fr) minmax(0, 1fr); }
.pj-tree { border-right: 1px solid var(--border); display: flex; flex-direction: column; min-height: 0; background: var(--bg-panel); overflow: hidden; }
.pj-tree-head { display: flex; align-items: center; gap: 2px; padding: 8px 8px 6px 12px; font-size: var(--fs-xs); font-weight: 600; text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); }
.pj-tree-head span { flex: 1; }
.pj-ic { width: 24px; height: 24px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); }
.pj-ic:hover { background: var(--bg-hover); color: var(--text); }
.pj-tree-body { flex: 1; overflow-y: auto; padding: 0 6px 12px; }
.pj-empty, .pj-drop-hint { font-size: var(--fs-xs); color: var(--text-3); padding: 8px 6px; margin: 0; }
.pj-code { min-width: 0; min-height: 0; display: flex; flex-direction: column; border-right: 1px solid var(--border); }
.pj-tabs { display: flex; align-items: center; gap: 4px; height: 36px; padding: 0 6px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
.pj-tablist { flex: 1; min-width: 0; display: flex; gap: 2px; overflow-x: auto; scrollbar-width: none; }
.pj-tab { display: inline-flex; align-items: center; gap: 6px; max-width: 180px; height: 28px; padding: 0 6px 0 10px; border-radius: var(--r-sm); font-size: var(--fs-sm); color: var(--text-3); flex: none; }
.pj-tab:hover { background: var(--bg-hover); color: var(--text-2); }
.pj-tab.on { background: var(--bg); color: var(--text); box-shadow: var(--shadow-card); }
.pj-tab-x { width: 16px; height: 16px; display: grid; place-items: center; border-radius: 3px; font-size: 14px; line-height: 1; color: var(--text-3); }
.pj-tab-x:hover { background: var(--bg-active); color: var(--text); }
.pj-status { font-size: var(--fs-xs); color: var(--text-3); white-space: nowrap; }
.pj-editor { flex: 1; min-height: 0; }
.pj-binary { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; color: var(--text-3); padding: 20px; text-align: center; }
.pj-binary img { max-width: 100%; max-height: 100%; object-fit: contain; background: repeating-conic-gradient(#0000000d 0 25%, transparent 0 50%) 0 0 / 16px 16px; }
.pj-binary p { margin: 0; font-size: var(--fs-sm); }
.pj-foot { padding: 3px 10px; font-size: var(--fs-xs); color: var(--text-3); border-top: 1px solid var(--border); }
.muted { color: var(--text-3); }
.pj-preview { min-width: 0; min-height: 0; display: flex; flex-direction: column; background: var(--bg-panel); overflow: hidden; }
.pv-bar { display: flex; align-items: center; gap: 6px; height: 36px; padding: 0 8px; border-bottom: 1px solid var(--border); flex-wrap: nowrap; overflow: hidden; }
.pv-path { flex: 1; min-width: 0; font-size: var(--fs-xs); color: var(--text-3); font-family: var(--font-mono); }
.pv-spacer { flex: 1; }
.pv-sel { height: 26px; padding: 0 8px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-sm); }
.pv-auto { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); white-space: nowrap; cursor: pointer; }
.pv-result { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-xs); color: #b87a1d; white-space: nowrap; }
.pv-result.ok { color: var(--green); }
.pv-result.bad { color: var(--red); }
.pv-bar :deep(.on) { background: var(--bg-active); }
.pv-frame { flex: 1; min-height: 0; display: flex; justify-content: center; background: var(--bg-hover); padding: 0; }
.pv-frame iframe { width: 100%; height: 100%; border: 0; background: #fff; }
.pv-frame.tablet iframe { width: 768px; max-width: 100%; margin: 12px 0; border-radius: 8px; box-shadow: var(--shadow-card); height: calc(100% - 24px); }
.pv-frame.mobile iframe { width: 390px; max-width: 100%; margin: 12px 0; border-radius: 12px; box-shadow: var(--shadow-card); height: calc(100% - 24px); }
.pv-issues { max-height: 150px; overflow-y: auto; border-bottom: 1px solid var(--border); padding: 4px; }
.pv-issue { width: 100%; display: flex; align-items: center; gap: 6px; padding: 4px 6px; border-radius: var(--r-sm); font-size: var(--fs-xs); text-align: left; }
.pv-issue:hover { background: var(--bg-hover); }
.pv-issue.error { color: var(--red); }
.pv-issue.warning { color: #b87a1d; }
.pv-loc { font-family: var(--font-mono); flex: none; color: var(--text-2); }
.pv-log { flex: 1; min-height: 0; overflow: auto; margin: 0; padding: 10px 12px; font: 11.5px/1.45 var(--font-mono); color: var(--text-2); white-space: pre-wrap; }
.pv-pdf { flex: 1; min-height: 0; display: flex; flex-direction: column; }
.pv-pdf > * { flex: 1; min-height: 0; }
.pub { display: flex; flex-direction: column; gap: 8px; }
.pub-label { font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; }
.pub-url { display: flex; align-items: center; gap: 4px; }
.pub-base { font-size: var(--fs-sm); color: var(--text-3); white-space: nowrap; max-width: 55%; overflow: hidden; text-overflow: ellipsis; }
.pub-hint { margin: 0; font-size: var(--fs-sm); color: var(--text-3); }
.pub-live { display: flex; align-items: center; gap: 6px; padding: 8px 10px; border-radius: var(--r-md); background: color-mix(in srgb, var(--green) 10%, transparent); font-size: var(--fs-sm); }
.pub-live a { flex: 1; min-width: 0; color: var(--text); }
.pj-dropzone { position: absolute; inset: 8px; z-index: 30; border: 2px dashed var(--accent); border-radius: var(--r-lg); background: color-mix(in srgb, var(--accent) 8%, var(--bg) 80%); display: flex; align-items: center; justify-content: center; gap: 10px; font-weight: 500; color: var(--accent-text); pointer-events: none; }
@media (max-width: 767px) {
  .pj-body { grid-template-columns: 1fr; }
  .pj-body > * { display: none !important; }
  .pj-body.m-files .pj-tree, .pj-body.m-code .pj-code, .pj-body.m-preview .pj-preview { display: flex !important; }
}
</style>
