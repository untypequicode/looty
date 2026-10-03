<script setup lang="ts">
import { Plus, ListTree } from 'lucide-vue-next'
import type { Task } from '~/types'
import { motion } from '~/composables/useMotion'

const props = defineProps<{ subtasks: Task[]; canWrite: boolean }>()
const emit = defineEmits<{
  (e: 'create', title: string): void
  (e: 'toggle', t: Task): void
  (e: 'open', t: Task): void
}>()
const adding = ref(false)
const value = ref('')
const input = ref<HTMLInputElement | null>(null)
const done = computed(() => props.subtasks.filter((s) => s.is_completed).length)

function start() {
  adding.value = true
  nextTick(() => input.value?.focus())
}
function submit() {
  if (value.value.trim()) emit('create', value.value.trim())
  value.value = ''
}
function toggle(s: Task, e: MouseEvent) {
  motion.pulse(e.currentTarget as Element)
  emit('toggle', s)
}
</script>

<template>
  <section class="st">
    <header class="st-head">
      <ListTree :size="14" class="faint" />
      <span class="st-title">{{ $t('task.subtasks') }}</span>
      <span v-if="subtasks.length" class="faint tabular">{{ done }}/{{ subtasks.length }}</span>
      <div v-if="subtasks.length" class="st-bar"><div :style="{ width: `${(done / subtasks.length) * 100}%` }" /></div>
      <span class="spacer" />
      <UiButton v-if="canWrite" size="xs" variant="ghost" :icon="Plus" @click="start" />
    </header>
    <div class="st-list">
      <div v-for="s in subtasks" :key="s.id" class="st-row" @click="emit('open', s)">
        <button class="st-check" :class="{ on: s.is_completed }" :disabled="!canWrite" @click.stop="toggle(s, $event)">
          <svg v-if="s.is_completed" width="10" height="10" viewBox="0 0 10 10"><path d="M2 5.2 4.1 7.2 8 3" fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>
        <span class="st-name truncate" :class="{ done: s.is_completed }">{{ s.title }}</span>
        <TaskPriorityIcon v-if="s.priority" :priority="s.priority" :size="13" />
      </div>
      <form v-if="adding" class="st-row adding" @submit.prevent="submit">
        <span class="st-check" />
        <input ref="input" v-model="value" :placeholder="$t('task.subtaskPlaceholder')" @keydown.esc="adding = false" @blur="!value.trim() && (adding = false)" />
      </form>
      <button v-else-if="canWrite && !subtasks.length" class="st-empty" @click="start">{{ $t('task.addSubtask') }}</button>
    </div>
  </section>
</template>

<style scoped>
.st-head { display: flex; align-items: center; gap: 8px; height: 30px; }
.st-title { font-weight: 600; font-size: var(--fs-md); }
.st-bar { width: 60px; height: 4px; border-radius: 4px; background: var(--bg-active); overflow: hidden; }
.st-bar > div { height: 100%; background: var(--green); transition: width var(--dur-slow) var(--ease-out); }
.st-list { display: flex; flex-direction: column; margin-top: 4px; }
.st-row { display: flex; align-items: center; gap: 10px; height: 34px; padding: 0 8px; border-radius: var(--r-sm); cursor: pointer; }
.st-row:hover { background: var(--bg-hover); }
.st-row.adding { cursor: default; background: var(--bg-hover); }
.st-row input { flex: 1; border: none; outline: none; background: transparent; }
.st-check {
  width: 15px; height: 15px; border-radius: 50%; border: 1.5px solid var(--text-3); display: grid; place-items: center; flex: none;
  transition: background var(--dur-fast), border-color var(--dur-fast);
}
.st-check:hover:not(:disabled) { border-color: var(--accent); }
.st-check.on { background: var(--accent); border-color: var(--accent); }
.st-name { flex: 1; }
.st-name.done { color: var(--text-3); text-decoration: line-through; }
.st-empty { text-align: left; color: var(--text-3); padding: 6px 8px; border-radius: var(--r-sm); font-size: var(--fs-md); }
.st-empty:hover { background: var(--bg-hover); color: var(--text-2); }
</style>
