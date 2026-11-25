<script setup lang="ts">
import { Maximize2, PictureInPicture2 } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'

/**
 * Un flux vidéo d'appel (caméra ou partage d'écran) avec plein écran et image dans l'image.
 * `muted` : l'aperçu local est toujours muet ; le son d'un partage distant passe par cette vidéo.
 */
const props = defineProps<{ stream: MediaStream | null; muted?: boolean; mirror?: boolean; label: string; big?: boolean; speaking?: boolean; contain?: boolean }>()
const { t } = useI18n()
const el = ref<HTMLVideoElement | null>(null)
const box = ref<HTMLElement | null>(null)
const pip = typeof document !== 'undefined' && !!document.pictureInPictureEnabled

watchEffect(() => {
  const v = el.value
  if (!v) return
  if (v.srcObject !== props.stream) v.srcObject = props.stream
  if (props.stream) v.play().catch(() => { /* geste requis : reprise au prochain clic */ })
})
onBeforeUnmount(() => { if (el.value) el.value.srcObject = null })

function fullscreen() {
  if (document.fullscreenElement) void document.exitFullscreen()
  else void box.value?.requestFullscreen?.().catch(() => {})
}
async function togglePip() {
  try {
    if (document.pictureInPictureElement) await document.exitPictureInPicture()
    else await el.value?.requestPictureInPicture()
  } catch { /* non disponible pour ce flux */ }
}
</script>

<template>
  <figure ref="box" class="cv" :class="{ big, speaking, contain }">
    <video ref="el" autoplay playsinline :muted="muted" :class="{ mirror }" />
    <figcaption class="cv-label">{{ label }}</figcaption>
    <span class="cv-tools">
      <button v-if="pip" type="button" class="cv-btn" :aria-label="t('calls.pip')" :title="t('calls.pip')" @click="togglePip"><PictureInPicture2 :size="14" /></button>
      <button type="button" class="cv-btn" :aria-label="t('calls.fullscreen')" :title="t('calls.fullscreen')" @click="fullscreen"><Maximize2 :size="14" /></button>
    </span>
  </figure>
</template>

<style scoped>
.cv { position: relative; margin: 0; background: #0b0b0c; border-radius: var(--r-sm); overflow: hidden; aspect-ratio: 16 / 9; outline: 2px solid transparent; outline-offset: -2px; }
.cv.speaking { outline-color: var(--accent); }
.cv.big { aspect-ratio: auto; max-height: min(52vh, 560px); }
.cv video { width: 100%; height: 100%; object-fit: cover; display: block; background: #0b0b0c; }
.cv.contain video, .cv.big video { object-fit: contain; }
.cv video.mirror { transform: scaleX(-1); }
.cv-label { position: absolute; left: 6px; bottom: 6px; max-width: calc(100% - 64px); padding: 1px 6px; border-radius: var(--r-xs); background: rgba(0, 0, 0, 0.6); color: #fff; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cv-tools { position: absolute; right: 4px; top: 4px; display: flex; gap: 4px; opacity: 0; transition: opacity 0.12s; }
.cv:hover .cv-tools, .cv:focus-within .cv-tools { opacity: 1; }
.cv-btn { display: grid; place-items: center; width: 24px; height: 24px; border-radius: var(--r-xs); background: rgba(0, 0, 0, 0.6); color: #fff; }
.cv-btn:focus-visible { outline: 2px solid var(--accent); }
@media (hover: none) { .cv-tools { opacity: 1; } }
</style>
