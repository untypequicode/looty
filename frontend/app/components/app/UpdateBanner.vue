<script setup lang="ts">
import { RefreshCw, Download, X, Sparkles } from 'lucide-vue-next'
import { useLocalStorage } from '@vueuse/core'
import { useI18n } from '~/composables/useI18n'
import { useReleases, webUpdateReady } from '~/composables/useReleases'

const { t } = useI18n()
const { appUpdate } = useReleases()
const route = useRoute()
// Annonce d'une nouvelle version de l'application : masquable, par version.
const dismissed = useLocalStorage('looty.releases.dismissed', '')
const showApp = computed(() => appUpdate.value && dismissed.value !== `${appUpdate.value.platform}-${appUpdate.value.version}`)
function reload() { window.location.reload() }
function download() {
  if (appUpdate.value) window.open(appUpdate.value.url, '_blank', 'noopener')
}
</script>

<template>
  <div v-if="webUpdateReady" class="ub" role="status">
    <RefreshCw :size="14" />
    <span>{{ t('releases.webReady') }}</span>
    <UiButton size="xs" variant="primary" @click="reload">{{ t('releases.reload') }}</UiButton>
  </div>
  <div v-else-if="showApp && appUpdate" class="ub" role="status">
    <Sparkles :size="14" />
    <span>{{ t('releases.appReady', { app: t(`releases.platform.${appUpdate.platform}`), version: appUpdate.version }) }}</span>
    <UiButton size="xs" variant="ghost" :to="`/w/${route.params.ws}/whats-new`">{{ t('releases.whatsNew') }}</UiButton>
    <UiButton size="xs" variant="primary" :icon="Download" @click="download">{{ t('releases.download') }}</UiButton>
    <button class="ub-x" :aria-label="t('common.close')" @click="dismissed = `${appUpdate.platform}-${appUpdate.version}`"><X :size="13" /></button>
  </div>
</template>

<style scoped>
.ub { display: flex; align-items: center; gap: 10px; padding: 6px 14px; background: var(--accent-soft); color: var(--accent-text, var(--text)); font-size: var(--fs-sm); border-bottom: 1px solid var(--border); flex: none; }
.ub span { flex: 1; }
.ub-x { width: 22px; height: 22px; display: grid; place-items: center; border-radius: 4px; color: var(--text-3); }
.ub-x:hover { background: var(--bg-hover); color: var(--text); }
</style>
