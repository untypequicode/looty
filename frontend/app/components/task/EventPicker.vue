<script setup lang="ts">
import { CalendarDays, Search, Repeat } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { useCalendarEvents, useLootyCalendars, startOfDay, addDays, type CalEvent } from '~/composables/useCalendar'

/**
 * Choix d'une occurrence d'événement de l'espace pour une tâche (ex. « à rendre au TD de
 * jeudi ») : prochains 60 jours, recherche par titre.
 */
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ pick: [e: CalEvent | null] }>()
defineProps<{ linked?: boolean }>()
const { wsId } = useWorkspace()
const { t, formatDate } = useI18n()
const from = ref(addDays(startOfDay(new Date()), -7))
const to = computed(() => addDays(from.value, 67))
const { data } = useCalendarEvents(wsId, from, to)
const { data: cals } = useLootyCalendars(wsId)
const q = ref('')
const color = (e: CalEvent) => cals.value?.find((c) => c.id === e.calendar_id)?.color ?? 'var(--accent)'
const list = computed(() => {
  const s = q.value.trim().toLowerCase()
  const now = Date.now() - 3600_000
  return (data.value?.events ?? [])
    .filter((e) => e.looty && (!s || e.summary.toLowerCase().includes(s)) && (s || +new Date(e.ends_at) >= now))
    .sort((a, b) => +new Date(a.starts_at) - +new Date(b.starts_at))
    .slice(0, 40)
})
const when = (e: CalEvent) => e.all_day ? formatDate(e.starts_at) : `${formatDate(e.starts_at, { weekday: 'short', day: 'numeric', month: 'short' })} ${new Date(e.starts_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
function pick(e: CalEvent | null) { emit('pick', e); open.value = false }
</script>

<template>
  <UiPopover v-model:open="open" :width="340">
    <template v-if="$slots.default" #trigger><slot /></template>
    <div class="evp">
      <label class="evp-search"><Search :size="13" class="faint" /><input v-model="q" :placeholder="t('planning.searchEvent')" autofocus /></label>
      <div class="evp-list">
        <button v-for="e in list" :key="e.id" class="evp-item" @click="pick(e)">
          <i class="evp-bar" :style="{ background: color(e) }" />
          <span class="evp-title truncate">{{ e.summary || t('calendar.untitled') }}</span>
          <Repeat v-if="e.recurring" :size="11" class="faint" />
          <span class="evp-when">{{ when(e) }}</span>
        </button>
        <p v-if="!list.length" class="evp-empty"><CalendarDays :size="16" /> {{ t('planning.noEvents') }}</p>
      </div>
      <button v-if="linked" class="evp-clear" @click="pick(null)">{{ t('planning.unlink') }}</button>
    </div>
  </UiPopover>
</template>

<style scoped>
.evp { padding: 6px; display: flex; flex-direction: column; gap: 6px; }
.evp-search { display: flex; align-items: center; gap: 6px; padding: 0 8px; height: 30px; border-radius: var(--r-sm); background: var(--bg-input); box-shadow: inset 0 0 0 1px var(--border-strong); }
.evp-search input { flex: 1; border: none; outline: none; background: transparent; font: inherit; font-size: var(--fs-sm); color: var(--text); }
.evp-list { max-height: 300px; overflow-y: auto; display: flex; flex-direction: column; gap: 1px; }
.evp-item { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; font-size: var(--fs-sm); }
.evp-item:hover { background: var(--bg-hover); }
.evp-bar { width: 3px; height: 18px; border-radius: 2px; flex-shrink: 0; }
.evp-title { flex: 1; min-width: 0; }
.evp-when { color: var(--text-3); font-size: var(--fs-xs); white-space: nowrap; text-transform: capitalize; }
.evp-empty { display: flex; align-items: center; gap: 6px; color: var(--text-3); font-size: var(--fs-sm); padding: 12px 8px; margin: 0; }
.evp-clear { height: 28px; border-radius: var(--r-sm); color: var(--text-2); font-size: var(--fs-sm); }
.evp-clear:hover { background: var(--bg-hover); color: var(--red); }
</style>
