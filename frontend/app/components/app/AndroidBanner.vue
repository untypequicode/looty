<script setup lang="ts">
import { Smartphone, X } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { isAndroidBrowser, androidBannerHidden, useAppHandoff } from '~/composables/useAppHandoff'
import { isPane } from '~/composables/usePane'

const { t } = useI18n()
const route = useRoute()
const { intentUrl } = useAppHandoff()
// Navigateur Android seulement ; jamais de redirection automatique.
const show = computed(() => isAndroidBrowser && !isPane && !androidBannerHidden.value)
</script>

<template>
  <div v-if="show" class="android-banner" role="status">
    <Smartphone :size="14" />
    <span>{{ t('handoff.openInApp') }}</span>
    <a class="android-banner-open" :href="intentUrl(route.fullPath)">{{ t('handoff.open') }}</a>
    <button class="android-banner-x" :aria-label="t('common.close')" @click="androidBannerHidden = true"><X :size="13" /></button>
  </div>
</template>

<style scoped>
/* En bas, au-dessus du panneau de tâche plein écran (z-index 60) et sous les dialogues. */
.android-banner {
  position: fixed; left: 0; right: 0; bottom: 0; z-index: 70;
  display: flex; align-items: center; gap: 10px; padding: 8px 14px calc(8px + var(--safe-bottom, 0px));
  background: var(--bg-elevated); color: var(--text); font-size: var(--fs-sm); border-top: 1px solid var(--border); box-shadow: var(--shadow-md);
}
.android-banner span { flex: 1; }
.android-banner-open { padding: 3px 10px; border-radius: var(--r-sm); background: var(--accent); color: var(--text-on-accent); font-weight: 500; }
.android-banner-open:hover { background: var(--accent-hover); }
.android-banner-x { width: 22px; height: 22px; display: grid; place-items: center; border-radius: 4px; color: var(--text-3); }
.android-banner-x:hover { background: var(--bg-hover); color: var(--text); }
</style>
