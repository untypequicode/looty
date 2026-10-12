<script setup lang="ts">
import { X } from 'lucide-vue-next'
import type { Tab } from '~/composables/useTabs'
import { useLocationInfo, locationOfRoute } from '~/composables/usePresenceInfo'
import { useI18n } from '~/composables/useI18n'

const props = defineProps<{ tab: Tab; active: boolean; closable: boolean; /** Affiché dans le volet de droite (écran partagé). */ side?: boolean }>()
const emit = defineEmits<{ activate: []; close: []; menu: [e: MouseEvent] }>()
const router = useRouter()
const { t } = useI18n()
const location = computed(() => {
  const r = router.resolve(props.tab.path)
  return locationOfRoute({ path: r.path, params: r.params as Record<string, any>, query: r.query as Record<string, any> })
})
const info = useLocationInfo(location)
const label = computed(() => info.value?.label ?? t('nav.home'))

function onAux(e: MouseEvent) {
  if (e.button === 1) { e.preventDefault(); emit('close') }
}
</script>

<template>
  <div
    class="tab" :class="{ active, side }" role="tab" :aria-selected="active || side" :title="label" tabindex="0"
    @click="emit('activate')" @keydown.enter="emit('activate')" @mousedown.middle.prevent @auxclick="onAux" @contextmenu.prevent="emit('menu', $event)"
  >
    <span v-if="info && 'emoji' in info && info.emoji" class="tab-emoji">{{ info.emoji }}</span>
    <component :is="info.icon" v-else-if="info" :size="13" class="tab-icon" />
    <span class="tab-label truncate">{{ label }}</span>
    <button v-if="closable" class="tab-close" :aria-label="t('tabs.close')" @click.stop="emit('close')"><X :size="12" /></button>
  </div>
</template>

<style scoped>
.tab {
  position: relative; display: flex; align-items: center; gap: 6px; height: 28px; min-width: 60px; max-width: 200px; flex: 0 1 200px;
  padding: 0 6px 0 10px; border-radius: var(--r-sm); font-size: var(--fs-sm); color: var(--text-3); cursor: default; user-select: none;
}
.tab:hover { background: var(--bg-hover); color: var(--text-2); }
.tab.active { background: var(--bg-elevated); color: var(--text); box-shadow: 0 0 0 1px var(--border), var(--shadow-sm, 0 1px 2px rgba(0,0,0,.06)); }
.tab.side { color: var(--text); background: color-mix(in srgb, var(--accent) 9%, var(--bg-elevated)); box-shadow: inset 0 -2px 0 var(--accent), 0 0 0 1px var(--border); }
.tab-icon { flex: none; }
.tab-emoji { font-size: 12px; flex: none; }
.tab-label { flex: 1; min-width: 0; }
.tab-close { width: 18px; height: 18px; display: grid; place-items: center; border-radius: 4px; color: var(--text-3); opacity: 0; flex: none; }
.tab:hover .tab-close, .tab.active .tab-close { opacity: 1; }
.tab-close:hover { background: var(--bg-active, var(--bg-hover)); color: var(--text); }
</style>
