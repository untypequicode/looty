<script setup lang="ts">
import { Search, ListFilter, SlidersHorizontal, X, Check, UserRound, Tag, Signal, CalendarDays } from 'lucide-vue-next'
import type { Board } from '~/types'
import type { DueFilter, BoardFilters } from '~/composables/useBoardFilters'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { PRIORITIES } from '~/composables/useBoard'

const props = defineProps<{ board: Board; filters: any; total: number; shown: number }>()
const { members, member } = useWorkspace()
const { t } = useI18n()

const f = props.filters as BoardFilters
const filterOpen = ref(false)
const displayOpen = ref(false)
const tab = ref<'assignee' | 'label' | 'priority' | 'due'>('assignee')
const search = ref<HTMLInputElement | null>(null)
const searchOpen = ref(!!f.q.value)

function toggle(list: { value: string[] }, v: string) {
  list.value = list.value.includes(v) ? list.value.filter((x) => x !== v) : [...list.value, v]
}

const tabs = computed(() => [
  { key: 'assignee' as const, label: t('task.assignee'), icon: UserRound, count: f.assignees.value.length },
  { key: 'label' as const, label: t('task.labels'), icon: Tag, count: f.labels.value.length },
  { key: 'priority' as const, label: t('task.priority'), icon: Signal, count: f.priorities.value.length },
  { key: 'due' as const, label: t('task.dueDate'), icon: CalendarDays, count: f.due.value.length },
])
const dueOptions: DueFilter[] = ['overdue', 'today', 'week', 'none']

const chips = computed(() => [
  ...f.assignees.value.map((a) => ({ key: `a${a}`, label: a === 'none' ? t('filters.unassigned') : member(a)?.name ?? '?', remove: () => toggle(f.assignees, a) })),
  ...f.labels.value.map((l) => ({ key: `l${l}`, label: props.board.labels.find((x) => x.id === l)?.name ?? '?', color: props.board.labels.find((x) => x.id === l)?.color, remove: () => toggle(f.labels, l) })),
  ...f.priorities.value.map((p) => ({ key: `p${p}`, label: t(`priority.${p}`), remove: () => toggle(f.priorities, p) })),
  ...f.due.value.map((d) => ({ key: `d${d}`, label: t(`filters.due.${d}`), remove: () => toggle(f.due as any, d) })),
])

function openSearch() {
  searchOpen.value = true
  nextTick(() => search.value?.focus())
}
defineExpose({ openSearch, openFilters: () => (filterOpen.value = true) })
</script>

<template>
  <div class="tb">
    <div class="tb-row">
      <UiPopover v-model:open="filterOpen" :width="440" placement="bottom-start">
        <template #trigger>
          <UiButton size="sm" variant="ghost" :icon="ListFilter" :class="{ active: chips.length }">
            {{ t('common.filter') }}<span v-if="chips.length" class="tb-badge">{{ chips.length }}</span>
          </UiButton>
        </template>
        <div class="fm">
          <nav class="fm-tabs">
            <button v-for="tb in tabs" :key="tb.key" class="fm-tab" :class="{ active: tab === tb.key }" @click="tab = tb.key">
              <component :is="tb.icon" :size="14" /> <span class="truncate">{{ tb.label }}</span>
              <span v-if="tb.count" class="fm-count">{{ tb.count }}</span>
            </button>
          </nav>
          <div class="fm-panel">
            <template v-if="tab === 'assignee'">
              <button class="fm-opt" @click="toggle(f.assignees, 'none')">
                <span class="fm-check" :class="{ on: f.assignees.value.includes('none') }"><Check :size="11" :stroke-width="3" /></span>
                <UserRound :size="16" class="faint" /> {{ t('filters.unassigned') }}
              </button>
              <button v-for="m in members.data.value ?? []" :key="m.user_id" class="fm-opt" @click="toggle(f.assignees, m.user_id)">
                <span class="fm-check" :class="{ on: f.assignees.value.includes(m.user_id) }"><Check :size="11" :stroke-width="3" /></span>
                <UiAvatar :name="m.name" :src="m.avatar_url" :size="18" /> <span class="truncate">{{ m.name }}</span>
              </button>
            </template>
            <template v-else-if="tab === 'label'">
              <button v-for="l in board.labels" :key="l.id" class="fm-opt" @click="toggle(f.labels, l.id)">
                <span class="fm-check" :class="{ on: f.labels.value.includes(l.id) }"><Check :size="11" :stroke-width="3" /></span>
                <span class="dot" :style="{ background: l.color }" /> <span class="truncate">{{ l.name }}</span>
              </button>
              <p v-if="!board.labels.length" class="faint fm-empty">{{ t('board.noLabels') }}</p>
            </template>
            <template v-else-if="tab === 'priority'">
              <button v-for="p in PRIORITIES" :key="p" class="fm-opt" @click="toggle(f.priorities, String(p))">
                <span class="fm-check" :class="{ on: f.priorities.value.includes(String(p)) }"><Check :size="11" :stroke-width="3" /></span>
                <TaskPriorityIcon :priority="p" :size="15" /> {{ t(`priority.${p}`) }}
              </button>
            </template>
            <template v-else>
              <button v-for="d in dueOptions" :key="d" class="fm-opt" @click="toggle(f.due as any, d)">
                <span class="fm-check" :class="{ on: f.due.value.includes(d) }"><Check :size="11" :stroke-width="3" /></span>
                {{ t(`filters.due.${d}`) }}
              </button>
            </template>
          </div>
        </div>
      </UiPopover>

      <div v-if="searchOpen" class="tb-search">
        <Search :size="14" class="faint" />
        <input ref="search" v-model="f.q.value" :placeholder="t('filters.searchTasks')" @keydown.esc="f.q.value = ''; searchOpen = false; ($event.target as HTMLElement).blur()" @blur="!f.q.value && (searchOpen = false)" />
        <button v-if="f.q.value" @click="f.q.value = ''"><X :size="13" /></button>
      </div>
      <UiTooltip v-else :text="t('common.search')" shortcut="f">
        <UiButton size="sm" variant="ghost" :icon="Search" @click="openSearch" />
      </UiTooltip>

      <div class="tb-chips">
        <span v-for="c in chips" :key="c.key" class="tb-chip">
          <span v-if="c.color" class="dot" :style="{ background: c.color }" />
          {{ c.label }}
          <button @click="c.remove()"><X :size="12" /></button>
        </span>
        <button v-if="chips.length > 1" class="tb-clear" @click="f.clear()">{{ t('filters.clear') }}</button>
      </div>

      <span class="spacer" />
      <span v-if="shown !== total" class="faint tb-count tabular">{{ t('filters.showing', { shown, total }) }}</span>

      <UiPopover v-model:open="displayOpen" :width="300" placement="bottom-end">
        <template #trigger><UiButton size="sm" variant="ghost" :icon="SlidersHorizontal">{{ t('board.display') }}</UiButton></template>
        <div class="disp">
          <div class="disp-col"><span>{{ t('filters.completed') }}</span>
            <UiSegmented v-model="f.doneMode.value" :options="[{ value: 'show', label: t('filters.doneShow') }, { value: 'hide', label: t('filters.doneHide') }, { value: 'mine', label: t('filters.doneMine') }]" />
          </div>
          <label class="disp-row"><span>{{ t('filters.showSubtasks') }}</span><UiSwitch v-model="f.showSubtasks.value" /></label>
        </div>
      </UiPopover>
    </div>
  </div>
</template>

<style scoped>
.tb { padding: 6px 12px; border-bottom: 1px solid var(--border); flex: none; }
.tb-row { display: flex; align-items: center; gap: 4px; min-height: 30px; flex-wrap: wrap; }
.tb-badge {
  margin-left: 4px; min-width: 16px; height: 16px; padding: 0 4px; border-radius: 8px;
  background: var(--accent); color: #fff; font-size: 10px; display: inline-grid; place-items: center;
}
.tb-search {
  display: flex; align-items: center; gap: 6px; height: 26px; padding: 0 8px; width: 220px;
  border-radius: var(--r-sm); background: var(--bg-input); box-shadow: 0 0 0 1px var(--border-strong);
}
.tb-search input { flex: 1; border: none; outline: none; background: transparent; min-width: 0; font-size: var(--fs-sm); }
.tb-search button { color: var(--text-3); display: grid; }
.tb-chips { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
.tb-chip {
  display: inline-flex; align-items: center; gap: 5px; height: 24px; padding: 0 4px 0 8px;
  border-radius: var(--r-sm); background: var(--bg-hover); border: 1px solid var(--border); font-size: var(--fs-sm);
}
.tb-chip button { width: 18px; height: 18px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); }
.tb-chip button:hover { background: var(--bg-active); color: var(--text); }
.tb-clear { font-size: var(--fs-sm); color: var(--text-3); padding: 0 6px; }
.tb-clear:hover { color: var(--text); }
.tb-count { font-size: var(--fs-sm); margin-right: 6px; }
.dot { width: 8px; height: 8px; border-radius: 50%; flex: none; }
.fm { display: flex; min-height: 240px; }
.fm-tabs { width: 150px; border-right: 1px solid var(--border); padding: 4px; display: flex; flex-direction: column; gap: 1px; flex: none; }
.fm-tab { display: flex; align-items: center; gap: 8px; height: 30px; padding: 0 8px; border-radius: var(--r-sm); color: var(--text-2); text-align: left; }
.fm-tab:hover { background: var(--bg-hover); }
.fm-tab.active { background: var(--bg-active); color: var(--text); }
.fm-count { margin-left: auto; font-size: var(--fs-xs); color: var(--accent-text); }
.fm-panel { flex: 1; padding: 4px; overflow-y: auto; max-height: 320px; }
.fm-opt { display: flex; align-items: center; gap: 9px; width: 100%; height: 30px; padding: 0 8px; border-radius: var(--r-sm); text-align: left; }
.fm-opt:hover { background: var(--bg-hover); }
.fm-check { width: 14px; height: 14px; border-radius: 4px; border: 1.5px solid var(--text-3); display: grid; place-items: center; color: transparent; flex: none; }
.fm-check.on { background: var(--accent); border-color: var(--accent); color: #fff; }
.fm-empty { padding: 12px; font-size: var(--fs-sm); }
.disp { padding: 8px; display: flex; flex-direction: column; gap: 4px; }
.disp-row { display: flex; align-items: center; justify-content: space-between; height: 32px; padding: 0 4px; cursor: pointer; }
.disp-col { display: flex; flex-direction: column; gap: 6px; padding: 4px 4px 8px; font-size: var(--fs-sm); color: var(--text-2); }
@media (max-width: 639px) {
  .fm { flex-direction: column; }
  .fm-tabs { width: auto; flex-direction: row; overflow-x: auto; border-right: none; border-bottom: 1px solid var(--border); }
  .fm-tab { flex: none; }
  .tb-search { width: 100%; order: 10; }
  .tb-count { display: none; }
}
</style>
