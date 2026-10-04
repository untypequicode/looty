<script setup lang="ts">
import { CloudOff, RefreshCw } from 'lucide-vue-next'
import { net, isDesktop } from '~/composables/useDesktop'
import { useI18n } from '~/composables/useI18n'

const { t } = useI18n()
const show = computed(() => isDesktop && (!net.value.online || net.value.syncing))
</script>

<template>
  <Transition name="fade">
    <div v-if="show" class="ob" :class="{ sync: net.online }" role="status">
      <RefreshCw v-if="net.online" :size="13" class="spin" />
      <CloudOff v-else :size="13" />
      <span v-if="net.online">{{ t('offline.syncing', { n: net.pending }) }}</span>
      <span v-else>{{ t('offline.banner') }}<template v-if="net.pending"> · {{ t('offline.pending', { n: net.pending }) }}</template></span>
    </div>
  </Transition>
</template>

<style scoped>
.ob { display: flex; align-items: center; justify-content: center; gap: 8px; height: 28px; padding: 0 12px; font-size: var(--fs-sm); font-weight: 500; background: color-mix(in srgb, var(--orange, #e2a03f) 18%, var(--bg)); color: var(--text); border-bottom: 1px solid var(--border); }
.ob.sync { background: var(--accent-soft); }
.spin { animation: ob-spin 1s linear infinite; }
@keyframes ob-spin { to { transform: rotate(360deg); } }
</style>
