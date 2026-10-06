<script setup lang="ts">
import { ArrowUp, ArrowDown, Eye, EyeOff, Columns2, RectangleHorizontal, RotateCcw } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useDashboard, WIDGETS, DEFAULT_LAYOUT, type WidgetConf, type WidgetId } from '~/composables/useDashboard'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

/** Personnalisation de l'accueil : widgets affichés, ordre, largeur. Enregistrement explicite. */
const open = defineModel<boolean>('open', { default: false })
const { wsId } = useWorkspace()
const { t } = useI18n()
const { layout, saved, save } = useDashboard(wsId)
interface Row extends WidgetConf { on: boolean }
const rows = ref<Row[]>([])
function load(from: WidgetConf[]) {
  const on = from.map((w) => ({ ...w, on: true }))
  const off = WIDGETS.filter((w) => !from.some((x) => x.id === w.id)).map((w) => ({ id: w.id, size: w.size, on: false }))
  rows.value = [...on, ...off]
}
watch(open, (o) => { if (o) load(layout.value) })
function move(i: number, d: number) {
  const j = i + d
  if (j < 0 || j >= rows.value.length) return
  ;[rows.value[i], rows.value[j]] = [rows.value[j]!, rows.value[i]!]
}
const fixed = (id: WidgetId) => !!WIDGETS.find((w) => w.id === id)?.fixedSize
const saving = ref(false)
async function apply() {
  saving.value = true
  try {
    await save(rows.value.filter((r) => r.on).map(({ id, size }) => ({ id, size })))
    toast.success(t('dashboard.saved'))
    open.value = false
  } finally { saving.value = false }
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('dashboard.title')" :description="saved ? t('dashboard.hint') : t('dashboard.firstHint')" :width="520" :closable="saved">
    <div class="hc">
      <div v-for="(r, i) in rows" :key="r.id" class="hc-row" :class="{ off: !r.on }">
        <button class="hc-toggle" :aria-pressed="r.on" :aria-label="r.on ? t('sidebarLayout.hide') : t('sidebarLayout.show')" @click="r.on = !r.on"><component :is="r.on ? Eye : EyeOff" :size="15" /></button>
        <span class="hc-text"><b>{{ t(`dashboard.widgets.${r.id}`) }}</b><small>{{ t(`dashboard.widgetHints.${r.id}`) }}</small></span>
        <button v-if="!fixed(r.id)" class="hc-btn" :title="r.size === 'full' ? t('dashboard.half') : t('dashboard.full')" @click="r.size = r.size === 'full' ? 'half' : 'full'">
          <component :is="r.size === 'full' ? RectangleHorizontal : Columns2" :size="14" />
        </button>
        <button class="hc-btn" :disabled="i === 0" :aria-label="t('sidebarLayout.up')" @click="move(i, -1)"><ArrowUp :size="14" /></button>
        <button class="hc-btn" :disabled="i === rows.length - 1" :aria-label="t('sidebarLayout.down')" @click="move(i, 1)"><ArrowDown :size="14" /></button>
      </div>
    </div>
    <template #footer>
      <UiButton variant="ghost" :icon="RotateCcw" class="hc-reset" @click="load(DEFAULT_LAYOUT)">{{ t('sidebarLayout.reset') }}</UiButton>
      <UiButton v-if="saved" variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="saving" @click="apply">{{ t('dashboard.save') }}</UiButton>
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
.hc-toggle, .hc-btn { width: 28px; height: 28px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); flex-shrink: 0; }
.hc-toggle[aria-pressed='true'] { color: var(--accent); }
.hc-btn:hover:not(:disabled), .hc-toggle:hover { background: var(--bg-active); color: var(--text); }
.hc-btn:disabled { opacity: .3; }
.hc-reset { margin-right: auto; }
</style>
