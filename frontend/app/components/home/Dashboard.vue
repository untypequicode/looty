<script setup lang="ts">
import { useCalendars, useCalendarEvents, useCalendarPrefs, startOfDay, addDays, sameDay } from '~/composables/useCalendar'
import { Home, CircleUserRound, AlertTriangle, CalendarClock, CheckCircle2, FileText, Kanban, ArrowRight, Plus, Activity, CalendarDays, MapPin, LayoutGrid, Star, Inbox as InboxIcon, NotebookPen, Users, Mail, BadgeCheck, CalendarHeart, CalendarRange, GraduationCap, Clock, BookOpenText, GripVertical, X } from 'lucide-vue-next'
import { useTodayClasses, useClassLinks } from '~/composables/useClasses'
import { useMemberPrefs } from '~/composables/usePrefs'
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
const dash = useDashboard(wsId)
const { layout } = dash
const customizeOpen = ref(false)
const has = (id: string) => layout.value.some((w) => w.i === id)

// --- Cours (emploi du temps relié aux documents) ---
const { current: classNow, next: classNext, now: clock } = useTodayClasses(wsId)
const { links: classLinks } = useClassLinks(wsId)
const { prefs: dashPrefs } = useMemberPrefs<{ dashboard?: { knowsClasses?: boolean } }>(wsId)
/** Ajouté volontairement : visible même sans cours (indication pour le configurer). */
const classesChosen = computed(() => !!dashPrefs.value?.dashboard?.knowsClasses)
const showClasses = computed(() => classNow.value.length > 0 || classNext.value.length > 0 || classesChosen.value)
/** Les widgets placés sur la grille (celui des cours n'a pas de case tant qu'il n'a rien à montrer). */
const shown = computed(() => layout.value.filter((w) => w.i !== 'classes' || showClasses.value))
const hm = (iso: string) => new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(new Date(iso))
const classProgress = (e: { starts_at: string; ends_at: string }) => {
  const s = +new Date(e.starts_at), en = +new Date(e.ends_at)
  return Math.min(1, Math.max(0, (+clock.value - s) / (en - s)))
}
const minutesLeft = (e: { ends_at: string }) => Math.max(0, Math.round((+new Date(e.ends_at) - +clock.value) / 60000))
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
      <AppPanelGrid :items="shown" :row-height="28" @change="dash.move">
        <template #default="{ item: w, stacked }">
          <section v-if="w.i === 'greeting'" class="bare hero" data-reveal>
            <template v-if="!stacked"><span class="pg-handle bare-grip" :title="t('panelGrid.move')"><GripVertical :size="14" /></span><button class="bare-x" :aria-label="t('panelGrid.remove')" :title="t('panelGrid.remove')" @click="dash.remove(w.i)"><X :size="14" /></button></template>
            <div>
              <p class="date">{{ today }}</p>
              <h1>{{ greeting }}</h1>
            </div>
          </section>

          <section v-else-if="w.i === 'classes'" class="bare classes" data-reveal>
            <template v-if="!stacked"><span class="pg-handle bare-grip" :title="t('panelGrid.move')"><GripVertical :size="14" /></span><button class="bare-x" :aria-label="t('panelGrid.remove')" :title="t('panelGrid.remove')" @click="dash.remove(w.i)"><X :size="14" /></button></template>
            <article v-for="c in classNow" :key="c.id + c.starts_at" class="class-now" :style="{ '--cc': c.color || 'var(--accent)' }">
              <div class="cn-head">
                <span class="cn-badge"><GraduationCap :size="13" /> {{ t('classes.now') }}</span>
                <span class="cn-time"><Clock :size="12" /> {{ hm(c.starts_at) }} – {{ hm(c.ends_at) }} · {{ t('classes.left', { n: minutesLeft(c) }) }}</span>
              </div>
              <h2 class="cn-title">{{ c.summary }}</h2>
              <p v-if="c.location" class="cn-loc"><MapPin :size="12" /> {{ c.location }}</p>
              <div class="cn-progress"><span :style="{ transform: `scaleX(${classProgress(c)})` }" /></div>
              <div class="cn-docs">
                <div v-for="d in c.documents" :key="d.link_id" class="cn-doc">
                  <NuxtLink :to="links.doc(wsId, d.id)" class="cn-doc-main"><span class="cn-ic">{{ d.icon || '📄' }}</span><b class="truncate">{{ d.title || t('docs.untitled') }}</b></NuxtLink>
                  <NuxtLink :to="links.read(wsId, d.id)" class="cn-read" :title="t('reader.open')"><BookOpenText :size="13" /></NuxtLink>
                  <div v-if="d.children.length" class="cn-sub">
                    <NuxtLink v-for="s in d.children" :key="s.id" :to="links.doc(wsId, s.id)" class="cn-subpage" :style="{ paddingLeft: `${(s.depth - 1) * 12 + 8}px` }">
                      <span>{{ s.icon || '·' }}</span><span class="truncate">{{ s.title || t('docs.untitled') }}</span>
                    </NuxtLink>
                  </div>
                </div>
              </div>
            </article>
            <div v-if="classNext.length" class="class-next">
              <h3>{{ classNow.length ? t('classes.after') : t('classes.today') }}</h3>
              <div v-for="c in classNext" :key="c.id + c.starts_at" class="cx-row">
                <span class="cx-bar" :style="{ background: c.color || 'var(--accent)' }" />
                <span class="cx-time tabular">{{ hm(c.starts_at) }}</span>
                <span class="cx-title truncate">{{ c.summary }}</span>
                <span v-if="c.location" class="cx-loc truncate"><MapPin :size="11" /> {{ c.location }}</span>
                <span class="cx-docs">
                  <NuxtLink v-for="d in c.documents" :key="d.link_id" :to="links.doc(wsId, d.id)" class="cx-doc">{{ d.icon || '📄' }} <span class="truncate">{{ d.title || t('docs.untitled') }}</span></NuxtLink>
                </span>
              </div>
            </div>
            <div v-if="!classNow.length && !classNext.length" class="class-empty">
              <GraduationCap :size="18" />
              <span v-if="classLinks?.length">{{ t('classes.noneToday') }}</span>
              <span v-else>{{ t('classes.setupHint') }} <NuxtLink :to="links.calendar(wsId)">{{ t('classes.setupLink') }}</NuxtLink></span>
            </div>
          </section>

          <section v-else-if="w.i === 'stats'" class="bare stats">
            <template v-if="!stacked"><span class="pg-handle bare-grip" :title="t('panelGrid.move')"><GripVertical :size="14" /></span><button class="bare-x" :aria-label="t('panelGrid.remove')" :title="t('panelGrid.remove')" @click="dash.remove(w.i)"><X :size="14" /></button></template>
            <NuxtLink v-for="(s, i) in stats" :key="s.key" :to="s.to" class="stat" :class="`tone-${s.tone}`" data-reveal>
              <div class="stat-icon"><component :is="s.icon" :size="16" /></div>
              <div class="stat-value tabular" :ref="(el) => { if (el) statEls[i] = el as HTMLElement }">{{ isLoading ? '–' : s.value }}</div>
              <div class="stat-label">{{ s.label }}</div>
            </NuxtLink>
          </section>

          <AppGridPanel v-else-if="w.i === 'agenda'" :title="t('home.upcoming')" :icon="CalendarDays" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
              <template #actions><NuxtLink :to="links.calendar(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink></template>
            <div v-if="!nextEvents.length" class="panel-empty"><CalendarDays :size="20" /><span>{{ t('home.noEvents') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="e in nextEvents" :key="e.id" :to="e.event_id ? `${links.calendar(wsId)}?event=${e.event_id}` : links.calendar(wsId)" class="task-row">
                <span class="ev-bar" :style="{ background: evColor(e.calendar_id) }" />
                <span class="task-title truncate">{{ e.summary }}</span>
                <span v-if="e.location" class="task-board truncate"><MapPin :size="11" /> {{ e.location }}</span>
                <span class="task-due">{{ evTime(e) }}</span>
              </NuxtLink>
            </div>
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'myTasks' || w.i === 'overdue' || w.i === 'toApprove'" :title="t(`dashboard.widgets.${w.i}`)" :icon="w.i === 'overdue' ? AlertTriangle : w.i === 'toApprove' ? BadgeCheck : CircleUserRound" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
              <template #actions><NuxtLink :to="links.myTasks(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink></template>
            <div v-if="loadingTasks" class="panel-body"><div v-for="i in 3" :key="i" class="skeleton row-skel" /></div>
            <div v-else-if="!(w.i === 'overdue' ? overdueTasks : w.i === 'toApprove' ? toApprove : upcoming).length" class="panel-empty">
              <CheckCircle2 :size="20" /><span>{{ t(`dashboard.empty.${w.i}`) }}</span>
            </div>
            <div v-else class="panel-body">
              <NuxtLink v-for="tk in (w.i === 'overdue' ? overdueTasks : w.i === 'toApprove' ? toApprove : upcoming)" :key="tk.id" :to="links.task(wsId, tk.board_id, tk.id)" class="task-row">
                <TaskPriorityIcon :priority="tk.priority" :size="14" />
                <span class="task-title truncate">{{ tk.title }}</span>
                <span class="task-board truncate">{{ tk.board_name }} · {{ tk.column_name }}</span>
                <span v-if="tk.due_date" class="task-due" :class="dueClass(tk.due_date)">{{ formatDate(tk.due_date) }}</span>
              </NuxtLink>
            </div>
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'docs'" :title="t('nav.docs')" :icon="FileText" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
              <template #actions><UiSegmented v-model="docTab" :options="[{ value: 'viewed', label: t('home.recentlyViewed') }, { value: 'updated', label: t('home.recentlyUpdated') }]" @update:model-value="touchedTab = true" /></template>
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
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'favorites'" :title="t('dashboard.widgets.favorites')" :icon="Star" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
            <div v-if="!favoritePages.length" class="panel-empty"><Star :size="20" /><span>{{ t('dashboard.empty.favorites') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="p in favoritePages" :key="p.id" :to="links.doc(wsId, p.id)" class="doc-row">
                <span class="doc-icon">{{ p.icon || '' }}<FileText v-if="!p.icon" :size="15" /></span>
                <span class="truncate doc-title">{{ p.title || t('docs.untitled') }}</span>
              </NuxtLink>
            </div>
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'inbox'" :title="t('dashboard.widgets.inbox')" :icon="InboxIcon" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
              <template #actions><NuxtLink :to="links.inbox(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink></template>
            <div v-if="!unreadNotifs.length" class="panel-empty"><InboxIcon :size="20" /><span>{{ t('dashboard.empty.inbox') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="n in unreadNotifs" :key="n.id" :to="links.inbox(wsId)" class="task-row">
                <UiAvatar :name="n.actor_name ?? undefined" :size="18" />
                <span class="task-title truncate">{{ notifText(n) }}</span>
                <span class="task-due">{{ relativeTime(n.created_at) }}</span>
              </NuxtLink>
            </div>
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'notepad'" :title="t('notepad.title')" :icon="NotebookPen" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
              <template #actions><button class="panel-link" @click="notepadOpen = true">{{ t('common.open') }} <ArrowRight :size="13" /></button></template>
            <div v-if="!notepad?.content" class="panel-empty"><NotebookPen :size="20" /><span>{{ t('notepad.placeholder') }}</span></div>
            <DocViewer v-else :html="notepad.content" size="sm" class="np-preview" />
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'mail'" :title="t('nav.mail')" :icon="Mail" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
              <template #actions><NuxtLink :to="links.mail(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink></template>
            <div v-if="!mailAccounts?.length" class="panel-empty"><Mail :size="20" /><span>{{ t('dashboard.empty.mail') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="a in mailAccounts" :key="a.id" :to="links.mail(wsId, a.id)" class="task-row">
                <Mail :size="14" class="faint" />
                <span class="task-title truncate">{{ a.label }}</span>
                <span class="task-due" :class="{ soon: a.unread }">{{ a.unread ? t('dashboard.unread', { n: a.unread }) : t('dashboard.upToDate') }}</span>
              </NuxtLink>
            </div>
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'online'" :title="t('dashboard.widgets.online')" :icon="Users" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
            <div v-if="!onlineMembers.length" class="panel-empty"><Users :size="20" /><span>{{ t('dashboard.empty.online') }}</span></div>
            <div v-else class="panel-body online">
              <NuxtLink v-for="m in onlineMembers" :key="m.user_id" :to="`/w/${wsId}/activity?user=${m.user_id}`" class="task-row">
                <UiAvatar :name="m.name" :src="m.avatar_url" :size="22" online />
                <span class="task-title truncate">{{ m.name }}</span>
                <span class="task-board truncate">{{ presence[m.user_id]?.location }}</span>
              </NuxtLink>
            </div>
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'month'" :title="formatDate(new Date(), { month: 'long', year: 'numeric' })" :icon="CalendarRange" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
              <template #actions><NuxtLink :to="links.calendar(wsId)" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink></template>
            <div class="mini">
              <NuxtLink v-for="d in monthDays" :key="+d" :to="links.calendar(wsId)" class="mini-day" :class="{ out: d.getMonth() !== new Date().getMonth(), today: sameDay(d, new Date()) }">
                <span>{{ d.getDate() }}</span>
                <span class="mini-dots"><i v-for="e in dayEvents(d).slice(0, 3)" :key="e.id" :style="{ background: evColor(e.calendar_id) }" /></span>
              </NuxtLink>
            </div>
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'myEvents'" :title="t('dashboard.widgets.myEvents')" :icon="CalendarHeart" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
            <div v-if="!myEvents.length" class="panel-empty"><CalendarHeart :size="20" /><span>{{ t('dashboard.empty.myEvents') }}</span></div>
            <div v-else class="panel-body">
              <NuxtLink v-for="e in myEvents" :key="e.id" :to="`${links.calendar(wsId)}?event=${e.event_id}`" class="task-row">
                <span class="ev-bar" :style="{ background: evColor(e.calendar_id) }" />
                <span class="task-title truncate">{{ e.summary }}</span>
                <span class="task-due">{{ evTime(e) }}</span>
              </NuxtLink>
            </div>
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'boards'" :title="t('nav.boards')" :icon="Kanban" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
              <template #actions><UiButton v-if="can.manageBoards" size="xs" variant="ghost" :icon="Plus" @click="createBoardOpen = true">{{ t('board.new') }}</UiButton></template>
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
          </AppGridPanel>

          <AppGridPanel v-else-if="w.i === 'activity'" :title="t('home.activity')" :icon="Activity" :fixed="stacked" data-reveal @remove="dash.remove(w.i)">
              <template #actions><NuxtLink :to="`/w/${wsId}/activity`" class="panel-link">{{ t('common.viewAll') }} <ArrowRight :size="13" /></NuxtLink></template>
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
          </AppGridPanel>
        </template>
      </AppPanelGrid>
    </div>
    <HomeCustomize v-model:open="customizeOpen" />
  </div>
</template>

<style scoped>
.classes { display: flex; flex-direction: column; gap: 12px; }
.class-now { position: relative; padding: 16px 18px; border-radius: var(--r-lg); background: color-mix(in srgb, var(--cc) 9%, var(--bg-elevated)); box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--cc) 55%, transparent), var(--shadow-card); }
.cn-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; flex-wrap: wrap; }
.cn-badge { display: inline-flex; align-items: center; gap: 5px; padding: 2px 9px; border-radius: var(--r-full); background: var(--cc); color: #fff; font-size: var(--fs-xs); font-weight: 600; text-transform: uppercase; letter-spacing: .03em; }
.cn-time { display: inline-flex; align-items: center; gap: 5px; color: var(--text-2); font-size: var(--fs-sm); font-variant-numeric: tabular-nums; }
.cn-title { margin: 10px 0 2px; font-size: 22px; font-weight: 700; letter-spacing: -.01em; }
.cn-loc { display: flex; align-items: center; gap: 5px; margin: 0; color: var(--text-3); font-size: var(--fs-sm); }
.cn-progress { height: 3px; margin: 12px 0; border-radius: 2px; background: color-mix(in srgb, var(--cc) 18%, transparent); overflow: hidden; }
.cn-progress span { display: block; height: 100%; background: var(--cc); transform-origin: left; transition: transform 1s linear; }
.cn-docs { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 8px; }
.cn-doc { position: relative; padding: 8px; border-radius: var(--r-md); background: var(--bg-elevated); box-shadow: 0 0 0 1px var(--border); }
.cn-doc-main { display: flex; align-items: center; gap: 8px; padding: 4px 26px 4px 4px; border-radius: var(--r-sm); color: var(--text); min-width: 0; }
.cn-doc-main:hover { background: var(--bg-hover); }
.cn-ic { font-size: 16px; }
.cn-read { position: absolute; top: 10px; right: 8px; color: var(--text-3); padding: 3px; border-radius: var(--r-sm); }
.cn-read:hover { color: var(--text); background: var(--bg-hover); }
.cn-sub { display: flex; flex-direction: column; margin-top: 4px; max-height: 220px; overflow-y: auto; }
.cn-subpage { display: flex; align-items: center; gap: 6px; padding-top: 3px; padding-bottom: 3px; padding-right: 6px; border-radius: var(--r-sm); color: var(--text-2); font-size: var(--fs-sm); min-width: 0; }
.cn-subpage:hover { background: var(--bg-hover); color: var(--text); }
.class-next { padding: 10px 14px; border-radius: var(--r-lg); background: var(--bg-elevated); box-shadow: var(--shadow-card); }
.class-next h3 { margin: 0 0 6px; font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); }
.cx-row { display: flex; align-items: center; gap: 10px; min-height: 32px; font-size: var(--fs-sm); }
.cx-bar { flex: none; width: 3px; height: 18px; border-radius: 2px; }
.cx-time { flex: none; min-width: 44px; white-space: nowrap; color: var(--text-2); }
.cx-title { flex: 1; min-width: 0; font-weight: 500; }
.cx-loc { flex: none; max-width: 160px; display: inline-flex; align-items: center; gap: 4px; color: var(--text-3); }
.cx-docs { display: flex; gap: 6px; flex: none; max-width: 45%; overflow: hidden; }
.cx-doc { display: inline-flex; align-items: center; gap: 4px; max-width: 200px; padding: 1px 8px; border-radius: var(--r-full); background: var(--bg-hover); color: var(--text-2); }
.cx-doc:hover { color: var(--text); }
.class-empty { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-radius: var(--r-lg); background: var(--bg-elevated); color: var(--text-3); font-size: var(--fs-sm); box-shadow: var(--shadow-card); }
.class-empty a { color: var(--accent-text); }
@media (max-width: 639px) { .cx-loc, .cx-docs { display: none; } }
.ev-bar { width: 3px; height: 16px; border-radius: 2px; flex-shrink: 0; }
.home { flex: 1; overflow-y: auto; padding: 32px 40px 64px; }
.home > * { max-width: 1120px; margin-left: auto; margin-right: auto; }
.hero .date { margin: 0; color: var(--text-3); font-size: var(--fs-sm); text-transform: capitalize; }
.hero h1 { margin: 4px 0 0; font-family: var(--font-display); font-size: 26px; font-weight: 600; letter-spacing: -0.022em; }

.bare { position: relative; height: 100%; overflow-y: auto; }
.bare-grip, .bare-x { position: absolute; top: 2px; opacity: 0; transition: opacity var(--dur-fast); color: var(--text-3); z-index: 1; }
.bare-grip { right: 30px; cursor: grab; padding: 4px; }
.bare-x { right: 4px; padding: 4px; border-radius: var(--r-xs); }
.bare-x:hover { color: var(--text); background: var(--bg-hover); }
.bare:hover .bare-grip, .bare:hover .bare-x { opacity: 1; }
@media (pointer: coarse) { .bare-grip, .bare-x { opacity: .6; } }
.stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(130px, 1fr)); gap: 12px; align-content: stretch; }
.hero { padding: 4px 0 4px; }
.np-preview { padding: 0; max-height: 220px; overflow: hidden; }
.mini { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; }
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

.panel-link { display: inline-flex; align-items: center; gap: 4px; color: var(--text-3); font-size: var(--fs-sm); }
.panel-link:hover { color: var(--text); }
.panel-body { padding: 0; }
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

.boards { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 8px; padding: 0 0 4px; }
.board-card { padding: 12px; border-radius: var(--r-md); background: var(--bg-card); box-shadow: var(--shadow-card); transition: background var(--dur-fast), transform var(--dur) var(--ease-out); }
.board-card:hover { background: var(--bg-card-hover); transform: translateY(-1px); }
.board-top { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.board-name { font-weight: 600; }
.board-pct { color: var(--text-3); font-size: var(--fs-sm); }
.bar { height: 4px; border-radius: 4px; background: var(--bg-active); margin: 10px 0 8px; overflow: hidden; }
.bar-fill { height: 100%; border-radius: 4px; background: linear-gradient(90deg, var(--accent), #b18cff); transition: width 0.8s var(--ease-out); }
.board-meta { font-size: var(--fs-sm); color: var(--text-3); }

.feed { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; }
.feed-item { display: flex; align-items: flex-start; gap: 10px; padding: 7px 0; border-bottom: 1px solid var(--border); }
.feed-item:last-child { border-bottom: none; }
.feed-text { flex: 1; min-width: 0; color: var(--text-2); line-height: 1.45; }
.feed-text strong { color: var(--text); font-weight: 500; }
.feed-link { color: var(--text); font-weight: 500; }
.feed-link:hover { text-decoration: underline; }
.feed-time { color: var(--text-3); font-size: var(--fs-sm); flex: none; }

@media (max-width: 1023px) {
  .home { padding: 24px 20px 48px; }
}
@media (max-width: 639px) {
  .home { padding: 18px 14px 32px; }
  .stats { gap: 8px; }
  .hero h1 { font-size: 22px; }
  .task-board { display: none; }
}
</style>
