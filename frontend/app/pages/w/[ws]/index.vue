<script setup lang="ts">
import { Home, CircleUserRound, AlertTriangle, CalendarClock, CheckCircle2, FileText, Kanban, ArrowRight, Plus, Activity } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { useWorkspace, useMyTasks, links } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import { useAuth } from '~/composables/useAuth'
import { useUi } from '~/composables/useUi'
import { useI18n } from '~/composables/useI18n'
import { motion } from '~/composables/useMotion'
import { api } from '~/composables/api'

definePageMeta({ layout: 'workspace' })

interface Overview {
  boards: { id: string; name: string; total: number; done: number; overdue: number; due_soon: number; updated_at?: string | null }[]
  my: { open: number; overdue: number; due_week: number; completed_week: number }
  activity: { id: string; task_id: string; task_title: string; board_id: string; board_name: string; user_id: string; user_name?: string | null; activity_type: string; new_value?: any; created_at: string }[]
  docs_updated_week: number
}

const { wsId, workspace, pages, member, canWrite } = useWorkspace()
const { data: myTasks, isLoading: loadingTasks } = useMyTasks()
const { createAndOpen } = usePages()
const { user } = useAuth()
const { createBoardOpen, openCreateTask } = useUi()
const { t, relativeTime, formatDate } = useI18n()

const { data: overview, isLoading } = useQuery({
  queryKey: computed(() => ['ws', wsId.value, 'overview']),
  queryFn: () => api<Overview>(`/api/workspaces/${wsId.value}/overview`),
  enabled: computed(() => !!wsId.value),
  staleTime: 15_000,
})

const greeting = computed(() => {
  const h = new Date().getHours()
  return t(h < 5 ? 'home.goodNight' : h < 12 ? 'home.goodMorning' : h < 18 ? 'home.goodAfternoon' : 'home.goodEvening', { name: user.value?.name?.split(' ')[0] ?? '' })
})
const today = computed(() => formatDate(new Date(), { weekday: 'long', day: 'numeric', month: 'long' }))

const stats = computed(() => [
  { key: 'open', label: t('home.stats.assigned'), value: overview.value?.my.open ?? 0, icon: CircleUserRound, to: links.myTasks(wsId.value), tone: 'accent' },
  { key: 'overdue', label: t('home.stats.overdue'), value: overview.value?.my.overdue ?? 0, icon: AlertTriangle, to: links.myTasks(wsId.value), tone: 'red' },
  { key: 'week', label: t('home.stats.dueWeek'), value: overview.value?.my.due_week ?? 0, icon: CalendarClock, to: links.myTasks(wsId.value), tone: 'yellow' },
  { key: 'done', label: t('home.stats.completedWeek'), value: overview.value?.my.completed_week ?? 0, icon: CheckCircle2, to: links.myTasks(wsId.value), tone: 'green' },
])

const upcoming = computed(() => (myTasks.value ?? []).slice(0, 7))
const recentlyViewed = computed(() =>
  [...(pages.data.value ?? [])].filter((p) => p.last_viewed_at).sort((a, b) => +new Date(b.last_viewed_at!) - +new Date(a.last_viewed_at!)).slice(0, 6),
)
const recentlyUpdated = computed(() =>
  [...(pages.data.value ?? [])].sort((a, b) => +new Date(b.updated_at ?? 0) - +new Date(a.updated_at ?? 0)).slice(0, 6),
)
const docTab = ref<'viewed' | 'updated'>('updated')
watch(recentlyViewed, (v) => { if (v.length && docTab.value === 'updated' && !touchedTab.value) docTab.value = 'viewed' }, { immediate: true })
const touchedTab = ref(false)
const docList = computed(() => (docTab.value === 'viewed' ? recentlyViewed.value : recentlyUpdated.value))

function activityText(a: Overview['activity'][number]) {
  switch (a.activity_type) {
    case 'create': return t('activity.created')
    case 'move_column': return t('activity.moved')
    case 'toggle_validation': return a.new_value?.is_completed ? t('activity.completed') : t('activity.reopened')
    case 'add_assignee': return t('activity.assigned')
    case 'update_title': return t('activity.renamed')
    default: return t('activity.updated')
  }
}

function dueClass(d?: string | null) {
  if (!d) return ''
  const diff = new Date(d).getTime() - Date.now()
  return diff < 0 ? 'overdue' : diff < 3 * 86400000 ? 'soon' : ''
}

// Animations d'entrée
const root = ref<HTMLElement | null>(null)
const statEls = ref<HTMLElement[]>([])
let animated = false
watch([overview, root], async () => {
  if (!overview.value || !root.value || animated) return
  animated = true
  await nextTick()
  motion.stagger(root.value.querySelectorAll('[data-reveal]'), { y: 10, each: 0.05 })
  stats.value.forEach((s, i) => motion.countUp(statEls.value[i], s.value))
}, { immediate: true })

useHead({ title: () => t('nav.home') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: workspace.data.value?.name ?? '', icon: Home }]">
      <UiButton v-if="canWrite" size="sm" variant="primary" :icon="Plus" @click="openCreateTask()">{{ t('task.new') }}</UiButton>
    </AppPageHeader>
  
    <div ref="root" class="home">
      <section class="hero" data-reveal>
        <p class="date">{{ today }}</p>
        <h1>{{ greeting }}</h1>
      </section>
  
      <section class="stats">
        <NuxtLink v-for="(s, i) in stats" :key="s.key" :to="s.to" class="stat" :class="`tone-${s.tone}`" data-reveal>
          <div class="stat-icon"><component :is="s.icon" :size="16" /></div>
          <div class="stat-value tabular" :ref="(el) => { if (el) statEls[i] = el as HTMLElement }">{{ isLoading ? '–' : s.value }}</div>
          <div class="stat-label">{{ s.label }}</div>
        </NuxtLink>
      </section>
  
      <div class="grid">
        <!-- Mes tâches -->
        <section class="panel" data-reveal>
          <header class="panel-head">
            <h2><CircleUserRound :size="15" /> {{ t('home.myTasks') }}</h2>
            <NuxtLink :to="links.myTasks(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink>
          </header>
          <div v-if="loadingTasks" class="panel-body"><div v-for="i in 4" :key="i" class="skeleton row-skel" /></div>
          <div v-else-if="!upcoming.length" class="panel-empty">
            <CheckCircle2 :size="20" />
            <span>{{ t('home.noTasks') }}</span>
          </div>
          <div v-else class="panel-body">
            <NuxtLink v-for="tk in upcoming" :key="tk.id" :to="links.task(wsId, tk.board_id, tk.id)" class="task-row">
              <TaskPriorityIcon :priority="tk.priority" :size="14" />
              <span class="task-title truncate">{{ tk.title }}</span>
              <span class="task-board truncate">{{ tk.board_name }} · {{ tk.column_name }}</span>
              <span v-if="tk.due_date" class="task-due" :class="dueClass(tk.due_date)">{{ formatDate(tk.due_date) }}</span>
            </NuxtLink>
          </div>
        </section>
  
        <!-- Documents -->
        <section class="panel" data-reveal>
          <header class="panel-head">
            <h2><FileText :size="15" /> {{ t('nav.docs') }}</h2>
            <UiSegmented
              v-model="docTab"
              :options="[{ value: 'viewed', label: t('home.recentlyViewed') }, { value: 'updated', label: t('home.recentlyUpdated') }]"
              @update:model-value="touchedTab = true"
            />
          </header>
          <div v-if="!docList.length" class="panel-empty">
            <FileText :size="20" />
            <span>{{ t('home.noDocs') }}</span>
            <UiButton v-if="canWrite" size="sm" :icon="Plus" @click="createAndOpen()">{{ t('docs.new') }}</UiButton>
          </div>
          <div v-else class="panel-body">
            <NuxtLink v-for="p in docList" :key="p.id" :to="links.doc(wsId, p.id)" class="doc-row">
              <span class="doc-icon">{{ p.icon || '' }}<FileText v-if="!p.icon" :size="15" /></span>
              <span class="truncate doc-title">{{ p.title || t('docs.untitled') }}</span>
              <span class="doc-meta">
                <UiAvatar v-if="docTab === 'updated' && p.updated_by" :name="member(p.updated_by)?.name" :src="member(p.updated_by)?.avatar_url" :size="16" />
                {{ relativeTime(docTab === 'viewed' ? p.last_viewed_at : p.updated_at) }}
              </span>
            </NuxtLink>
          </div>
        </section>
  
        <!-- Tableaux -->
        <section class="panel wide" data-reveal>
          <header class="panel-head">
            <h2><Kanban :size="15" /> {{ t('nav.boards') }}</h2>
            <UiButton v-if="canWrite" size="xs" variant="ghost" :icon="Plus" @click="createBoardOpen = true">{{ t('board.new') }}</UiButton>
          </header>
          <div v-if="!overview?.boards.length" class="panel-empty">
            <Kanban :size="20" />
            <span>{{ t('board.noBoards') }}</span>
          </div>
          <div v-else class="boards">
            <NuxtLink v-for="b in overview.boards" :key="b.id" :to="links.board(wsId, b.id)" class="board-card">
              <div class="board-top">
                <span class="board-name truncate">{{ b.name }}</span>
                <span class="board-pct tabular">{{ b.total ? Math.round((b.done / b.total) * 100) : 0 }}%</span>
              </div>
              <div class="bar"><div class="bar-fill" :style="{ width: `${b.total ? (b.done / b.total) * 100 : 0}%` }" /></div>
              <div class="board-meta">
                <span>{{ t('home.boardOpen', { count: b.total - b.done }) }}</span>
                <span v-if="b.overdue" class="overdue">· {{ t('home.boardOverdue', { count: b.overdue }) }}</span>
                <span v-else-if="b.due_soon" class="soon">· {{ t('home.boardDueSoon', { count: b.due_soon }) }}</span>
              </div>
            </NuxtLink>
          </div>
        </section>
  
        <!-- Activité -->
        <section class="panel wide" data-reveal>
          <header class="panel-head"><h2><Activity :size="15" /> {{ t('home.activity') }}</h2></header>
          <div v-if="!overview?.activity.length" class="panel-empty"><Activity :size="20" /><span>{{ t('home.noActivity') }}</span></div>
          <ol v-else class="feed">
            <li v-for="a in overview.activity" :key="a.id" class="feed-item">
              <UiAvatar :name="a.user_name" :src="member(a.user_id)?.avatar_url" :size="20" />
              <span class="feed-text">
                <strong>{{ a.user_name }}</strong> {{ activityText(a) }}
                <NuxtLink :to="links.task(wsId, a.board_id, a.task_id)" class="feed-link">{{ a.task_title }}</NuxtLink>
                <span class="faint"> · {{ a.board_name }}</span>
              </span>
              <time class="feed-time">{{ relativeTime(a.created_at) }}</time>
            </li>
          </ol>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.home { flex: 1; overflow-y: auto; padding: 32px 40px 64px; }
.home > * { max-width: 1120px; margin-left: auto; margin-right: auto; }
.hero .date { margin: 0; color: var(--text-3); font-size: var(--fs-sm); text-transform: capitalize; }
.hero h1 { margin: 4px 0 0; font-family: var(--font-display); font-size: 26px; font-weight: 600; letter-spacing: -0.022em; }

.stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; margin-top: 24px; }
.stat {
  position: relative;
  padding: 14px 16px;
  border-radius: var(--r-lg);
  background: var(--bg-panel);
  box-shadow: var(--shadow-card);
  transition: transform var(--dur) var(--ease-out), background var(--dur-fast);
  overflow: hidden;
}
.stat:hover { transform: translateY(-1px); background: var(--bg-card-hover); }
.stat-icon { width: 26px; height: 26px; border-radius: 7px; display: grid; place-items: center; margin-bottom: 12px; }
.tone-accent .stat-icon { background: var(--accent-soft); color: var(--accent-text); }
.tone-red .stat-icon { background: var(--red-soft); color: var(--red); }
.tone-yellow .stat-icon { background: rgba(242, 201, 76, 0.14); color: var(--yellow); }
.tone-green .stat-icon { background: var(--green-soft); color: var(--green); }
.stat-value { font-size: 26px; font-weight: 600; letter-spacing: -0.02em; line-height: 1; }
.stat-label { margin-top: 6px; color: var(--text-2); font-size: var(--fs-sm); }

.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-top: 16px; }
.panel { background: var(--bg-panel); border-radius: var(--r-lg); box-shadow: var(--shadow-card); min-width: 0; display: flex; flex-direction: column; }
.panel.wide { grid-column: span 1; }
.panel-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 12px 14px 8px; }
.panel-head h2 { margin: 0; display: flex; align-items: center; gap: 8px; font-size: var(--fs-md); font-weight: 600; }
.panel-head h2 svg { color: var(--text-3); }
.panel-link { display: inline-flex; align-items: center; gap: 4px; color: var(--text-3); font-size: var(--fs-sm); }
.panel-link:hover { color: var(--text); }
.panel-body { padding: 0 6px 8px; }
.panel-empty { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 28px 16px 32px; color: var(--text-3); text-align: center; }
.row-skel { height: 32px; margin: 4px 8px; }

.task-row, .doc-row {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 36px;
  padding: 0 8px;
  border-radius: var(--r-sm);
  transition: background var(--dur-fast);
}
.task-row:hover, .doc-row:hover { background: var(--bg-hover); }
.task-title, .doc-title { flex: 1; min-width: 0; font-weight: 500; }
.task-board { color: var(--text-3); font-size: var(--fs-sm); max-width: 40%; }
.task-due { font-size: var(--fs-sm); color: var(--text-2); font-variant-numeric: tabular-nums; flex: none; }
.overdue { color: var(--red) !important; }
.soon { color: var(--orange) !important; }
.doc-icon { width: 18px; display: grid; place-items: center; color: var(--text-3); flex: none; }
.doc-meta { display: flex; align-items: center; gap: 6px; color: var(--text-3); font-size: var(--fs-sm); flex: none; }

.boards { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 8px; padding: 0 12px 12px; }
.board-card { padding: 12px; border-radius: var(--r-md); background: var(--bg-card); box-shadow: var(--shadow-card); transition: background var(--dur-fast), transform var(--dur) var(--ease-out); }
.board-card:hover { background: var(--bg-card-hover); transform: translateY(-1px); }
.board-top { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.board-name { font-weight: 600; }
.board-pct { color: var(--text-3); font-size: var(--fs-sm); }
.bar { height: 4px; border-radius: 4px; background: var(--bg-active); margin: 10px 0 8px; overflow: hidden; }
.bar-fill { height: 100%; border-radius: 4px; background: linear-gradient(90deg, var(--accent), #b18cff); transition: width 0.8s var(--ease-out); }
.board-meta { font-size: var(--fs-sm); color: var(--text-3); }

.feed { list-style: none; margin: 0; padding: 0 14px 12px; display: flex; flex-direction: column; }
.feed-item { display: flex; align-items: flex-start; gap: 10px; padding: 7px 0; border-bottom: 1px solid var(--border); }
.feed-item:last-child { border-bottom: none; }
.feed-text { flex: 1; min-width: 0; color: var(--text-2); line-height: 1.45; }
.feed-text strong { color: var(--text); font-weight: 500; }
.feed-link { color: var(--text); font-weight: 500; }
.feed-link:hover { text-decoration: underline; }
.feed-time { color: var(--text-3); font-size: var(--fs-sm); flex: none; }

@media (max-width: 1023px) {
  .home { padding: 24px 20px 48px; }
  .grid { grid-template-columns: 1fr; }
}
@media (max-width: 639px) {
  .home { padding: 18px 14px 32px; }
  .stats { grid-template-columns: repeat(2, 1fr); gap: 8px; }
  .hero h1 { font-size: 22px; }
  .task-board { display: none; }
}
</style>
