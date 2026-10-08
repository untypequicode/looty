<script setup lang="ts">
import { useCalendars, useCalendarEvents, useCalendarPrefs, startOfDay, addDays, sameDay } from '~/composables/useCalendar'
import { Home, CircleUserRound, AlertTriangle, CalendarClock, CheckCircle2, FileText, Kanban, ArrowRight, Plus, Activity, CalendarDays, MapPin, LayoutGrid, Star, Inbox as InboxIcon, NotebookPen, Users, Mail, BadgeCheck, CalendarHeart, CalendarRange } from 'lucide-vue-next'
import { useDashboard } from '~/composables/useDashboard'
import { useNotifications } from '~/composables/useNotifications'
import { useMailAccounts } from '~/composables/useMail'
import { presence } from '~/composables/useRealtime'
import { notepadOpen } from '~/composables/useUi'
import { useLootyCalendars, startOfWeek } from '~/composables/useCalendar'
import { useQuery } from '@tanstack/vue-query'
import { useAccess } from '~/composables/useAccess'
import { useWorkspace, useMyTasks, links } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import { useAuth } from '~/composables/useAuth'
import { useUi } from '~/composables/useUi'
import { useI18n } from '~/composables/useI18n'
import { motion } from '~/composables/useMotion'
import { api } from '~/composables/api'


interface Overview {
  boards: { id: string; name: string; total: number; done: number; overdue: number; due_soon: number; updated_at?: string | null }[]
  my: { open: number; overdue: number; due_week: number; completed_week: number }
  activity: { id: string; task_id: string; task_title: string; board_id: string; board_name: string; user_id: string; user_name?: string | null; activity_type: string; new_value?: any; created_at: string }[]
  docs_updated_week: number
}

const { wsId, workspace, pages, member } = useWorkspace()
const { can } = useAccess(wsId)
const { data: myTasks, isLoading: loadingTasks } = useMyTasks()
const { createAndOpen } = usePages()
const { user } = useAuth()
const { createBoardOpen, openCreateTask } = useUi()

// --- Tableau de bord personnalisable ---
const { layout, saved: dashSaved, loaded: dashLoaded } = useDashboard(wsId)
const customizeOpen = ref(false)
// Première visite : la personnalisation s'ouvre ; elle doit être enregistrée au moins une fois.
watch([dashLoaded, dashSaved], ([l, s]) => { if (l && !s) customizeOpen.value = true }, { immediate: true })
const has = (id: string) => layout.value.some((w) => w.id === id)
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

// Agenda des deux prochains jours (si l'utilisateur a des calendriers)
const calFrom = ref(startOfDay(new Date()))
const calTo = computed(() => addDays(calFrom.value, 2))
const { data: calendarsList } = useCalendars(wsId)
const { data: upcomingCal } = useCalendarEvents(wsId, calFrom, calTo)
const { hiddenIds: hiddenCals } = useCalendarPrefs(wsId)
const calColor = (id: string) => calendarsList.value?.find((c) => c.id === id)?.color ?? 'var(--accent)'
const { data: lootyCals } = useLootyCalendars(wsId)
const evColor = (id: string) => lootyCals.value?.find((c) => c.id === id)?.color ?? calColor(id)
const nextEvents = computed(() => (upcomingCal.value?.events ?? []).filter((e) => !hiddenCals.value.has(e.calendar_id) && (e.all_day || +new Date(e.ends_at) > Date.now())).slice(0, 6))
// Widgets supplémentaires
const overdueTasks = computed(() => (myTasks.value ?? []).filter((tk) => tk.due_date && new Date(tk.due_date) < new Date()).slice(0, 7))
const toApprove = computed(() => (myTasks.value ?? []).filter((tk) => tk.is_per_person && tk.assigned_to_me && !tk.done_by_me).slice(0, 7))
const favoritePages = computed(() => (pages.data.value ?? []).filter((p) => p.is_favorite).slice(0, 8))
const { notifications } = useNotifications()
const unreadNotifs = computed(() => (notifications.value ?? []).filter((n) => !n.read_at).slice(0, 6))
const { data: mailAccounts } = useMailAccounts()
const { members } = useWorkspace()
const onlineMembers = computed(() => (members.data.value ?? []).filter((m) => presence.value[m.user_id] && m.user_id !== user.value?.id))
const { data: notepad } = useQuery({
  queryKey: ['notepad'],
  queryFn: () => api<{ content: string }>('/api/user/notepad', { silent: true }),
  enabled: computed(() => has('notepad')),
  staleTime: 30_000,
})
// Mini-calendrier du mois et événements où je suis mentionné (14 jours).
const monthStart = computed(() => startOfWeek(new Date(new Date().getFullYear(), new Date().getMonth(), 1)))
const monthEnd = computed(() => addDays(monthStart.value, 42))
const { data: monthEvents } = useCalendarEvents(wsId, monthStart, monthEnd)
const monthDays = computed(() => Array.from({ length: 42 }, (_, i) => addDays(monthStart.value, i)))
const dayEvents = (d: Date) => (monthEvents.value?.events ?? []).filter((e) => !hiddenCals.value.has(e.calendar_id) && sameDay(new Date(e.starts_at), d))
const myEvents = computed(() => (monthEvents.value?.events ?? [])
  .filter((e) => e.members?.includes(user.value?.id ?? '') && +new Date(e.ends_at) > Date.now() && +new Date(e.starts_at) < Date.now() + 14 * 86400000)
  .sort((a, b) => +new Date(a.starts_at) - +new Date(b.starts_at)).slice(0, 6))
const notifText = (n: { kind: string; data?: any; actor_name?: string | null }) => `${n.actor_name ?? ''} · ${t(`dashboard.notif.${n.kind}`, {})} ${n.data?.title ?? ''}`.replace(/^ · /, '')
const evTime = (e: { all_day: boolean; starts_at: string }) => {
  if (e.all_day) return t('calendar.allDay')
  const d = new Date(e.starts_at)
  const day = sameDay(d, new Date()) ? '' : `${new Intl.DateTimeFormat(undefined, { weekday: 'short' }).format(d)} `
  return day + new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(d)
}
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: workspace.data.value?.name ?? '', icon: Home }]">
      <UiTooltip :text="t('dashboard.customize')"><UiButton size="sm" variant="ghost" :icon="LayoutGrid" :aria-label="t('dashboard.customize')" @click="customizeOpen = true" /></UiTooltip>
      <UiButton v-if="can.editTasks" size="sm" variant="primary" :icon="Plus" @click="openCreateTask()">{{ t('task.new') }}</UiButton>
    </AppPageHeader>
  
    <div ref="root" class="home">
      <div class="grid">
        <template v-for="w in layout" :key="w.id">
          <section v-if="w.id === 'greeting'" class="hero full" data-reveal>
            <div>
              <p class="date">{{ today }}</p>
              <h1>{{ greeting }}</h1>
            </div>
          </section>

          <section v-else-if="w.id === 'stats'" class="stats full">
            <NuxtLink v-for="(s, i) in stats" :key="s.key" :to="s.to" class="stat" :class="`tone-${s.tone}`" data-reveal>
              <div class="stat-icon"><component :is="s.icon" :size="16" /></div>
              <div class="stat-value tabular" :ref="(el) => { if (el) statEls[i] = el as HTMLElement }">{{ isLoading ? '–' : s.value }}</div>
              <div class="stat-label">{{ s.label }}</div>
            </NuxtLink>
          </section>

          <section v-else-if="w.id === 'agenda'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head">
              <h2><CalendarDays :size="15" /> {{ t('home.upcoming') }}</h2>
              <NuxtLink :to="links.calendar(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink>
            </header>
            <div v-if="!nextEvents.length" class="panel-empty"><CalendarDays :size="20" /><span>{{ t('home.noEvents') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="e in nextEvents" :key="e.id" :to="e.event_id ? `${links.calendar(wsId)}?event=${e.event_id}` : links.calendar(wsId)" class="task-row">
                <span class="ev-bar" :style="{ background: evColor(e.calendar_id) }" />
                <span class="task-title truncate">{{ e.summary }}</span>
                <span v-if="e.location" class="task-board truncate"><MapPin :size="11" /> {{ e.location }}</span>
                <span class="task-due">{{ evTime(e) }}</span>
              </NuxtLink>
            </div>
          </section>

          <section v-else-if="w.id === 'myTasks' || w.id === 'overdue' || w.id === 'toApprove'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head">
              <h2><component :is="w.id === 'overdue' ? AlertTriangle : w.id === 'toApprove' ? BadgeCheck : CircleUserRound" :size="15" /> {{ t(`dashboard.widgets.${w.id}`) }}</h2>
              <NuxtLink :to="links.myTasks(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink>
            </header>
            <div v-if="loadingTasks" class="panel-body"><div v-for="i in 3" :key="i" class="skeleton row-skel" /></div>
            <div v-else-if="!(w.id === 'overdue' ? overdueTasks : w.id === 'toApprove' ? toApprove : upcoming).length" class="panel-empty">
              <CheckCircle2 :size="20" /><span>{{ t(`dashboard.empty.${w.id}`) }}</span>
            </div>
            <div v-else class="panel-body">
              <NuxtLink v-for="tk in (w.id === 'overdue' ? overdueTasks : w.id === 'toApprove' ? toApprove : upcoming)" :key="tk.id" :to="links.task(wsId, tk.board_id, tk.id)" class="task-row">
                <TaskPriorityIcon :priority="tk.priority" :size="14" />
                <span class="task-title truncate">{{ tk.title }}</span>
                <span class="task-board truncate">{{ tk.board_name }} · {{ tk.column_name }}</span>
                <span v-if="tk.due_date" class="task-due" :class="dueClass(tk.due_date)">{{ formatDate(tk.due_date) }}</span>
              </NuxtLink>
            </div>
          </section>

          <section v-else-if="w.id === 'docs'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head">
              <h2><FileText :size="15" /> {{ t('nav.docs') }}</h2>
              <UiSegmented v-model="docTab" :options="[{ value: 'viewed', label: t('home.recentlyViewed') }, { value: 'updated', label: t('home.recentlyUpdated') }]" @update:model-value="touchedTab = true" />
            </header>
            <div v-if="!docList.length" class="panel-empty">
              <FileText :size="20" /><span>{{ t('home.noDocs') }}</span>
              <UiButton v-if="can.editDocs" size="sm" :icon="Plus" @click="createAndOpen()">{{ t('docs.new') }}</UiButton>
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

          <section v-else-if="w.id === 'favorites'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head"><h2><Star :size="15" /> {{ t('dashboard.widgets.favorites') }}</h2></header>
            <div v-if="!favoritePages.length" class="panel-empty"><Star :size="20" /><span>{{ t('dashboard.empty.favorites') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="p in favoritePages" :key="p.id" :to="links.doc(wsId, p.id)" class="doc-row">
                <span class="doc-icon">{{ p.icon || '' }}<FileText v-if="!p.icon" :size="15" /></span>
                <span class="truncate doc-title">{{ p.title || t('docs.untitled') }}</span>
              </NuxtLink>
            </div>
          </section>

          <section v-else-if="w.id === 'inbox'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head">
              <h2><InboxIcon :size="15" /> {{ t('dashboard.widgets.inbox') }}</h2>
              <NuxtLink :to="links.inbox(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink>
            </header>
            <div v-if="!unreadNotifs.length" class="panel-empty"><InboxIcon :size="20" /><span>{{ t('dashboard.empty.inbox') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="n in unreadNotifs" :key="n.id" :to="links.inbox(wsId)" class="task-row">
                <UiAvatar :name="n.actor_name ?? undefined" :size="18" />
                <span class="task-title truncate">{{ notifText(n) }}</span>
                <span class="task-due">{{ relativeTime(n.created_at) }}</span>
              </NuxtLink>
            </div>
          </section>

          <section v-else-if="w.id === 'notepad'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head">
              <h2><NotebookPen :size="15" /> {{ t('notepad.title') }}</h2>
              <button class="panel-link" @click="notepadOpen = true">{{ t('common.open') }} <ArrowRight :size="13" /></button>
            </header>
            <div v-if="!notepad?.content" class="panel-empty"><NotebookPen :size="20" /><span>{{ t('notepad.placeholder') }}</span></div>
            <DocViewer v-else :html="notepad.content" size="sm" class="np-preview" />
          </section>

          <section v-else-if="w.id === 'mail'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head">
              <h2><Mail :size="15" /> {{ t('nav.mail') }}</h2>
              <NuxtLink :to="links.mail(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink>
            </header>
            <div v-if="!mailAccounts?.length" class="panel-empty"><Mail :size="20" /><span>{{ t('dashboard.empty.mail') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="a in mailAccounts" :key="a.id" :to="links.mail(wsId, a.id)" class="task-row">
                <Mail :size="14" class="faint" />
                <span class="task-title truncate">{{ a.label }}</span>
                <span class="task-due" :class="{ soon: a.unread }">{{ a.unread ? t('dashboard.unread', { n: a.unread }) : t('dashboard.upToDate') }}</span>
              </NuxtLink>
            </div>
          </section>

          <section v-else-if="w.id === 'online'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head"><h2><Users :size="15" /> {{ t('dashboard.widgets.online') }}</h2></header>
            <div v-if="!onlineMembers.length" class="panel-empty"><Users :size="20" /><span>{{ t('dashboard.empty.online') }}</span></div>
            <div v-else class="panel-body online">
              <NuxtLink v-for="m in onlineMembers" :key="m.user_id" :to="`/w/${wsId}/activity?user=${m.user_id}`" class="task-row">
                <UiAvatar :name="m.name" :src="m.avatar_url" :size="22" online />
                <span class="task-title truncate">{{ m.name }}</span>
                <span class="task-board truncate">{{ presence[m.user_id]?.location }}</span>
              </NuxtLink>
            </div>
          </section>

          <section v-else-if="w.id === 'month'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head">
              <h2><CalendarRange :size="15" /> {{ formatDate(new Date(), { month: 'long', year: 'numeric' }) }}</h2>
              <NuxtLink :to="links.calendar(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink>
            </header>
            <div class="mini">
              <NuxtLink v-for="d in monthDays" :key="+d" :to="links.calendar(wsId)" class="mini-day" :class="{ out: d.getMonth() !== new Date().getMonth(), today: sameDay(d, new Date()) }">
                <span>{{ d.getDate() }}</span>
                <span class="mini-dots"><i v-for="e in dayEvents(d).slice(0, 3)" :key="e.id" :style="{ background: evColor(e.calendar_id) }" /></span>
              </NuxtLink>
            </div>
          </section>

          <section v-else-if="w.id === 'myEvents'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head"><h2><CalendarHeart :size="15" /> {{ t('dashboard.widgets.myEvents') }}</h2></header>
            <div v-if="!myEvents.length" class="panel-empty"><CalendarHeart :size="20" /><span>{{ t('dashboard.empty.myEvents') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="e in myEvents" :key="e.id" :to="`${links.calendar(wsId)}?event=${e.event_id}`" class="task-row">
                <span class="ev-bar" :style="{ background: evColor(e.calendar_id) }" />
                <span class="task-title truncate">{{ e.summary }}</span>
                <span class="task-due">{{ evTime(e) }}</span>
              </NuxtLink>
            </div>
          </section>

          <section v-else-if="w.id === 'boards'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head">
              <h2><Kanban :size="15" /> {{ t('nav.boards') }}</h2>
              <UiButton v-if="can.manageBoards" size="xs" variant="ghost" :icon="Plus" @click="createBoardOpen = true">{{ t('board.new') }}</UiButton>
            </header>
            <div v-if="!overview?.boards.length" class="panel-empty"><Kanban :size="20" /><span>{{ t('board.noBoards') }}</span></div>
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

          <section v-else-if="w.id === 'activity'" class="panel" :class="w.size" data-reveal>
            <header class="panel-head">
              <h2><Activity :size="15" /> {{ t('home.activity') }}</h2>
              <NuxtLink :to="`/w/${wsId}/activity`" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink>
            </header>
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
        </template>
      </div>
      <div v-if="dashLoaded && !dashSaved" class="customize-hint">
        <LayoutGrid :size="15" /> {{ t('dashboard.notSaved') }}
        <UiButton size="xs" variant="primary" @click="customizeOpen = true">{{ t('dashboard.customize') }}</UiButton>
      </div>
    </div>
    <HomeCustomize v-model:open="customizeOpen" />
  </div>
</template>

<style scoped>
.ev-bar { width: 3px; height: 16px; border-radius: 2px; flex-shrink: 0; }
.home { flex: 1; overflow-y: auto; padding: 32px 40px 64px; }
.home > * { max-width: 1120px; margin-left: auto; margin-right: auto; }
.hero .date { margin: 0; color: var(--text-3); font-size: var(--fs-sm); text-transform: capitalize; }
.hero h1 { margin: 4px 0 0; font-family: var(--font-display); font-size: 26px; font-weight: 600; letter-spacing: -0.022em; }

.stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; }
.grid > .full, .grid > .panel.full { grid-column: 1 / -1; }
.hero { padding: 4px 0 4px; }
.customize-hint { display: flex; align-items: center; justify-content: center; gap: 10px; margin-top: 20px; color: var(--text-3); font-size: var(--fs-sm); }
.np-preview { padding: 0 14px 12px; max-height: 220px; overflow: hidden; }
.mini { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; padding: 0 10px 12px; }
.mini-day { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 4px 0; border-radius: var(--r-sm); font-size: var(--fs-sm); font-variant-numeric: tabular-nums; }
.mini-day:hover { background: var(--bg-hover); }
.mini-day.out { color: var(--text-4); }
.mini-day.today span:first-child { background: var(--accent); color: #fff; border-radius: 999px; padding: 0 6px; }
.mini-dots { display: flex; gap: 2px; height: 5px; }
.mini-dots i { width: 5px; height: 5px; border-radius: 50%; }
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

.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
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
