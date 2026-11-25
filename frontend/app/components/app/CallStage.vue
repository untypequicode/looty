<script setup lang="ts">
import { MonitorOff } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { callState, peerList, stopShare } from '~/composables/useCall'

/**
 * Scène vidéo de l'appel : les partages d'écran en grand (qui partage, bouton pour arrêter), puis la grille
 * des caméras, la personne qui parle en surbrillance. Rien n'est monté s'il n'y a aucune vidéo.
 */
const { t } = useI18n()
const nameOf = (n: string) => n || t('calls.unknown')
const shares = computed(() => peerList.value.filter((p) => p.shareStream))
const cams = computed(() => {
  const list = peerList.value.filter((p) => p.camStream).map((p) => ({ id: p.id, name: nameOf(p.name), stream: p.camStream, speaking: p.speaking, self: false }))
  if (callState.localCam) list.unshift({ id: '_me', name: t('calls.you'), stream: callState.localCam, speaking: callState.selfSpeaking, self: true })
  return list
})
const any = computed(() => shares.value.length > 0 || cams.value.length > 0 || !!callState.localShare)
</script>

<template>
  <div v-if="any" class="stage" :aria-label="t('calls.video')" role="group">
    <div v-if="callState.localShare" class="stage-share">
      <AppCallVideo :stream="callState.localShare" muted big :label="t('calls.youShare')" />
      <button type="button" class="stage-stop" @click="stopShare"><MonitorOff :size="14" /> {{ t('calls.stopShare') }}</button>
    </div>
    <div v-for="p in shares" :key="p.id" class="stage-share">
      <AppCallVideo :stream="p.shareStream" big :label="t('calls.sharing', { name: nameOf(p.name) })" />
    </div>
    <div v-if="cams.length" class="stage-grid" :class="{ small: shares.length || callState.localShare }" :data-count="cams.length">
      <AppCallVideo v-for="c in cams" :key="c.id" :stream="c.stream" :muted="c.self" :mirror="c.self" :speaking="c.speaking" :label="c.name" />
    </div>
  </div>
</template>

<style scoped>
.stage { display: flex; flex-direction: column; gap: 6px; }
.stage-share { display: flex; flex-direction: column; gap: 4px; align-items: stretch; }
.stage-stop { align-self: flex-start; display: inline-flex; align-items: center; gap: 6px; padding: 3px 10px; border-radius: var(--r-sm); background: var(--red); color: #fff; font-size: var(--fs-sm, 12px); }
.stage-stop:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.stage-grid { display: grid; gap: 6px; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); }
.stage-grid[data-count='1'] { grid-template-columns: minmax(0, 360px); justify-content: center; }
.stage-grid.small { grid-template-columns: repeat(auto-fit, minmax(110px, 1fr)); }
</style>
