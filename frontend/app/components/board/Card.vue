<script setup lang="ts">
import { CalendarDays, ListTree, Check, Users } from 'lucide-vue-next'
import type { Task } from '~/types'
import { useWorkspace } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'

const props = defineProps<{
  task: Task
  subtasks: Task[]
  focused?: boolean
  canWrite?: boolean
}>()
const emit = defineEmits<{
  (e: 'open', t: Task): void
  (e: 'toggle', t: Task, ev: MouseEvent): void
  (e: 'menu', t: Task, ev: MouseEvent): void
}>()

const { member } = useWorkspace()
const { user } = useAuth()
const { formatDate } = useI18n()

const due = computed(() => {
  if (!props.task.due_date) return null
  const d = new Date(props.task.due_date)
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
  const day = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  const tone = props.task.is_completed ? '' : day < today ? 'overdue' : day - today <= 2 * 86400000 ? 'soon' : ''
  return { label: formatDate(d), tone }
})
const subDone = computed(() => props.subtasks.filter((s) => s.is_completed).length)
const myValidation = computed(() => props.task.is_per_person && !!user.value && props.task.validations.includes(user.value.id))
const validationText = computed(() => {
  if (!props.task.is_per_person) return null
  const total = props.task.assignees.length || 1
  return `${props.task.validations.length}/${total}`
})
</script>

<template>
  <article
    class="card"
    :class="{ focused, done: task.is_completed }"
    :data-task-id="task.id"
    tabindex="-1"
    @click="emit('open', task)"
    @contextmenu.prevent="emit('menu', task, $event)"
  >
    <div class="card-head">
      <button
        v-if="canWrite"
        class="card-check"
        :class="{ on: task.is_completed, partial: myValidation && !task.is_completed }"
        :aria-label="$t('task.markDone')"
        @click.stop="emit('toggle', task, $event)"
      >
        <Check v-if="task.is_completed || myValidation" :size="11" :stroke-width="3" />
      </button>
      <h4 class="card-title">{{ task.title }}</h4>
      <span v-if="task.assignees.length" class="card-avatars">
        <UiAvatar
          v-for="a in task.assignees.slice(0, 3)"
          :key="a"
          :name="member(a)?.name"
          :src="member(a)?.avatar_url"
          :size="18"
          ring
          :class="{ validated: task.is_per_person && task.validations.includes(a) }"
        />
      </span>
    </div>
    <div v-if="task.priority || task.labels.length || due || subtasks.length || validationText" class="card-meta">
      <span v-if="task.priority" class="chip icon-only" :title="$t(`priority.${task.priority}`)"><TaskPriorityIcon :priority="task.priority" :size="13" /></span>
      <span v-for="l in task.labels.slice(0, 3)" :key="l.id" class="chip label"><span class="dot" :style="{ background: l.color }" />{{ l.name }}</span>
      <span v-if="task.labels.length > 3" class="chip">+{{ task.labels.length - 3 }}</span>
      <span v-if="due" class="chip" :class="due.tone"><CalendarDays :size="12" />{{ due.label }}</span>
      <span v-if="subtasks.length" class="chip" :class="{ complete: subDone === subtasks.length }"><ListTree :size="12" />{{ subDone }}/{{ subtasks.length }}</span>
      <span v-if="validationText" class="chip" :title="$t('task.perPerson')"><Users :size="12" />{{ validationText }}</span>
    </div>
  </article>
</template>

<style scoped>
.card {
  position: relative;
  padding: 10px 11px 9px;
  border-radius: var(--r-md);
  background: var(--bg-card);
  box-shadow: var(--shadow-card);
  cursor: pointer;
  user-select: none;
  outline: none;
  transition: background var(--dur-fast) var(--ease), box-shadow var(--dur-fast) var(--ease), transform var(--dur) var(--ease-out);
}
.card:hover { background: var(--bg-card-hover); box-shadow: var(--shadow-card), 0 0 0 1px var(--border-strong); }
.card.focused { box-shadow: 0 0 0 1.5px var(--accent), var(--shadow-card); }
.card.done .card-title { color: var(--text-3); }
.card-head { display: flex; align-items: flex-start; gap: 8px; }
.card-title {
  flex: 1;
  min-width: 0;
  margin: 0;
  font-size: var(--fs-md);
  font-weight: 500;
  line-height: 1.4;
  color: var(--text);
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}
.card-check {
  flex: none;
  width: 15px;
  height: 15px;
  margin-top: 1.5px;
  border-radius: 50%;
  border: 1.5px solid var(--text-3);
  display: grid;
  place-items: center;
  color: #fff;
  transition: background var(--dur-fast), border-color var(--dur-fast), transform var(--dur-fast);
}
.card-check:hover { border-color: var(--accent); transform: scale(1.1); }
.card-check.on { background: var(--accent); border-color: var(--accent); }
.card-check.partial { border-color: var(--accent); color: var(--accent); border-style: dashed; }
.card-avatars { display: flex; flex: none; }
.card-avatars > * + * { margin-left: -5px; }
.card-avatars .validated { box-shadow: 0 0 0 2px var(--green); }
.card-meta { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 8px; }
.chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 20px;
  padding: 0 6px;
  border-radius: var(--r-xs);
  border: 1px solid var(--border);
  color: var(--text-2);
  font-size: 11.5px;
  font-weight: 500;
  white-space: nowrap;
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.chip.icon-only { padding: 0 4px; }
.chip.label { border-radius: var(--r-full); }
.chip .dot { width: 7px; height: 7px; border-radius: 50%; flex: none; }
.chip.overdue { color: var(--red); border-color: color-mix(in srgb, var(--red) 30%, transparent); }
.chip.soon { color: var(--orange); }
.chip.complete { color: var(--green); }
@media (pointer: coarse) {
  .card { padding: 12px; }
  .card-check { width: 20px; height: 20px; }
}
</style>
