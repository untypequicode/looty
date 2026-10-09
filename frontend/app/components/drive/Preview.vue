<script setup lang="ts">
import { ChevronLeft, ChevronRight, Download, X, FileQuestion } from 'lucide-vue-next'
import type { DriveItem } from '~/composables/useProjects'
import { fileSize } from '~/composables/useProjects'
import { useI18n } from '~/composables/useI18n'

/**
 * Aperçu d'un fichier du drive : images, PDF, vidéo, audio et texte, avec navigation
 * (flèches du clavier) parmi les fichiers du dossier.
 */
const props = defineProps<{ items: DriveItem[]; ws: string }>()
const current = defineModel<DriveItem | null>({ default: null })
const { t, formatDateTime } = useI18n()

type PreviewKind = 'image' | 'pdf' | 'video' | 'audio' | 'text' | null
const TEXT_EXT = /\.(txt|md|csv|tsv|json|xml|yaml|yml|toml|ini|log|tex|bib|py|js|ts|html|css|sql|sh|rs|c|h|cpp|java|kt|go|rb|php)$/i
const kindOf = (i: DriveItem): PreviewKind => {
  const m = i.mime ?? ''
  if (m.startsWith('image/') && !m.includes('svg')) return 'image'
  if (m === 'application/pdf') return 'pdf'
  if (m.startsWith('video/')) return 'video'
  if (m.startsWith('audio/')) return 'audio'
  if ((m.startsWith('text/') || TEXT_EXT.test(i.name)) && i.size <= 2 * 1024 * 1024) return 'text'
  return null
}

const files = computed(() => props.items.filter((i) => !i.is_dir))
const index = computed(() => files.value.findIndex((i) => i.id === current.value?.id))
const kind = computed(() => (current.value ? kindOf(current.value) : null))
const url = (i: DriveItem, inline = true) => `/api/workspaces/${props.ws}/drive/files/${i.id}/download${inline ? '?inline=1' : ''}`
const text = ref('')
const loadingText = ref(false)
watch([current, kind], async ([c, k]) => {
  text.value = ''
  if (!c || k !== 'text') return
  loadingText.value = true
  try { text.value = await (await fetch(url(c, false), { credentials: 'include' })).text() } finally { loadingText.value = false }
}, { immediate: true })

function go(delta: number) {
  const list = files.value
  if (!list.length || index.value < 0) return
  current.value = list[(index.value + delta + list.length) % list.length]!
}
function onKey(e: KeyboardEvent) {
  if (!current.value) return
  if (e.key === 'ArrowRight') { e.preventDefault(); go(1) }
  else if (e.key === 'ArrowLeft') { e.preventDefault(); go(-1) }
  else if (e.key === 'Escape') current.value = null
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
function download() { if (current.value) window.location.href = url(current.value, false) }
</script>

<template>
  <Teleport to="body">
    <div v-if="current" class="pv" role="dialog" :aria-label="current.name" @click.self="current = null">
      <header class="pv-head">
        <span class="pv-name truncate">{{ current.name }}</span>
        <span class="pv-meta">{{ fileSize(current.size) }}<template v-if="current.expires_at"> · {{ t('drive.expires') }} {{ formatDateTime(current.expires_at) }}</template></span>
        <span class="pv-count" v-if="files.length > 1">{{ index + 1 }} / {{ files.length }}</span>
        <button class="pv-btn" :aria-label="t('drive.download')" :title="t('drive.download')" @click="download"><Download :size="17" /></button>
        <button class="pv-btn" :aria-label="t('common.close')" @click="current = null"><X :size="18" /></button>
      </header>
      <div class="pv-body" @click.self="current = null">
        <button v-if="files.length > 1" class="pv-nav prev" :aria-label="t('drive.previous')" @click="go(-1)"><ChevronLeft :size="22" /></button>
        <img v-if="kind === 'image'" :key="current.id" :src="url(current)" :alt="current.name" class="pv-img" />
        <div v-else-if="kind === 'pdf'" class="pv-pdf"><ViewerPdf :key="current.id" :src="url(current)" :name="current.name" height="100%" /></div>
        <video v-else-if="kind === 'video'" :key="current.id" :src="url(current)" controls class="pv-media" />
        <audio v-else-if="kind === 'audio'" :key="current.id" :src="url(current)" controls class="pv-audio" />
        <div v-else-if="kind === 'text'" class="pv-text-wrap"><UiSpinner v-if="loadingText" /><pre v-else class="pv-text">{{ text }}</pre></div>
        <div v-else class="pv-none">
          <FileQuestion :size="36" />
          <p>{{ t('drive.noPreview') }}</p>
          <UiButton variant="primary" :icon="Download" @click="download">{{ t('drive.download') }}</UiButton>
        </div>
        <button v-if="files.length > 1" class="pv-nav next" :aria-label="t('drive.next')" @click="go(1)"><ChevronRight :size="22" /></button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.pv { position: fixed; inset: 0; z-index: 950; background: rgba(10, 10, 14, .94); backdrop-filter: blur(6px); display: flex; flex-direction: column; color: #fff; }
.pv-head { display: flex; align-items: center; gap: 12px; padding: 10px 14px; }
.pv-name { font-weight: 600; min-width: 0; }
.pv-meta, .pv-count { font-size: var(--fs-sm); color: rgba(255, 255, 255, .6); white-space: nowrap; }
.pv-count { margin-left: auto; }
.pv-btn { width: 34px; height: 34px; display: grid; place-items: center; border-radius: var(--r-md); color: #fff; }
.pv-btn:hover { background: rgba(255, 255, 255, .12); }
.pv-meta + .pv-btn { margin-left: auto; }
.pv-body { flex: 1; min-height: 0; display: flex; align-items: center; justify-content: center; position: relative; padding: 0 60px 24px; }
.pv-img { max-width: 100%; max-height: 100%; object-fit: contain; border-radius: 4px; background: repeating-conic-gradient(#ffffff14 0 25%, transparent 0 50%) 0 0 / 16px 16px; }
.pv-pdf { width: min(1000px, 100%); height: 100%; background: var(--bg); border-radius: var(--r-lg); overflow: hidden; color: var(--text); display: flex; }
.pv-pdf > * { flex: 1; }
.pv-media { max-width: 100%; max-height: 100%; border-radius: 6px; background: #000; }
.pv-audio { width: min(560px, 100%); }
.pv-text-wrap { width: min(1000px, 100%); height: 100%; overflow: auto; background: var(--bg); color: var(--text); border-radius: var(--r-lg); display: flex; }
.pv-text { margin: 0; padding: 16px 20px; font: 12.5px/1.55 var(--font-mono); white-space: pre-wrap; word-break: break-word; flex: 1; }
.pv-none { display: flex; flex-direction: column; align-items: center; gap: 12px; color: rgba(255, 255, 255, .75); }
.pv-nav { position: absolute; top: 50%; transform: translateY(-50%); width: 44px; height: 44px; border-radius: 50%; display: grid; place-items: center; color: #fff; background: rgba(255, 255, 255, .1); }
.pv-nav:hover { background: rgba(255, 255, 255, .2); }
.pv-nav.prev { left: 10px; }
.pv-nav.next { right: 10px; }
@media (max-width: 767px) { .pv-body { padding: 0 8px 12px; } .pv-nav { display: none; } .pv-meta { display: none; } }
</style>
