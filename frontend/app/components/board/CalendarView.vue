<script setup lang="ts">
import { ChevronLeft, ChevronRight, CalendarPlus, CheckCircle2 } from 'lucide-vue-next'
import type { Board, Task } from '~/types'
import { api } from '~/composables/api'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useAccess } from '~/composables/useAccess'
import { useI18n } from '~/composables/useI18n'
import { useCalendarEvents, useLootyCalendars, useCalendarInvalidate, startOfDay, startOfWeek, addDays, sameDay, onDay, type CalEvent } from '~/composables/useCalendar'
import { allTasks } from '~/composables/useBoard'
import { isTimed, timeOf } from '~/utils/dates'

/**
 * Vue calendrier d'un tableau : ses tâches (échéance, heure) et les événements de son
 * calendrier Looty (créé à la demande, couleur au choix).
 */
const props = defineProps<{ board: Board; fullBoard: Board }>()
const emit = defineEmits<{ open: [t: Task] }>()
const { wsId } = useWorkspace()
const { can } = useAccess(wsId)
const { t, dateLocale } = useI18n()
const invalidate = useCalendarInvalidate()
const cursor = ref(new Date())
const first = computed(() => startOfWeek(new Date(cursor.value.getFullYear(), cursor.value.getMonth(), 1)))
const days = computed(() => Array.from({ length: 42 }, (_, i) => addDays(first.value, i)))
const to = computed(() => addDays(first.value, 42))
const { data } = useCalendarEvents(wsId, first, to)
const { data: cals } = useLootyCalendars(wsId)
const cal = computed(() => cals.value?.find((c) => c.board_id === props.fullBoard.id))
const events = computed(() => (data.value?.events ?? []).filter((e) => cal.value && e.calendar_id === cal.value.id))
const tasks = computed(() => allTasks(props.board).filter((tk) => tk.due_date))
const tasksOn = (d: Date) => tasks.value.filter((tk) => {
  const due = new Date(tk.due_date!)
  // Date « jour » (midi UTC) : comparer la date UTC.
  return isTimed(tk.due_date) ? sameDay(due, d) : due.getUTCFullYear() === d.getFullYear() && due.getUTCMonth() === d.getMonth() && due.getUTCDate() === d.getDate()
}).sort((a, b) => +new Date(a.due_date!) - +new Date(b.due_date!))
const monthLabel = computed(() => cursor.value.toLocaleDateString(dateLocale.value, { month: 'long', year: 'numeric' }))
const weekdays = computed(() => Array.from({ length: 7 }, (_, i) => addDays(startOfWeek(new Date()), i).toLocaleDateString(dateLocale.value, { weekday: 'short' })))
const today = startOfDay(new Date())
const time = (e: CalEvent) => (e.all_day ? '' : new Date(e.starts_at).toLocaleTimeString(dateLocale.value, { hour: '2-digit', minute: '2-digit' }))
function move(n: number) { cursor.value = new Date(cursor.value.getFullYear(), cursor.value.getMonth() + n, 1) }
const creating = ref(false)
async function createCalendar() {
  creating.value = true
  try {
    await api(`/api/workspaces/${wsId.value}/planning/calendars`, { method: 'POST', body: { board_id: props.fullBoard.id, color: '#4cb782' } })
    invalidate(wsId.value)
  } finally { creating.value = false }
}
</script>

<template>
  <div class="bc">
    <div class="bc-head">
      <UiButton size="sm" variant="ghost" :icon="ChevronLeft" @click="move(-1)" />
      <UiButton size="sm" @click="cursor = new Date()">{{ t('calendar.today') }}</UiButton>
      <UiButton size="sm" variant="ghost" :icon="ChevronRight" @click="move(1)" />
      <span class="bc-month">{{ monthLabel }}</span>
      <span class="spacer" />
      <NuxtLink v-if="cal" :to="links.calendar(wsId)" class="bc-cal"><i :style="{ background: cal.color }" /> {{ cal.name }}</NuxtLink>
      <UiButton v-else-if="can.editCalendar || can.manageBoards" size="sm" :icon="CalendarPlus" :loading="creating" @click="createCalendar">{{ t('planning.createBoardCalendar') }}</UiButton>
    </div>
    <div class="bc-grid">
      <div v-for="w in weekdays" :key="w" class="bc-wd">{{ w }}</div>
      <div v-for="d in days" :key="+d" class="bc-cell" :class="{ out: d.getMonth() !== cursor.getMonth(), today: sameDay(d, today) }">
        <span class="bc-num">{{ d.getDate() }}</span>
        <NuxtLink v-for="e in events.filter((e) => onDay(e, d))" :key="e.id" :to="`${links.calendar(wsId)}?event=${e.event_id}`" class="bc-ev" :style="{ '--c': cal?.color }">
          <span v-if="time(e)" class="bc-time">{{ time(e) }}</span>{{ e.summary }}
        </NuxtLink>
        <button v-for="tk in tasksOn(d)" :key="tk.id" class="bc-task" :class="{ done: tk.is_completed }" @click="emit('open', tk)">
          <CheckCircle2 v-if="tk.is_completed" :size="11" />
          <span v-if="isTimed(tk.due_date)" class="bc-time">{{ timeOf(tk.due_date) }}</span>
          <span class="truncate">{{ tk.title }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.bc { padding: 12px 20px 24px; display: flex; flex-direction: column; gap: 10px; height: 100%; overflow: auto; }
.bc-head { display: flex; align-items: center; gap: 6px; }
.bc-month { font-weight: 600; text-transform: capitalize; margin-left: 6px; }
.bc-cal { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); }
.bc-cal i { width: 10px; height: 10px; border-radius: 3px; }
.bc-grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); border-top: 1px solid var(--border); border-left: 1px solid var(--border); }
.bc-wd { padding: 6px 8px; font-size: var(--fs-xs); color: var(--text-3); text-transform: uppercase; border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); }
.bc-cell { min-height: 108px; padding: 4px; display: flex; flex-direction: column; gap: 2px; border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); min-width: 0; }
.bc-cell.out { background: var(--bg-panel); }
.bc-cell.out .bc-num { color: var(--text-4); }
.bc-num { font-size: var(--fs-sm); color: var(--text-2); padding: 0 4px; align-self: flex-start; border-radius: 999px; }
.bc-cell.today .bc-num { background: var(--accent); color: #fff; }
.bc-ev { display: block; padding: 1px 6px; border-radius: 4px; font-size: 11.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; background: color-mix(in srgb, var(--c) 20%, var(--bg)); border-left: 3px solid var(--c); color: var(--text); }
.bc-task { display: flex; align-items: center; gap: 4px; padding: 1px 6px; border-radius: 4px; font-size: 11.5px; text-align: left; background: var(--bg-card); box-shadow: var(--shadow-card); color: var(--text); min-width: 0; }
.bc-task.done { opacity: .55; text-decoration: line-through; }
.bc-time { font-variant-numeric: tabular-nums; color: var(--text-3); margin-right: 4px; flex-shrink: 0; }
@media (max-width: 760px) { .bc-cell { min-height: 72px; } .bc-ev, .bc-task { font-size: 10.5px; } }
</style>
