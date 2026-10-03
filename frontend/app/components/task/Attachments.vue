<script setup lang="ts">
import { Paperclip, Upload, FileText, FileArchive, FileImage, FileVideo, File, Trash2, Download, X } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { TaskAttachment } from '~/types'
import { api, uploadFile } from '~/composables/api'
import { confirmDialog } from '~/composables/useConfirm'
import { useI18n } from '~/composables/useI18n'
import { motion } from '~/composables/useMotion'

const props = defineProps<{ taskId: string; canWrite: boolean }>()
const { t, relativeTime } = useI18n()
const qc = useQueryClient()
const key = computed(() => ['attachments', props.taskId])
const { data: attachments } = useQuery({ queryKey: key, queryFn: () => api<TaskAttachment[]>(`/api/boards/tasks/${props.taskId}/attachments`) })

const uploading = ref(0)
const dragOver = ref(false)
const input = ref<HTMLInputElement | null>(null)
const preview = ref<TaskAttachment | null>(null)
const pdfPreview = ref<TaskAttachment | null>(null)
const pdfOpen = computed({ get: () => !!pdfPreview.value, set: (v) => { if (!v) pdfPreview.value = null } })

const url = (a: TaskAttachment) => `/${a.file_path}`
const isImage = (a: TaskAttachment) => (a.mime_type ?? '').startsWith('image/')
const isPdf = (a: TaskAttachment) => a.mime_type === 'application/pdf'
const images = computed(() => (attachments.value ?? []).filter(isImage))
const files = computed(() => (attachments.value ?? []).filter((a) => !isImage(a)))

function iconFor(a: TaskAttachment) {
  const m = a.mime_type ?? ''
  if (m.startsWith('video/')) return FileVideo
  if (m.includes('zip') || m.includes('compressed')) return FileArchive
  if (m.includes('pdf') || m.startsWith('text/')) return FileText
  if (m.startsWith('image/')) return FileImage
  return File
}
function size(n?: number | null) {
  if (!n) return ''
  const u = ['B', 'KB', 'MB', 'GB']
  const i = Math.min(u.length - 1, Math.floor(Math.log(n) / Math.log(1024)))
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${u[i]}`
}

async function upload(list: FileList | File[]) {
  for (const f of [...list]) {
    uploading.value++
    try {
      const a = await uploadFile(f, `/api/boards/tasks/${props.taskId}/attachments`)
      qc.setQueryData<TaskAttachment[]>(key.value, (old) => [a as unknown as TaskAttachment, ...(old ?? [])])
    } finally {
      uploading.value--
    }
  }
}
function onDrop(e: DragEvent) {
  dragOver.value = false
  if (e.dataTransfer?.files?.length && props.canWrite) upload(e.dataTransfer.files)
}
async function remove(a: TaskAttachment) {
  if (!(await confirmDialog({ title: t('attachments.deleteTitle'), message: a.filename, danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/boards/tasks/${props.taskId}/attachments/${a.id}`, { method: 'DELETE' })
  qc.setQueryData<TaskAttachment[]>(key.value, (old) => old?.filter((x) => x.id !== a.id))
}
function open(a: TaskAttachment) {
  if (isImage(a)) preview.value = a
  else if (isPdf(a)) pdfPreview.value = a
  else window.open(url(a), '_blank', 'noopener')
}

const lightbox = ref<HTMLElement | null>(null)
watch(preview, (p) => { if (p) nextTick(() => motion.dialog.onEnter(lightbox.value!, () => {})) })
</script>

<template>
  <section class="att" :class="{ over: dragOver }" @dragover.prevent="canWrite && (dragOver = true)" @dragleave.self="dragOver = false" @drop.prevent="onDrop">
    <header class="att-head">
      <Paperclip :size="14" class="faint" />
      <span class="att-title">{{ t('attachments.title') }}</span>
      <span v-if="attachments?.length" class="faint">{{ attachments.length }}</span>
      <UiSpinner v-if="uploading" :size="12" />
      <span class="spacer" />
      <UiButton v-if="canWrite" size="xs" variant="ghost" :icon="Upload" @click="input?.click()" />
      <input ref="input" type="file" multiple hidden @change="upload(($event.target as HTMLInputElement).files!); ($event.target as HTMLInputElement).value = ''" />
    </header>

    <div v-if="images.length" class="att-grid">
      <button v-for="a in images" :key="a.id" class="att-thumb" @click="open(a)">
        <img :src="url(a)" :alt="a.filename" loading="lazy" />
        <span v-if="canWrite" class="att-del" role="button" @click.stop="remove(a)"><Trash2 :size="12" /></span>
      </button>
    </div>
    <div v-if="files.length" class="att-files">
      <div v-for="a in files" :key="a.id" class="att-file" @click="open(a)">
        <component :is="iconFor(a)" :size="18" class="att-ficon" :class="{ pdf: isPdf(a) }" />
        <span class="att-fname truncate">{{ a.filename }}</span>
        <span class="att-fmeta">{{ size(a.size_bytes) }} · {{ relativeTime(a.created_at) }}</span>
        <a class="att-act" :href="url(a)" download @click.stop><Download :size="14" /></a>
        <button v-if="canWrite" class="att-act" @click.stop="remove(a)"><Trash2 :size="14" /></button>
      </div>
    </div>
    <button v-if="!attachments?.length && canWrite" class="att-drop" @click="input?.click()">
      <Upload :size="15" /> {{ t('attachments.drop') }}
    </button>

    <UiDialog v-model:open="pdfOpen" :title="pdfPreview?.filename" :width="960">
      <ViewerPdf v-if="pdfPreview" :src="url(pdfPreview)" :name="pdfPreview.filename" height="72vh" />
    </UiDialog>
    <Teleport to="body">
      <div v-if="preview" class="lb" @click="preview = null">
        <div ref="lightbox" class="lb-inner" @click.stop>
          <img :src="url(preview)" :alt="preview.filename" />
          <div class="lb-bar">
            <span class="truncate">{{ preview.filename }}</span>
            <a :href="url(preview)" download class="lb-btn"><Download :size="15" /></a>
            <button class="lb-btn" @click="preview = null"><X :size="15" /></button>
          </div>
        </div>
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
.att { border-radius: var(--r-md); transition: box-shadow var(--dur-fast); }
.att.over { box-shadow: 0 0 0 2px var(--border-focus); background: var(--accent-soft); }
.att-head { display: flex; align-items: center; gap: 8px; height: 30px; }
.att-title { font-weight: 600; }
.att-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(110px, 1fr)); gap: 8px; margin-top: 6px; }
.att-thumb { position: relative; aspect-ratio: 4 / 3; border-radius: var(--r-md); overflow: hidden; box-shadow: var(--shadow-card); background: var(--bg-card); }
.att-thumb img { width: 100%; height: 100%; object-fit: cover; transition: transform var(--dur-slow) var(--ease-out); }
.att-thumb:hover img { transform: scale(1.04); }
.att-del {
  position: absolute; top: 6px; right: 6px; width: 24px; height: 24px; border-radius: var(--r-sm); display: grid; place-items: center;
  background: rgba(0, 0, 0, 0.6); color: #fff; opacity: 0; transition: opacity var(--dur-fast);
}
.att-thumb:hover .att-del { opacity: 1; }
.att-files { display: flex; flex-direction: column; gap: 2px; margin-top: 6px; }
.att-file { display: flex; align-items: center; gap: 10px; height: 38px; padding: 0 8px; border-radius: var(--r-sm); cursor: pointer; }
.att-file:hover { background: var(--bg-hover); }
.att-ficon { color: var(--text-2); flex: none; }
.att-ficon.pdf { color: var(--red); }
.att-fname { flex: 1; font-weight: 500; }
.att-fmeta { color: var(--text-3); font-size: var(--fs-sm); flex: none; }
.att-act { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); opacity: 0; }
.att-file:hover .att-act { opacity: 1; }
.att-act:hover { background: var(--bg-active); color: var(--text); }
.att-drop {
  display: flex; align-items: center; justify-content: center; gap: 8px; width: 100%; height: 56px; margin-top: 6px;
  border: 1px dashed var(--border-strong); border-radius: var(--r-md); color: var(--text-3);
}
.att-drop:hover { color: var(--text); background: var(--bg-hover); }
.lb { position: fixed; inset: 0; z-index: 2000; background: rgba(0, 0, 0, 0.82); display: grid; place-items: center; padding: 24px; }
.lb-inner { max-width: 92vw; max-height: 90vh; display: flex; flex-direction: column; gap: 8px; }
.lb-inner img { max-width: 92vw; max-height: calc(90vh - 48px); object-fit: contain; border-radius: var(--r-md); }
.lb-bar { display: flex; align-items: center; gap: 8px; color: #ddd; font-size: var(--fs-sm); }
.lb-bar span { flex: 1; }
.lb-btn { width: 32px; height: 32px; display: grid; place-items: center; border-radius: var(--r-sm); color: #ddd; }
.lb-btn:hover { background: rgba(255, 255, 255, 0.12); }
@media (pointer: coarse) { .att-act, .att-del { opacity: 1; } }
</style>
