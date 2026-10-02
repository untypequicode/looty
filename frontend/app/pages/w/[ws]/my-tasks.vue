<script setup lang="ts">
import { CircleUserRound, CheckCircle2, Circle } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import type { MyTask } from '~/types'
import { useMyTasks, useWorkspace, links, wsKeys } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { api } from '~/composables/api'
import { motion } from '~/composables/useMotion'

definePageMeta({ layout: 'workspace' })

const { data: tasks, isLoading } = useMyTasks()
const { wsId } = useWorkspace()
const { t, formatDate } = useI18n()
const qc = useQueryClient()
const groupBy = ref<'due' | 'board'>('due')

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()

const groups = computed(() => {
  const list = tasks.value ?? []
  const map = new Map<string, { key: string; label: string; tone?: string; items: MyTask[] }>()
  const push = (key: string, label: string, item: MyTask, tone?: string) => {
    if (!map.has(key)) map.set(key, { key, label, tone, items: [] })
    map.get(key)!.items.push(item)
  }
  if (groupBy.value === 'board') {
    for (const tk of list) push(tk.board_id, tk.board_name, tk)
    return [...map.values()]
  }
  const today = startOfDay(new Date())
  for (const tk of list) {
    if (!tk.due_date) { push('none', t('myTasks.noDate'), tk); continue }
    const d = startOfDay(new Date(tk.due_date))
    if (d < today) push('overdue', t('myTasks.overdue'), tk, 'red')
    else if (d === today) push('today', t('date.today'), tk, 'orange')
    else if (d - today <= 7 * 86400000) push('week', t('myTasks.thisWeek'), tk)
    else push('later', t('myTasks.later'), tk)
  }
  const order = ['overdue', 'today', 'week', 'later', 'none']
  return order.filter((k) => map.has(k)).map((k) => map.get(k)!)
})

const completing = ref<Set<string>>(new Set())
async function complete(tk: MyTask, e: MouseEvent) {
  completing.value = new Set([...completing.value, tk.id])
  motion.pulse(e.currentTarget as Element)
  try {
    await api(`/api/boards/tasks/${tk.id}/toggle_validation`, { method: 'PUT' })
    setTimeout(() => qc.invalidateQueries({ queryKey: wsKeys.myTasks(wsId.value) }), 450)
  } catch {
    completing.value.delete(tk.id)
  }
}

const root = ref<HTMLElement | null>(null)
watch([groups, root], () => nextTick(() => root.value && motion.stagger(root.value.querySelectorAll('.mt-row'), { y: 4, each: 0.012 })), { once: true })
useHead({ title: () => t('nav.myTasks') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('nav.myTasks'), icon: CircleUserRound }]">
      <UiSegmented v-model="groupBy" :options="[{ value: 'due', label: t('myTasks.byDue') }, { value: 'board', label: t('myTasks.byBoard') }]" />
    </AppPageHeader>
    <div ref="root" class="mt">
      <div v-if="isLoading" class="mt-inner"><div v-for="i in 6" :key="i" class="skeleton" style="height: 38px; margin: 6px 0" /></div>
      <UiEmpty v-else-if="!tasks?.length" :icon="CheckCircle2" :title="t('myTasks.empty')" :description="t('myTasks.emptyHint')" />
      <div v-else class="mt-inner">
        <section v-for="g in groups" :key="g.key" class="mt-group">
          <h2 class="mt-group-title" :class="g.tone">{{ g.label }} <span class="faint tabular">{{ g.items.length }}</span></h2>
          <NuxtLink v-for="tk in g.items" :key="tk.id" :to="links.task(wsId, tk.board_id, tk.id)" class="mt-row" :class="{ done: completing.has(tk.id) }">
            <button class="mt-check" :aria-label="t('task.markDone')" @click.prevent.stop="complete(tk, $event)">
              <CheckCircle2 v-if="completing.has(tk.id)" :size="16" class="ok" />
              <Circle v-else :size="16" />
            </button>
            <TaskPriorityIcon :priority="tk.priority" :size="14" />
            <span class="mt-title truncate">{{ tk.title }}</span>
            <span class="mt-col">{{ tk.column_name }}</span>
            <span v-if="groupBy === 'due'" class="mt-board truncate">{{ tk.board_name }}</span>
            <span v-if="tk.due_date" class="mt-due tabular">{{ formatDate(tk.due_date) }}</span>
          </NuxtLink>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mt { flex: 1; overflow-y: auto; padding: 8px 16px 48px; }
.mt-inner { max-width: 960px; margin: 0 auto; }
.mt-group { margin-top: 16px; }
.mt-group-title { display: flex; align-items: center; gap: 8px; margin: 0 0 4px; padding: 0 10px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-2); }
.mt-group-title.red { color: var(--red); }
.mt-group-title.orange { color: var(--orange); }
.mt-row {
  display: flex; align-items: center; gap: 10px; height: 38px; padding: 0 10px;
  border-radius: var(--r-sm); transition: background var(--dur-fast), opacity var(--dur-slow);
}
.mt-row:hover { background: var(--bg-hover); }
.mt-row.done { opacity: 0.45; }
.mt-row.done .mt-title { text-decoration: line-through; }
.mt-check { width: 22px; height: 22px; display: grid; place-items: center; color: var(--text-3); border-radius: 50%; flex: none; }
.mt-check:hover { color: var(--accent); }
.ok { color: var(--accent); }
.mt-title { flex: 1; min-width: 0; font-weight: 500; }
.mt-col { font-size: var(--fs-sm); color: var(--text-3); padding: 1px 7px; border-radius: var(--r-full); border: 1px solid var(--border); flex: none; }
.mt-board { font-size: var(--fs-sm); color: var(--text-3); max-width: 160px; flex: none; }
.mt-due { font-size: var(--fs-sm); color: var(--text-2); flex: none; width: 64px; text-align: right; }
@media (max-width: 639px) { .mt-board, .mt-col { display: none; } .mt { padding: 4px 8px 32px; } }
</style>
