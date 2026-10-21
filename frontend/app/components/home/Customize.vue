<script setup lang="ts">
import { Eye, EyeOff, RotateCcw } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useDashboard, WIDGETS, type WidgetId } from '~/composables/useDashboard'
import { useI18n } from '~/composables/useI18n'

/** Widgets de l'accueil : afficher ou masquer chacun, ou revenir à la disposition par défaut. Tout s'enregistre aussitôt. */
const open = defineModel<boolean>('open', { default: false })
const { wsId } = useWorkspace()
const { t } = useI18n()
const { has, add, remove, reset } = useDashboard(wsId)
const toggle = (id: WidgetId) => (has(id) ? remove(id) : add(id))
</script>

<template>
  <UiDialog v-model:open="open" :title="t('dashboard.title')" :description="t('dashboard.hint')" :width="520">
    <div class="hc">
      <div v-for="w in WIDGETS" :key="w.id" class="hc-row" :class="{ off: !has(w.id) }">
        <button class="hc-toggle" :aria-pressed="has(w.id)" :aria-label="has(w.id) ? t('sidebarLayout.hide') : t('sidebarLayout.show')" @click="toggle(w.id)"><component :is="has(w.id) ? Eye : EyeOff" :size="15" /></button>
        <span class="hc-text"><b>{{ t(`dashboard.widgets.${w.id}`) }}</b><small>{{ t(`dashboard.widgetHints.${w.id}`) }}</small></span>
      </div>
    </div>
    <template #footer>
      <UiButton variant="ghost" :icon="RotateCcw" class="hc-reset" @click="reset()">{{ t('sidebarLayout.reset') }}</UiButton>
      <UiButton variant="primary" @click="open = false">{{ t('common.close') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.hc { display: flex; flex-direction: column; gap: 2px; max-height: 60vh; overflow-y: auto; }
.hc-row { display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 4px; border-radius: var(--r-sm); }
.hc-row:hover { background: var(--bg-hover); }
.hc-row.off .hc-text { opacity: .5; }
.hc-text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.hc-text small { color: var(--text-3); font-size: var(--fs-sm); }
.hc-toggle { width: 28px; height: 28px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); flex-shrink: 0; }
.hc-toggle[aria-pressed='true'] { color: var(--accent); }
.hc-toggle:hover { background: var(--bg-active); color: var(--text); }
.hc-reset { margin-right: auto; }
</style>
