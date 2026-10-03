<script setup lang="ts">
import { CalendarPlus } from 'lucide-vue-next'
import type { Board, Task } from '~/types'
import { columnProgress } from '~/composables/useBoard'
import { useI18n } from '~/composables/useI18n'
import { useBreakpoint } from '~/composables/useBreakpoint'

const props = defineProps<{ board: Board; fullBoard: Board; canWrite: boolean }>()
const emit = defineEmits<{
  (e: 'open', t: Task): void
  (e: 'reschedule', t: Task, start: string | null, due: string): void
}>()
const { t, dateLocale } = useI18n()

const { isMobile } = useBreakpoint()
const nameW = computed(() => (isMobile.value ? 150 : 260))
const zoom = ref<'day' | 'week'>('day')
const dayW = computed(() => (zoom.value === 'day' ? 36 : 14))
const DAY = 86400000
const startOfDay = (d: Date | string | number) => { const x = new Date(d); return new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime() }
const today = startOfDay(Date.now())

const tasks = computed(() => props.board.columns.flatMap((c) => c.tasks.map((tk) => ({ tk, col: c }))))
const scheduled = computed(() => tasks.value.filter(({ tk }) => tk.due_date || tk.start_date))
const unscheduled = computed(() => tasks.value.filter(({ tk }) => !tk.due_date && !tk.start_date))

const range = computed(() => {
  let min = today - 14 * DAY
  let max = today + 45 * DAY
  for (const { tk } of scheduled.value) {
    const s = startOfDay(tk.start_date ?? tk.due_date!)
    const e = startOfDay(tk.due_date ?? tk.start_date!)
    min = Math.min(min, s - 7 * DAY)
    max = Math.max(max, e + 14 * DAY)
  }
  return { start: min, days: Math.round((max - min) / DAY) + 1 }
})
const days = computed(() => Array.from({ length: range.value.days }, (_, i) => new Date(range.value.start + i * DAY)))
const months = computed(() => {
  const out: { label: string; span: number }[] = []
  for (const d of days.value) {
    const label = d.toLocaleDateString(dateLocale.value, { month: 'long', year: 'numeric' })
    if (out.at(-1)?.label === label) out.at(-1)!.span++
    else out.push({ label, span: 1 })
  }
  return out
})

// --- Glisser / redimensionner ---
const drag = ref<{ id: string; mode: 'move' | 'start' | 'end'; x0: number; s0: number; e0: number; ds: number; de: number } | null>(null)

function barGeom(tk: Task) {
  let s = startOfDay(tk.start_date ?? tk.due_date!)
  let e = startOfDay(tk.due_date ?? tk.start_date!)
  if (drag.value?.id === tk.id) { s += drag.value.ds * DAY; e += drag.value.de * DAY }
  if (e < s) [s, e] = [e, s]
  return { left: ((s - range.value.start) / DAY) * dayW.value, width: ((e - s) / DAY + 1) * dayW.value }
}

function onDown(e: PointerEvent, tk: Task, mode: 'move' | 'start' | 'end') {
  if (!props.canWrite) return
  e.stopPropagation()
  ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  drag.value = { id: tk.id, mode, x0: e.clientX, s0: 0, e0: 0, ds: 0, de: 0 }
}
function onMove(e: PointerEvent) {
  if (!drag.value) return
  const delta = Math.round((e.clientX - drag.value.x0) / dayW.value)
  if (drag.value.mode === 'move') { drag.value.ds = delta; drag.value.de = delta }
  else if (drag.value.mode === 'start') drag.value.ds = delta
  else drag.value.de = delta
}
function onUp(tk: Task) {
  const d = drag.value
  drag.value = null
  if (!d) return
  if (d.ds === 0 && d.de === 0) return emit('open', tk)
  const toIso = (ms: number) => { const x = new Date(ms); return new Date(Date.UTC(x.getFullYear(), x.getMonth(), x.getDate(), 12)).toISOString() }
  let s = startOfDay(tk.start_date ?? tk.due_date!) + d.ds * DAY
  let en = startOfDay(tk.due_date ?? tk.start_date!) + d.de * DAY
  if (en < s) [s, en] = [en, s]
  emit('reschedule', tk, s === en && !tk.start_date ? null : toIso(s), toIso(en))
}

function scheduleAt(tk: Task, e: MouseEvent) {
  if (!props.canWrite) return
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
  const idx = Math.floor((e.clientX - rect.left) / dayW.value)
  const d = new Date(range.value.start + idx * DAY)
  const iso = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate(), 12)).toISOString()
  emit('reschedule', tk, null, iso)
}

const scroller = ref<HTMLElement | null>(null)
function scrollToToday() {
  if (!scroller.value) return
  scroller.value.scrollLeft = ((today - range.value.start) / DAY) * dayW.value - scroller.value.clientWidth / 3
}
onMounted(() => nextTick(scrollToToday))
watch(zoom, () => nextTick(scrollToToday))
</script>

<template>
  <div class="tl">
    <div class="tl-toolbar">
      <UiSegmented v-model="zoom" :options="[{ value: 'day', label: t('timeline.days') }, { value: 'week', label: t('timeline.weeks') }]" />
      <UiButton size="xs" variant="ghost" @click="scrollToToday">{{ t('date.today') }}</UiButton>
      <span class="faint tl-hint">{{ canWrite ? t('timeline.hint') : '' }}</span>
    </div>
    <div ref="scroller" class="tl-scroll">
      <div class="tl-grid" :style="{ width: `${nameW + days.length * dayW}px` }">
        <!-- En-têtes -->
        <div class="tl-head">
          <div class="tl-corner">{{ t('task.task') }}</div>
          <div class="tl-cal">
            <div class="tl-months">
              <div v-for="m in months" :key="m.label" class="tl-month" :style="{ width: `${m.span * dayW}px` }"><span>{{ m.label }}</span></div>
            </div>
            <div class="tl-days">
              <div
                v-for="d in days"
                :key="d.getTime()"
                class="tl-day"
                :class="{ weekend: d.getDay() === 0 || d.getDay() === 6, today: d.getTime() === today, monday: d.getDay() === 1 }"
                :style="{ width: `${dayW}px` }"
              >
                <span v-if="zoom === 'day' || d.getDay() === 1">{{ d.getDate() }}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- Lignes -->
        <div class="tl-body">
          <div class="tl-today" :style="{ left: `${nameW + ((today - range.start) / DAY) * dayW + dayW / 2}px` }" />
          <div v-for="{ tk, col } in scheduled" :key="tk.id" class="tl-row">
            <button class="tl-name" @click="emit('open', tk)">
              <TaskStatusIcon v-bind="tk.is_completed ? { done: true } : columnProgress(fullBoard, col.id)" :size="14" />
              <span class="truncate">{{ tk.title }}</span>
            </button>
            <div class="tl-lane" :style="{ backgroundSize: `${dayW * 7}px 100%`, backgroundPosition: `${((range.start / DAY + 3) % 7) * -dayW}px 0` }">
              <div
                class="tl-bar"
                :class="{ done: tk.is_completed, dragging: drag?.id === tk.id, editable: canWrite, short: barGeom(tk).width < 90 }"
                :style="{ left: `${barGeom(tk).left}px`, width: `${barGeom(tk).width}px`, '--c': tk.labels[0]?.color ?? 'var(--accent)' }"
                @pointerdown="onDown($event, tk, 'move')"
                @pointermove="onMove"
                @pointerup="onUp(tk)"
              >
                <span v-if="canWrite" class="tl-handle l" @pointerdown="onDown($event, tk, 'start')" />
                <span class="tl-bar-label truncate">{{ tk.title }}</span>
                <span v-if="canWrite" class="tl-handle r" @pointerdown="onDown($event, tk, 'end')" />
              </div>
            </div>
          </div>
          <template v-if="unscheduled.length">
            <div class="tl-section">{{ t('timeline.unscheduled') }} · {{ unscheduled.length }}</div>
            <div v-for="{ tk, col } in unscheduled" :key="tk.id" class="tl-row unscheduled">
              <button class="tl-name" @click="emit('open', tk)">
                <TaskStatusIcon v-bind="columnProgress(fullBoard, col.id)" :size="14" />
                <span class="truncate">{{ tk.title }}</span>
              </button>
              <div class="tl-lane clickable" :title="t('timeline.clickToSchedule')" @click="scheduleAt(tk, $event)">
                <CalendarPlus :size="13" class="tl-plus" />
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tl { flex: 1; min-height: 0; display: flex; flex-direction: column; }
.tl-toolbar { display: flex; align-items: center; gap: 8px; padding: 8px 16px; border-bottom: 1px solid var(--border); flex: none; }
.tl-hint { font-size: var(--fs-sm); margin-left: 8px; }
.tl-scroll { flex: 1; overflow: auto; }
.tl-grid { position: relative; min-height: 100%; }
.tl-head { position: sticky; top: 0; z-index: 3; display: flex; background: var(--bg); border-bottom: 1px solid var(--border); }
.tl-corner {
  position: sticky; left: 0; z-index: 4; width: 260px; flex: none; display: flex; align-items: flex-end;
  padding: 0 16px 8px; background: var(--bg); color: var(--text-3); font-size: var(--fs-sm); border-right: 1px solid var(--border);
}
.tl-cal { flex: 1; }
.tl-months { display: flex; height: 26px; border-bottom: 1px solid var(--border); }
.tl-month { flex: none; padding: 6px 8px; font-size: var(--fs-sm); font-weight: 600; text-transform: capitalize; border-left: 1px solid var(--border); overflow: hidden; white-space: nowrap; }
.tl-month span { position: sticky; left: 268px; }
.tl-days { display: flex; height: 26px; }
.tl-day { flex: none; display: grid; place-items: center; font-size: 11px; color: var(--text-3); font-variant-numeric: tabular-nums; }
.tl-day.weekend { background: var(--bg-column); }
.tl-day.today span { background: var(--accent); color: #fff; border-radius: var(--r-xs); padding: 1px 4px; font-weight: 600; }
.tl-body { position: relative; }
.tl-today { position: absolute; top: 0; bottom: 0; width: 1.5px; background: var(--accent); z-index: 1; opacity: 0.7; pointer-events: none; }
.tl-row { display: flex; height: 38px; border-bottom: 1px solid var(--border); }
.tl-name {
  position: sticky; left: 0; z-index: 2; width: 260px; flex: none; display: flex; align-items: center; gap: 8px;
  padding: 0 16px; background: var(--bg); border-right: 1px solid var(--border); text-align: left; font-weight: 500;
}
.tl-name:hover { background: var(--bg-card-hover); }
.tl-lane {
  position: relative; flex: 1;
  background-image: linear-gradient(90deg, transparent calc(100% * 5 / 7), var(--bg-column) calc(100% * 5 / 7));
}
.tl-lane.clickable { cursor: copy; display: flex; align-items: center; }
.tl-plus { color: var(--text-4); margin-left: 12px; opacity: 0; position: sticky; left: 272px; }
.tl-row:hover .tl-plus { opacity: 1; }
.tl-bar {
  position: absolute; top: 7px; height: 24px; border-radius: 6px; display: flex; align-items: center;
  background: color-mix(in srgb, var(--c) 26%, var(--bg-card)); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 55%, transparent);
  color: var(--text); font-size: var(--fs-sm); font-weight: 500; touch-action: none; user-select: none;
  transition: box-shadow var(--dur-fast);
}
.tl-bar.editable { cursor: grab; }
.tl-bar.dragging { cursor: grabbing; box-shadow: var(--shadow-drag); z-index: 2; }
.tl-bar.done { opacity: 0.5; }
.tl-bar-label { padding: 0 10px; flex: 1; min-width: 0; }
/* Barre trop courte : le titre s'affiche à droite de la barre */
.tl-bar.short .tl-bar-label { position: absolute; left: calc(100% + 6px); padding: 0; overflow: visible; white-space: nowrap; color: var(--text-2); pointer-events: none; }
.tl-handle { width: 8px; align-self: stretch; cursor: ew-resize; flex: none; }
.tl-handle.l { border-radius: 6px 0 0 6px; }
.tl-handle.r { border-radius: 0 6px 6px 0; }
.tl-handle:hover { background: color-mix(in srgb, var(--c) 45%, transparent); }
.tl-section { position: sticky; left: 0; padding: 14px 16px 6px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-2); width: max-content; }
@media (max-width: 639px) {
  .tl-corner, .tl-name { width: 150px; }
  .tl-hint { display: none; }
}
</style>
