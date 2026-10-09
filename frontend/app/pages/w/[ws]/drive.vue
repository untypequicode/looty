<script setup lang="ts">
import {
  HardDrive, Upload, FolderPlus, Folder, File, FileImage, FileText, FileArchive, FileVideo, FileAudio, Download, Link2, Clock, Pencil,
  FolderInput, Trash2, MoreHorizontal, Search, X, ChevronRight, Settings, Timer, Eye,
} from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace } from '~/composables/useWorkspace'
import { confirmDialog, promptDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { useDrive, fileSize, type DriveItem } from '~/composables/useProjects'
import { refDebounced } from '@vueuse/core'

definePageMeta({ layout: 'workspace' })

/**
 * Drive temporaire : chaque fichier s'efface après la durée choisie (au plus le maximum
 * fixé par les administrateurs). Dossiers, envoi par glisser-déposer, liens publics.
 */
const { t, locale, formatDateTime } = useI18n()
const { wsId, isAdmin } = useWorkspace()
const route = useRoute()
const router = useRouter()
const qc = useQueryClient()
const folder = computed(() => (typeof route.query.folder === 'string' ? route.query.folder : ''))
const search = ref('')
const q = refDebounced(computed(() => search.value.trim()), 300)
const { data, isLoading, error } = useDrive(wsId, folder, q)
const refresh = () => qc.invalidateQueries({ queryKey: ['drive', wsId.value] })
const crumbs = computed(() => {
  const parts = folder.value ? folder.value.split('/') : []
  return parts.map((name, i) => ({ name, path: parts.slice(0, i + 1).join('/') }))
})
function go(path: string) { search.value = ''; router.push({ query: { ...route.query, folder: path || undefined } }) }
const fullPath = (i: DriveItem) => (i.folder ? `${i.folder}/${i.name}` : i.name)

// --- Durée ---
const days = ref<number>(0)
watch(() => data.value?.max_days, (m) => { if (m && (!days.value || days.value > m)) days.value = m }, { immediate: true })
const dayChoices = computed(() => (data.value?.day_choices ?? []).filter((d) => d <= (data.value?.max_days ?? 7)))
const dayLabel = (d: number) => t('drive.days', { count: d })
const daysOpen = ref(false)

// --- Envoi (avec progression) ---
const uploads = ref<{ name: string; loaded: number; total: number }[]>([])
const fileInput = ref<HTMLInputElement | null>(null)
const folderInput = ref<HTMLInputElement | null>(null)
async function send(list: FileList | File[]) {
  const arr = [...list]
  if (!arr.length || !data.value?.can_upload) return
  const total = arr.reduce((n, f) => n + f.size, 0)
  if (data.value && data.value.used_bytes + total > data.value.quota_bytes) { toast.error(t('errors.drive_quota_exceeded')); return }
  const form = new FormData()
  form.append('folder', folder.value)
  form.append('days', String(days.value))
  for (const f of arr) form.append('file', f, (f as File & { webkitRelativePath?: string }).webkitRelativePath || f.name)
  const entry = { name: arr.length === 1 ? arr[0]!.name : t('drive.nFiles', { count: arr.length }), loaded: 0, total }
  uploads.value = [...uploads.value, entry]
  try {
    await new Promise<void>((resolve, reject) => {
      const xhr = new XMLHttpRequest()
      xhr.open('POST', `/api/workspaces/${wsId.value}/drive/upload`)
      xhr.withCredentials = true
      xhr.upload.onprogress = (e) => { entry.loaded = Math.min(e.loaded, entry.total); uploads.value = [...uploads.value] }
      xhr.onload = () => (xhr.status < 300 ? resolve() : reject({ data: JSON.parse(xhr.responseText || '{}') }))
      xhr.onerror = () => reject(new Error('network'))
      xhr.send(form)
    })
    toast.success(t('drive.uploaded', { count: arr.length, days: dayLabel(days.value) }))
  } catch (e) {
    toast.error(errorMessage(e))
  } finally {
    uploads.value = uploads.value.filter((u) => u !== entry)
    if (fileInput.value) fileInput.value.value = ''
    if (folderInput.value) folderInput.value.value = ''
    refresh()
  }
}
const dragging = ref(false)
function onDrop(e: DragEvent) { dragging.value = false; if (e.dataTransfer?.files.length) send(e.dataTransfer.files) }

// --- Actions ---
async function newFolder() {
  const name = await promptDialog({ title: t('drive.newFolder'), placeholder: t('drive.folderName'), confirmLabel: t('common.create') })
  if (!name) return
  await api(`/api/workspaces/${wsId.value}/drive/folders`, { method: 'POST', body: { folder: folder.value, name } }).catch(() => {})
  refresh()
}
const fileUrl = (i: DriveItem, inline = false) => `/api/workspaces/${wsId.value}/drive/files/${i.id}/download${inline ? '?inline=1' : ''}`
/** Fichier affiché dans l'aperçu (images, PDF, vidéo, audio, texte). */
const preview = ref<DriveItem | null>(null)
function openItem(i: DriveItem) {
  if (i.is_dir) go(fullPath(i))
  else preview.value = i
}
async function rename(i: DriveItem) {
  const name = await promptDialog({ title: t('drive.rename'), initial: i.name, confirmLabel: t('common.save') })
  if (!name || name === i.name) return
  await api(`/api/workspaces/${wsId.value}/drive/files/${i.id}`, { method: 'PUT', body: { name } }).catch(() => {})
  refresh()
}
async function move(i: DriveItem) {
  const to = await promptDialog({ title: t('drive.moveTitle', { name: i.name }), initial: i.folder, placeholder: t('drive.movePlaceholder'), confirmLabel: t('drive.move') })
  if (to === null || to === i.folder) return
  await api(`/api/workspaces/${wsId.value}/drive/files/${i.id}`, { method: 'PUT', body: { folder: to } }).catch(() => {})
  refresh()
}
async function extend(i: DriveItem, d: number) {
  await api(`/api/workspaces/${wsId.value}/drive/files/${i.id}`, { method: 'PUT', body: { days: d } }).catch(() => {})
  toast.success(t('drive.extended', { days: dayLabel(d) }))
  refresh()
}
async function shareLink(i: DriveItem) {
  try {
    const r = await api<{ url: string; expires_at: string }>(`/api/workspaces/${wsId.value}/drive/files/${i.id}/share`, { method: 'POST' })
    await navigator.clipboard.writeText(r.url)
    toast.success(t('drive.linkCopied', { date: formatDateTime(r.expires_at) }))
    refresh()
  } catch { /* toast */ }
}
async function unshare(i: DriveItem) {
  await api(`/api/workspaces/${wsId.value}/drive/files/${i.id}/share`, { method: 'DELETE' }).catch(() => {})
  toast.success(t('drive.linkRemoved'))
  refresh()
}
async function remove(i: DriveItem) {
  if (!(await confirmDialog({ title: t('drive.deleteTitle', { name: i.name }), message: i.is_dir ? t('drive.deleteFolderHint') : t('drive.deleteHint'), danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/workspaces/${wsId.value}/drive/files/${i.id}`, { method: 'DELETE' }).catch(() => {})
  refresh()
}
const canEdit = (i: DriveItem) => !!data.value?.can_upload && (i.uploaded_by === data.value.user_id || data.value.can_manage)
const menuFor = ref<string | null>(null)
function items(i: DriveItem): MenuItem[] {
  const out: MenuItem[] = []
  if (!i.is_dir) out.push({ label: t('drive.preview'), icon: Eye, onSelect: () => (preview.value = i) }, { label: t('drive.download'), icon: Download, onSelect: () => (window.location.href = fileUrl(i)) })
  if (!i.is_dir) out.push({ label: i.shared ? t('drive.copyLinkAgain') : t('drive.shareLink'), icon: Link2, onSelect: () => shareLink(i) })
  if (!i.is_dir && i.shared) out.push({ label: t('drive.removeLink'), icon: X, onSelect: () => unshare(i) })
  if (canEdit(i)) {
    if (!i.is_dir) {
      out.push({ divider: true }, { heading: t('drive.keepFor') })
      for (const d of dayChoices.value) out.push({ label: dayLabel(d), icon: Clock, onSelect: () => extend(i, d) })
    }
    out.push({ divider: true }, { label: t('drive.rename'), icon: Pencil, onSelect: () => rename(i) }, { label: t('drive.move'), icon: FolderInput, onSelect: () => move(i) }, { divider: true }, { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => remove(i) })
  }
  return out
}

// --- Affichage ---
function iconOf(i: DriveItem) {
  if (i.is_dir) return Folder
  const m = i.mime ?? ''
  if (m.startsWith('image/')) return FileImage
  if (m.startsWith('video/')) return FileVideo
  if (m.startsWith('audio/')) return FileAudio
  if (/zip|tar|gzip|7z|rar/.test(m)) return FileArchive
  if (m.startsWith('text/') || m.includes('pdf') || m.includes('document')) return FileText
  return File
}
/** « dans 3 jours », « dans 5 h » ; rouge à moins de 24 h. */
function remaining(iso: string | null) {
  if (!iso) return ''
  const ms = new Date(iso).getTime() - Date.now()
  const rtf = new Intl.RelativeTimeFormat(locale.value, { numeric: 'auto' })
  if (ms < 3600_000) return rtf.format(Math.max(1, Math.round(ms / 60_000)), 'minute')
  if (ms < 86400_000) return rtf.format(Math.round(ms / 3600_000), 'hour')
  return rtf.format(Math.round(ms / 86400_000), 'day')
}
const soon = (iso: string | null) => !!iso && new Date(iso).getTime() - Date.now() < 86400_000
const usage = computed(() => (data.value ? Math.min(100, (data.value.used_bytes / Math.max(1, data.value.quota_bytes)) * 100) : 0))
useHead({ title: () => t('drive.title') })
</script>

<template>
  <div class="dv" @dragover.prevent="dragging = !!data?.can_upload" @dragleave.self="dragging = false" @drop.prevent="onDrop">
    <AppPageHeader :crumbs="[{ label: t('drive.title'), icon: HardDrive }]">
      <div class="dv-search">
        <Search :size="14" />
        <input v-model="search" :placeholder="t('drive.search')" />
        <button v-if="search" :aria-label="t('common.clear')" @click="search = ''"><X :size="13" /></button>
      </div>
      <template v-if="data?.can_upload">
        <UiButton size="sm" variant="ghost" :icon="FolderPlus" @click="newFolder">{{ t('drive.newFolder') }}</UiButton>
        <UiCombobox :model-value="String(days)" v-model:open="daysOpen" :items="dayChoices.map((d) => ({ value: String(d), label: dayLabel(d) }))" :width="180" @update:model-value="(v) => (days = Number(v))">
          <UiButton size="sm" variant="ghost" :icon="Timer" :title="t('drive.keepFor')">{{ dayLabel(days) }}</UiButton>
        </UiCombobox>
        <UiButton size="sm" variant="primary" :icon="Upload" @click="fileInput?.click()">{{ t('drive.upload') }}</UiButton>
      </template>
      <UiTooltip v-if="isAdmin" :text="t('drive.settings')"><UiButton size="sm" variant="ghost" :icon="Settings" :aria-label="t('drive.settings')" @click="navigateTo(`/w/${wsId}/settings/workspace`)" /></UiTooltip>
    </AppPageHeader>
    <input ref="fileInput" type="file" multiple hidden @change="send(($event.target as HTMLInputElement).files ?? [])" />
    <input ref="folderInput" type="file" multiple hidden webkitdirectory @change="send(($event.target as HTMLInputElement).files ?? [])" />

    <div class="dv-body">
      <div class="dv-top">
        <nav class="dv-crumbs" :aria-label="t('drive.title')">
          <button :class="{ on: !folder }" @click="go('')"><HardDrive :size="14" /> {{ t('drive.root') }}</button>
          <template v-for="c in crumbs" :key="c.path"><ChevronRight :size="13" class="faint" /><button :class="{ on: c.path === folder }" @click="go(c.path)">{{ c.name }}</button></template>
        </nav>
        <div v-if="data" class="dv-quota" :title="t('drive.quota', { used: fileSize(data.used_bytes), total: fileSize(data.quota_bytes) })">
          <span class="dv-bar"><span :style="{ width: `${usage}%` }" :class="{ high: usage > 85 }" /></span>
          <span>{{ fileSize(data.used_bytes) }} / {{ fileSize(data.quota_bytes) }}</span>
        </div>
      </div>
      <p v-if="data" class="dv-note"><Clock :size="13" /> {{ t('drive.note', { days: dayLabel(data.max_days) }) }}</p>

      <div v-for="(u, k) in uploads" :key="k" class="dv-upload">
        <Upload :size="14" /> <span class="truncate">{{ u.name }}</span>
        <span class="dv-bar"><span :style="{ width: `${u.total ? (u.loaded / u.total) * 100 : 0}%` }" /></span>
        <span class="faint">{{ fileSize(u.loaded) }} / {{ fileSize(u.total) }}</span>
      </div>

      <div v-if="isLoading" class="dv-list"><div v-for="i in 5" :key="i" class="skeleton" style="height: 44px; margin: 4px 0" /></div>
      <p v-else-if="error" class="dv-empty">{{ errorMessage(error) }}</p>
      <UiEmpty v-else-if="!data?.items.length" :icon="HardDrive" :title="q ? t('drive.noResults') : t('drive.emptyTitle')" :description="q ? '' : t('drive.emptyHint')">
        <div v-if="data?.can_upload && !q" class="dv-empty-actions">
          <UiButton variant="primary" :icon="Upload" @click="fileInput?.click()">{{ t('drive.upload') }}</UiButton>
          <UiButton :icon="FolderInput" @click="folderInput?.click()">{{ t('drive.uploadFolder') }}</UiButton>
        </div>
      </UiEmpty>
      <table v-else class="dv-table">
        <thead><tr><th>{{ t('drive.name') }}</th><th class="num">{{ t('drive.size') }}</th><th>{{ t('drive.expires') }}</th><th class="hide-sm">{{ t('drive.by') }}</th><th /></tr></thead>
        <tbody>
          <tr v-for="i in data.items" :key="i.id" class="dv-row" @dblclick="openItem(i)">
            <td>
              <button class="dv-name" @click="openItem(i)">
                <component :is="iconOf(i)" :size="16" class="dv-ic" :class="{ dir: i.is_dir }" />
                <span class="truncate">{{ i.name }}</span>
                <Link2 v-if="i.shared" :size="12" class="faint" :aria-label="t('drive.shared')" />
              </button>
              <div v-if="q && i.folder" class="dv-in faint">{{ i.folder }}</div>
            </td>
            <td class="num faint">{{ i.is_dir ? t('drive.items', { count: i.children }) : fileSize(i.size) }}</td>
            <td><span v-if="i.expires_at" class="dv-exp" :class="{ soon: soon(i.expires_at) }" :title="formatDateTime(i.expires_at)"><Clock :size="12" /> {{ remaining(i.expires_at) }}</span></td>
            <td class="hide-sm faint truncate">{{ i.uploaded_by_name }}</td>
            <td class="dv-act">
              <UiMenu :open="menuFor === i.id" :items="items(i)" placement="bottom-end" @update:open="(o: boolean) => (menuFor = o ? i.id : null)">
                <button class="dv-more" :aria-label="t('common.more')"><MoreHorizontal :size="15" /></button>
              </UiMenu>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <DrivePreview v-model="preview" :items="data?.items ?? []" :ws="wsId" />
    <div v-if="dragging" class="dv-drop"><Upload :size="22" /> {{ t('drive.dropHere', { days: dayLabel(days) }) }}</div>
  </div>
</template>

<style scoped>
.dv { display: flex; flex-direction: column; height: 100%; min-height: 0; position: relative; }
.dv-search { display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 8px; border-radius: var(--r-md); background: var(--bg-input); border: 1px solid var(--border); color: var(--text-3); width: 220px; }
.dv-search input { flex: 1; min-width: 0; border: 0; outline: none; background: transparent; color: var(--text); font: inherit; font-size: var(--fs-sm); }
.dv-search button { display: grid; place-items: center; color: var(--text-3); }
.dv-body { flex: 1; overflow-y: auto; padding: 14px 24px 48px; }
.dv-top { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
.dv-crumbs { display: flex; align-items: center; gap: 2px; flex-wrap: wrap; }
.dv-crumbs button { display: inline-flex; align-items: center; gap: 6px; padding: 4px 8px; border-radius: var(--r-sm); color: var(--text-2); font-weight: 500; }
.dv-crumbs button:hover { background: var(--bg-hover); }
.dv-crumbs button.on { color: var(--text); }
.dv-quota { display: flex; align-items: center; gap: 8px; font-size: var(--fs-xs); color: var(--text-3); }
.dv-bar { width: 120px; height: 6px; border-radius: 3px; background: var(--bg-active); overflow: hidden; display: inline-block; flex: none; }
.dv-bar span { display: block; height: 100%; background: #2a78d6; border-radius: 3px; transition: width .2s; }
.dv-bar span.high { background: var(--red); }
.dv-note { display: flex; align-items: center; gap: 6px; margin: 8px 0 12px; font-size: var(--fs-sm); color: var(--text-3); }
.dv-upload { display: flex; align-items: center; gap: 10px; padding: 8px 12px; margin-bottom: 6px; border-radius: var(--r-md); background: var(--bg-panel); box-shadow: var(--shadow-card); font-size: var(--fs-sm); }
.dv-upload .truncate { flex: 1; min-width: 0; }
.dv-table { width: 100%; border-collapse: collapse; font-size: var(--fs-sm); }
.dv-table th { text-align: left; font-weight: 500; color: var(--text-3); padding: 6px 8px; border-bottom: 1px solid var(--border); }
.dv-table td { padding: 4px 8px; border-bottom: 1px solid var(--border); vertical-align: middle; }
.dv-table .num { text-align: right; white-space: nowrap; }
.dv-row:hover { background: var(--bg-hover); }
.dv-name { display: flex; align-items: center; gap: 10px; min-width: 0; max-width: 100%; padding: 6px 0; color: var(--text); text-align: left; }
.dv-ic { flex: none; color: var(--text-3); }
.dv-ic.dir { color: #c58b2a; }
.dv-in { font-size: var(--fs-xs); margin: -4px 0 4px 26px; }
.dv-exp { display: inline-flex; align-items: center; gap: 4px; color: var(--text-2); white-space: nowrap; }
.dv-exp.soon { color: var(--red); }
.dv-act { width: 36px; }
.dv-more { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); }
.dv-more:hover { background: var(--bg-active); color: var(--text); }
.dv-empty { color: var(--text-3); text-align: center; padding: 24px; }
.dv-empty-actions { display: flex; gap: 8px; }
.faint { color: var(--text-3); }
.dv-drop { position: absolute; inset: 8px; z-index: 30; border: 2px dashed var(--accent); border-radius: var(--r-lg); background: color-mix(in srgb, var(--accent) 8%, var(--bg) 80%); display: flex; align-items: center; justify-content: center; gap: 10px; font-weight: 500; color: var(--accent-text); pointer-events: none; }
@media (max-width: 767px) { .dv-body { padding: 12px; } .hide-sm { display: none; } .dv-search { width: 140px; } }
</style>
