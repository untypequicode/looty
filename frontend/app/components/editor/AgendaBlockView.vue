<script setup lang="ts">
import { NodeViewWrapper, nodeViewProps } from '@tiptap/vue-3'
import { CalendarDays, CheckCircle2, Circle, GripVertical } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { CalEvent, CalTask } from '~/composables/useCalendar'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useAccess } from '~/composables/useAccess'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { isTimed, timeOf } from '~/utils/dates'

/**
 * Bloc « agenda » d'un document : événements de l'espace et échéances de mes tâches sur
 * les prochains jours, groupés par jour. Une échéance se coche depuis le bloc.
 */
const props = defineProps(nodeViewProps)
// En lecture : choix de vue et de filtre gardés localement (la page n'est pas modifiée).
const local = ref<Record<string, unknown>>({})
const attr = (k: string) => local.value[k] ?? props.node.attrs[k]
const { t, formatDate } = useI18n()
const { wsId } = useWorkspace()
const { allows } = useAccess(wsId)
const qc = useQueryClient()
const days = computed(() => Number(attr('days')) || 7)
const scope = computed(() => (attr('scope') as 'all' | 'events' | 'tasks') ?? 'all')
// `isEditable` n'est pas réactif : relu à chaque mise à jour de l'éditeur.
const editable = ref(props.editor.isEditable)
const syncEditable = () => { editable.value = props.editor.isEditable }
props.editor.on('update', syncEditable)
props.editor.on('transaction', syncEditable)
onBeforeUnmount(() => { props.editor.off('update', syncEditable); props.editor.off('transaction', syncEditable) })
const from = computed(() => { const d = new Date(); d.setHours(0, 0, 0, 0); return d })
const to = computed(() => new Date(from.value.getTime() + days.value * 86400_000))
const { data, isLoading } = useQuery({
  queryKey: computed(() => ['ws', wsId.value, 'calendar-events', 'agenda-block', days.value]),
  queryFn: () => api<{ events: CalEvent[]; tasks: CalTask[] }>(`/api/workspaces/${wsId.value}/calendars/events?from=${encodeURIComponent(from.value.toISOString())}&to=${encodeURIComponent(to.value.toISOString())}`),
  staleTime: 30_000,
})

interface Row { key: string; kind: 'event' | 'task'; when: string; title: string; sub: string; done?: boolean; to: string; id: string }
const dayKey = (d: Date) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`
const groups = computed(() => {
  const rows: Row[] = []
  if (scope.value !== 'tasks') {
    for (const e of data.value?.events ?? []) {
      rows.push({
        key: `e${e.id}${e.starts_at}`, kind: 'event', when: e.starts_at, title: e.summary,
        sub: e.all_day ? t('blocks.allDay') : `${timeOf(e.starts_at)}–${timeOf(e.ends_at)}`,
        to: e.event_id ? `${links.calendar(wsId.value)}?event=${e.event_id}` : links.calendar(wsId.value), id: e.id,
      })
    }
  }
  if (scope.value !== 'events') {
    for (const k of data.value?.tasks ?? []) {
      rows.push({ key: `t${k.id}`, kind: 'task', when: k.due_date, title: k.title, sub: `${k.board_name}${isTimed(k.due_date) ? ` · ${timeOf(k.due_date)}` : ''}`, done: k.is_completed, to: links.task(wsId.value, k.board_id, k.id), id: k.id })
    }
  }
  rows.sort((a, b) => a.when.localeCompare(b.when))
  const out: { key: string; label: string; today: boolean; rows: Row[] }[] = []
  const today = dayKey(new Date())
  for (const r of rows) {
    const d = new Date(r.when)
    const k = dayKey(d)
    let g = out.find((x) => x.key === k)
    if (!g) { g = { key: k, label: formatDate(d, { weekday: 'long', day: 'numeric', month: 'long' }), today: k === today, rows: [] }; out.push(g) }
    g.rows.push(r)
  }
  return out
})

async function toggle(r: Row) {
  if (!editable.value || !allows('boards', 'validate')) return
  await api(`/api/boards/tasks/${r.id}/toggle_validation`, { method: 'PUT' })
  qc.invalidateQueries({ queryKey: ['ws', wsId.value, 'calendar-events'] })
}
function set(attrs: Record<string, unknown>) {
  if (editable.value) props.updateAttributes(attrs)
  else local.value = { ...local.value, ...attrs }
}
</script>

<template>
  <NodeViewWrapper class="ag" :class="{ selected }">
    <div class="ag-card" contenteditable="false">
      <header class="ag-head">
        <span v-if="editable" class="ag-grip" data-drag-handle><GripVertical :size="14" /></span>
        <NuxtLink :to="links.calendar(wsId)" class="ag-name"><CalendarDays :size="15" /> {{ t('blocks.agenda') }}</NuxtLink>
        <span class="ag-space" />
        <UiSegmented :model-value="scope" :options="[{ value: 'all', label: t('blocks.all') }, { value: 'events', label: t('blocks.events') }, { value: 'tasks', label: t('blocks.deadlines') }]" @update:model-value="(v) => set({ scope: v })" />
        <UiSegmented :model-value="String(days)" :options="[{ value: '3', label: t('blocks.days', { count: 3 }) }, { value: '7', label: t('blocks.days', { count: 7 }) }, { value: '14', label: t('blocks.days', { count: 14 }) }]" @update:model-value="(v) => set({ days: Number(v) })" />
      </header>
      <div v-if="isLoading" class="ag-empty"><UiSpinner /></div>
      <p v-else-if="!groups.length" class="ag-empty">{{ t('blocks.nothingSoon') }}</p>
      <div v-else class="ag-body">
        <section v-for="g in groups" :key="g.key" class="ag-day">
          <h4 :class="{ today: g.today }">{{ g.today ? t('blocks.today') : g.label }}</h4>
          <NuxtLink v-for="r in g.rows" :key="r.key" :to="r.to" class="ag-row" :class="[r.kind, { done: r.done }]">
            <button v-if="r.kind === 'task'" class="ag-check" :aria-label="t('task.markDone')" @click.prevent.stop="toggle(r)"><component :is="r.done ? CheckCircle2 : Circle" :size="15" /></button>
            <span v-else class="ag-dot" />
            <span class="ag-title truncate">{{ r.title }}</span>
            <span class="ag-sub truncate">{{ r.sub }}</span>
          </NuxtLink>
        </section>
      </div>
    </div>
  </NodeViewWrapper>
</template>

<style scoped>
.ag { margin: 1.2em 0; }
.ag-card { border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); overflow: hidden; }
.ag.selected .ag-card { box-shadow: 0 0 0 1.5px var(--border-focus); }
.ag-head { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-bottom: 1px solid var(--border); flex-wrap: wrap; }
.ag-grip { color: var(--text-3); cursor: grab; display: grid; place-items: center; }
.ag-name { display: inline-flex; align-items: center; gap: 6px; font-weight: 600; color: var(--text); text-decoration: none; }
.ag-space { flex: 1; }
.ag-body { padding: 6px 12px 12px; max-height: 440px; overflow: auto; }
.ag-day h4 { margin: 10px 0 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-3); text-transform: capitalize; }
.ag-day h4.today { color: var(--accent-text); }
.ag-row { display: grid; grid-template-columns: 18px minmax(0, 1fr) auto; align-items: center; gap: 8px; padding: 5px 6px; border-radius: var(--r-md); color: var(--text); text-decoration: none; }
.ag-row:hover { background: var(--bg-hover); }
.ag-row.done .ag-title { text-decoration: line-through; color: var(--text-3); }
.ag-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--orange, #e2803f); justify-self: center; }
.ag-check { display: grid; place-items: center; color: var(--text-3); }
.ag-row.done .ag-check { color: var(--green); }
.ag-title { font-weight: 500; }
.ag-sub { color: var(--text-3); font-size: var(--fs-sm); }
.ag-empty { padding: 20px; color: var(--text-3); text-align: center; display: grid; place-items: center; }
</style>
