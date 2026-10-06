<script setup lang="ts">
import { Users, CalendarDays, CheckCircle2, Circle, Plus, ListTodo } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useAccess } from '~/composables/useAccess'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { useLootyCalendars, useCalendarInvalidate, type CalEvent, type EventDetail } from '~/composables/useCalendar'

/**
 * Planification d'un événement Looty (détail) : membres mentionnés, calendrier (déplacer),
 * tâches liées et ajout rapide d'une tâche pour cette occurrence (échéance = début).
 */
const props = defineProps<{ event: CalEvent; canWrite: boolean }>()
const { wsId, members, boards, member } = useWorkspace()
const { can } = useAccess(wsId)
const { t, formatDate } = useI18n()
const qc = useQueryClient()
const invalidate = useCalendarInvalidate()
const { data: calendars } = useLootyCalendars(wsId)

const key = computed(() => ['ws', wsId.value, 'event', props.event.event_id])
const { data: detail } = useQuery({
  queryKey: key,
  queryFn: () => api<EventDetail>(`/api/workspaces/${wsId.value}/events/${props.event.event_id}`),
  enabled: computed(() => !!props.event.event_id),
})
const refresh = () => { qc.invalidateQueries({ queryKey: key.value }); invalidate(wsId.value) }

// Membres mentionnés
const tagged = computed(() => detail.value?.members ?? props.event.members ?? [])
const memberItems = computed(() => (members.data.value ?? []).map((m) => ({ value: m.user_id, label: m.name, hint: m.email })))
const memberOpen = ref(false)
async function setMembers(ids: string[]) {
  await api(`/api/workspaces/${wsId.value}/events/${props.event.event_id}/members`, { method: 'PUT', body: { members: ids } })
  refresh()
}

// Calendrier
const calItems = computed(() => (calendars.value ?? []).map((c) => ({ value: c.id, label: c.name, hint: c.board_name ?? undefined })))
const calOpen = ref(false)
const currentCal = computed(() => calendars.value?.find((c) => c.id === (detail.value?.calendar_id ?? props.event.calendar_id)))
async function move(id: string | null) {
  if (!id || id === currentCal.value?.id) return
  await api(`/api/workspaces/${wsId.value}/events/${props.event.event_id}/calendar`, { method: 'PUT', body: { calendar_id: id } })
  toast.success(t('planning.moved', { name: calendars.value?.find((c) => c.id === id)?.name ?? '' }))
  refresh()
}

// Tâches liées : celles de cette occurrence d'abord
const occurrence = computed(() => props.event.starts_at)
const tasks = computed(() => {
  const all = detail.value?.tasks ?? []
  const same = (d?: string | null) => !!d && Math.abs(+new Date(d) - +new Date(occurrence.value)) < 60_000
  return [...all].sort((a, b) => Number(same(b.event_start)) - Number(same(a.event_start)))
})
const isThis = (d?: string | null) => !!d && Math.abs(+new Date(d) - +new Date(occurrence.value)) < 60_000
const adding = ref(false)
const newTitle = ref('')
// Tableau par défaut : celui du calendrier de l'événement, sinon le premier.
const boardId = ref<string | null>(null)
watch([currentCal, boards.data], () => { if (!boardId.value) boardId.value = currentCal.value?.board_id ?? boards.data.value?.[0]?.id ?? null }, { immediate: true })
const boardItems = computed(() => (boards.data.value ?? []).map((b) => ({ value: b.id, label: b.name })))
const boardOpen = ref(false)
async function addTask() {
  const title = newTitle.value.trim()
  if (!title || !boardId.value) return
  try {
    const board = await api<{ columns: { id: string; position: number }[] }>(`/api/boards/${boardId.value}`)
    const col = [...board.columns].sort((a, b) => a.position - b.position)[0]
    if (!col) return
    await api(`/api/boards/columns/${col.id}/tasks`, { method: 'POST', body: { title, due_date: props.event.starts_at, event_id: props.event.event_id, event_start: props.event.starts_at } })
    newTitle.value = ''
    adding.value = false
    toast.success(t('planning.taskAdded'))
    refresh()
    qc.invalidateQueries({ queryKey: ['board', boardId.value] })
  } catch (e) { toast.error(errorMessage(e)) }
}
</script>

<template>
  <div class="ep">
    <div class="ep-row">
      <Users :size="13" class="faint" />
      <div class="ep-avatars">
        <UiTooltip v-for="id in tagged" :key="id" :text="member(id)?.name ?? ''"><UiAvatar :name="member(id)?.name" :src="member(id)?.avatar_url" :size="20" /></UiTooltip>
        <span v-if="!tagged.length" class="faint ep-none">{{ t('planning.noMembers') }}</span>
      </div>
      <UiCombobox v-if="canWrite" v-model:open="memberOpen" multiple :items="memberItems" :model-value="tagged" :width="280" placement="bottom-end" @update:model-value="(v) => setMembers(v as string[])">
        <button class="ep-link">{{ t('planning.tag') }}</button>
      </UiCombobox>
    </div>
    <div v-if="canWrite && calendars && calendars.length > 1" class="ep-row">
      <CalendarDays :size="13" class="faint" />
      <UiCombobox v-model:open="calOpen" :items="calItems" :model-value="currentCal?.id ?? null" :width="260" @update:model-value="(v) => move(v as string)">
        <button class="ep-select"><i class="ep-dot" :style="{ background: currentCal?.color }" />{{ currentCal?.name }} · <span class="ep-link">{{ t('planning.move') }}</span></button>
      </UiCombobox>
    </div>
    <div class="ep-tasks">
      <div class="ep-label"><ListTodo :size="13" /> {{ t('planning.tasks') }}</div>
      <NuxtLink v-for="tk in tasks" :key="tk.id" :to="links.task(wsId, tk.board_id, tk.id)" class="ep-task" :class="{ other: !isThis(tk.event_start) }">
        <component :is="tk.is_completed ? CheckCircle2 : Circle" :size="13" :class="tk.is_completed ? 'ep-done' : 'faint'" />
        <span class="truncate">{{ tk.title }}</span>
        <span class="ep-meta">{{ isThis(tk.event_start) ? tk.board_name : formatDate(tk.event_start ?? tk.due_date ?? '') }}</span>
      </NuxtLink>
      <p v-if="!tasks.length && !adding" class="faint ep-none">{{ t('planning.noTasks') }}</p>
      <template v-if="can.editTasks">
        <form v-if="adding" class="ep-add" @submit.prevent="addTask">
          <input v-model="newTitle" :placeholder="t('planning.taskPlaceholder')" autofocus @keydown.esc="adding = false" />
          <UiCombobox v-model:open="boardOpen" v-model="boardId" :items="boardItems" :width="220" placement="bottom-end">
            <button type="button" class="ep-select small">{{ boards.data.value?.find((b) => b.id === boardId)?.name }}</button>
          </UiCombobox>
          <UiButton size="xs" variant="primary" type="submit" :disabled="!newTitle.trim()">{{ t('common.add') }}</UiButton>
        </form>
        <button v-else class="ep-link ep-addbtn" @click="adding = true"><Plus :size="12" /> {{ t('planning.addTask') }}</button>
      </template>
    </div>
  </div>
</template>

<style scoped>
.ep { display: flex; flex-direction: column; gap: 8px; margin-top: 6px; padding-top: 8px; border-top: 1px solid var(--border); }
.ep-row { display: flex; align-items: center; gap: 8px; font-size: var(--fs-sm); }
.ep-avatars { flex: 1; display: flex; gap: 2px; flex-wrap: wrap; align-items: center; }
.ep-none { font-size: var(--fs-sm); margin: 0; }
.ep-link { color: var(--accent); font-size: var(--fs-sm); display: inline-flex; align-items: center; gap: 4px; }
.ep-select { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); }
.ep-select.small { max-width: 110px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 0 6px; height: 24px; border-radius: var(--r-sm); box-shadow: inset 0 0 0 1px var(--border-strong); }
.ep-dot { width: 8px; height: 8px; border-radius: 50%; }
.ep-tasks { display: flex; flex-direction: column; gap: 2px; }
.ep-label { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-2); margin-bottom: 2px; }
.ep-task { display: flex; align-items: center; gap: 6px; padding: 3px 4px; border-radius: var(--r-sm); font-size: var(--fs-sm); color: var(--text); }
.ep-task:hover { background: var(--bg-hover); }
.ep-task.other { opacity: .65; }
.ep-meta { margin-left: auto; font-size: 11px; color: var(--text-3); white-space: nowrap; }
.ep-done { color: var(--green); }
.ep-add { display: flex; gap: 6px; align-items: center; }
.ep-add input { flex: 1; min-width: 0; height: 26px; padding: 0 8px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-sm); outline: none; }
.ep-addbtn { align-self: flex-start; margin-top: 2px; }
</style>
