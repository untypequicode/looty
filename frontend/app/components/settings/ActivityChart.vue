<script setup lang="ts">
import { Table2, LineChart } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'

/**
 * Activité quotidienne (tâches et pages créées) : graphique en lignes à deux séries.
 * Couleurs validées (CVD / contraste) pour chaque thème ; série 2 en pointillés
 * et étiquetée directement pour ne jamais dépendre de la seule couleur.
 */
const props = defineProps<{ data: { day: string; tasks: number; pages: number }[] }>()
const { t, formatDate } = useI18n()

const W = 640
const H = 200
const pad = { l: 32, r: 64, t: 12, b: 26 }
const view = ref<'chart' | 'table'>('chart')
const hover = ref<number | null>(null)

const max = computed(() => {
  const m = Math.max(1, ...props.data.flatMap((d) => [d.tasks, d.pages]))
  const step = m <= 5 ? 1 : m <= 20 ? 5 : 10
  return Math.ceil(m / step) * step
})
const x = (i: number) => pad.l + (i / Math.max(1, props.data.length - 1)) * (W - pad.l - pad.r)
const y = (v: number) => pad.t + (1 - v / max.value) * (H - pad.t - pad.b)
const path = (key: 'tasks' | 'pages') => props.data.map((d, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(d[key]).toFixed(1)}`).join(' ')
const ticks = computed(() => [0, max.value / 2, max.value])
const xTicks = computed(() => props.data.map((d, i) => ({ i, d })).filter(({ i }) => i % 7 === 0 || (i === props.data.length - 1 && i % 7 >= 4)))
const last = computed(() => props.data[props.data.length - 1])
const series = computed(() => [
  { key: 'tasks' as const, label: t('settings.admin.tasksCreated'), cls: 's1' },
  { key: 'pages' as const, label: t('settings.admin.pagesCreated'), cls: 's2' },
])

function onMove(e: PointerEvent) {
  const svg = e.currentTarget as SVGSVGElement
  const r = svg.getBoundingClientRect()
  const px = ((e.clientX - r.left) / r.width) * W
  const i = Math.round(((px - pad.l) / (W - pad.l - pad.r)) * (props.data.length - 1))
  hover.value = i >= 0 && i < props.data.length ? i : null
}
const tip = computed(() => (hover.value == null ? null : { ...props.data[hover.value], left: Math.min(84, Math.max(16, (x(hover.value) / W) * 100)) }))
</script>

<template>
  <div class="viz">
    <div class="viz-head">
      <div class="legend">
        <span v-for="s in series" :key="s.key" class="lg"><svg width="18" height="8"><line x1="0" y1="4" x2="18" y2="4" :class="['ln', s.cls]" /></svg>{{ s.label }}</span>
      </div>
      <UiSegmented v-model="view" :options="[{ value: 'chart', icon: LineChart, title: t('settings.admin.chart') }, { value: 'table', icon: Table2, title: t('settings.admin.table') }]" />
    </div>

    <div v-if="view === 'chart'" class="plot">
      <svg :viewBox="`0 0 ${W} ${H}`" role="img" :aria-label="t('settings.admin.activity')" @pointermove="onMove" @pointerleave="hover = null">
        <g class="grid">
          <line v-for="tk in ticks" :key="tk" :x1="pad.l" :x2="W - pad.r" :y1="y(tk)" :y2="y(tk)" />
          <text v-for="tk in ticks" :key="`l${tk}`" :x="pad.l - 8" :y="y(tk) + 3" text-anchor="end">{{ tk }}</text>
          <text v-for="xt in xTicks" :key="xt.i" :x="x(xt.i)" :y="H - 6" text-anchor="middle">{{ formatDate(xt.d.day, { day: 'numeric', month: 'short' }) }}</text>
        </g>
        <line v-if="hover != null" class="cross" :x1="x(hover)" :x2="x(hover)" :y1="pad.t" :y2="H - pad.b" />
        <path v-for="s in series" :key="s.key" :d="path(s.key)" :class="['ln', s.cls]" fill="none" />
        <template v-if="hover != null">
          <circle v-for="s in series" :key="`c${s.key}`" :cx="x(hover)" :cy="y(data[hover][s.key])" r="4" :class="['dot', s.cls]" />
        </template>
        <template v-if="last">
          <text v-for="(s, i) in series" :key="`d${s.key}`" class="direct" :x="W - pad.r + 8" :y="y(last[s.key]) + 4 + (i && Math.abs(y(last.tasks) - y(last.pages)) < 12 ? 12 : 0)">{{ last[s.key] }} {{ s.key === 'tasks' ? t('settings.admin.tasksShort') : t('settings.admin.pagesShort') }}</text>
        </template>
      </svg>
      <div v-if="tip" class="tip" :style="{ left: `${tip.left}%` }">
        <div class="tip-date">{{ formatDate(tip.day, { weekday: 'short', day: 'numeric', month: 'short' }) }}</div>
        <div v-for="s in series" :key="s.key" class="tip-row"><svg width="14" height="8"><line x1="0" y1="4" x2="14" y2="4" :class="['ln', s.cls]" /></svg>{{ s.label }} <strong>{{ tip[s.key] }}</strong></div>
      </div>
    </div>

    <div v-else class="table-wrap">
      <table>
        <thead><tr><th>{{ t('settings.admin.day') }}</th><th>{{ series[0].label }}</th><th>{{ series[1].label }}</th></tr></thead>
        <tbody><tr v-for="d in [...data].reverse()" :key="d.day"><td>{{ formatDate(d.day) }}</td><td class="tabular">{{ d.tasks }}</td><td class="tabular">{{ d.pages }}</td></tr></tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.viz { --series-1: #5e6ad2; --series-2: #26a46b; padding: 14px 16px; }
:global(html:not(.light)) .viz { --series-1: #7170ff; --series-2: #2f9e6a; }
.viz-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 8px; }
.legend { display: flex; gap: 16px; flex-wrap: wrap; }
.lg { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); }
.plot { position: relative; }
.plot > svg { width: 100%; height: auto; display: block; overflow: visible; touch-action: none; }
.lg svg, .tip-row svg { flex: none; }
.lg { white-space: nowrap; }
.grid line { stroke: var(--border); stroke-width: 1; }
.grid text { fill: var(--text-3); font-size: 10px; font-variant-numeric: tabular-nums; }
.ln { stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; fill: none; }
.ln.s1 { stroke: var(--series-1); }
.ln.s2 { stroke: var(--series-2); stroke-dasharray: 5 4; }
.dot { stroke: var(--bg-panel); stroke-width: 2; }
.dot.s1 { fill: var(--series-1); }
.dot.s2 { fill: var(--series-2); }
.cross { stroke: var(--text-4); stroke-width: 1; }
.direct { fill: var(--text-2); font-size: 11px; font-weight: 500; }
.tip {
  position: absolute; top: 0; transform: translateX(-50%); pointer-events: none; padding: 8px 10px; border-radius: var(--r-md);
  background: var(--bg-elevated); box-shadow: var(--shadow-lg); font-size: var(--fs-sm); white-space: nowrap; z-index: 2;
}
.tip-date { color: var(--text-3); margin-bottom: 4px; text-transform: capitalize; }
.tip-row { display: flex; align-items: center; gap: 6px; color: var(--text-2); }
.tip-row strong { color: var(--text); margin-left: auto; padding-left: 12px; }
.table-wrap { max-height: 260px; overflow-y: auto; }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-sm); }
th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--border); }
th { color: var(--text-3); font-weight: 500; position: sticky; top: 0; background: var(--bg-panel); }
</style>
