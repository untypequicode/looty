<script setup lang="ts">
import { Bell } from 'lucide-vue-next'
import type { TaskReminder } from '~/types'
import { useI18n } from '~/composables/useI18n'
import { useAuth } from '~/composables/useAuth'
import { reminderLabel, REMINDER_PRESETS } from '~/utils/reminders'

/** `null` : rappel par défaut de chaque personne assignée ; `[]` : aucun. */
const props = defineProps<{ reminders?: TaskReminder[] | null; canWrite: boolean }>()
const emit = defineEmits<{ (e: 'update', v: TaskReminder[] | null): void }>()
const { t, formatDate } = useI18n()
const { user } = useAuth()
const open = ref(false)

const DEFAULT = 'default'
const list = computed(() => props.reminders ?? [])
const keyOf = (r: TaskReminder) => (r.before != null ? `b${r.before}` : `a${r.at}`)
const selected = computed<string[]>(() => (props.reminders == null ? [DEFAULT] : list.value.map(keyOf)))

const items = computed(() => [
  { value: DEFAULT, label: t('taskReminder.default'), hint: reminderLabel(user.value?.default_reminder_minutes ?? null, t) },
  ...REMINDER_PRESETS.map((m) => ({ value: `b${m}`, label: reminderLabel(m, t) })),
  ...list.value.filter((r) => r.at).map((r) => ({ value: keyOf(r), label: dateLabel(r.at!) })),
])
function dateLabel(iso: string) {
  const d = new Date(iso)
  return `${formatDate(iso)} · ${d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
}
function onPick(v: string | string[]) {
  const next = (Array.isArray(v) ? v : [v]) as string[]
  const had = selected.value.includes(DEFAULT)
  if (next.includes(DEFAULT) && !had) return emit('update', null)
  const keep = next.filter((k) => k !== DEFAULT)
  emit('update', keep.map((k) => (k[0] === 'b' ? { before: Number(k.slice(1)) } : { at: k.slice(1) })))
}
function addDate(e: Event) {
  const input = e.target as HTMLInputElement
  if (!input.value) return
  const at = new Date(input.value).toISOString()
  input.value = ''
  if (list.value.some((r) => r.at === at) || list.value.length >= 8) return
  emit('update', [...list.value, { at }])
}
const summary = computed(() => {
  if (props.reminders == null) return t('taskReminder.defaultShort', { when: reminderLabel(user.value?.default_reminder_minutes ?? null, t) })
  if (!list.value.length) return t('taskReminder.none')
  return list.value.map((r) => (r.before != null ? reminderLabel(r.before, t) : dateLabel(r.at!))).join(', ')
})
</script>

<template>
  <div class="rem">
    <UiCombobox v-model:open="open" :model-value="selected" multiple :items="items" :width="260" @update:model-value="onPick">
      <button class="prop-val" :disabled="!canWrite" :title="t('taskReminder.hint')">
        <Bell :size="14" class="faint" /> <span class="truncate" :class="{ faint: reminders == null || !list.length }">{{ summary }}</span>
      </button>
    </UiCombobox>
    <label v-if="canWrite" class="at" :title="t('taskReminder.addDate')">
      <span class="faint">{{ t('taskReminder.addDate') }}</span>
      <input type="datetime-local" @change="addDate" />
    </label>
  </div>
</template>

<style scoped>
.rem { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; min-width: 0; }
.at { display: flex; align-items: center; gap: 6px; font-size: 12px; padding: 0 8px; }
.at input { font: inherit; font-size: 12px; background: transparent; color: inherit; border: 1px solid var(--border, transparent); border-radius: 6px; padding: 1px 4px; }
</style>
