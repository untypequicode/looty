<script setup lang="ts">
import { Users, Eye, PencilLine, ListChecks, MessageSquare, UserPlus, FileText, Table2, BarChart3 } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'

/**
 * Statistiques d'usage (instance ou espace) : chiffres clés, activité dans le temps,
 * heures d'activité, pages les plus consultées, personnes les plus actives.
 */
const props = defineProps<{ endpoint: string; instance?: boolean }>()
const { t, relativeTime, dateLocale } = useI18n()
interface Data {
  days: number
  totals: { members: number; active_users: number; views: number; edits: number; task_actions: number; comments: number; sessions: number; new_users: number }
  daily: { day: string; active: number; views: number; actions: number }[]
  heatmap: number[][]
  timezone: string
  top_pages: { id: string; title: string; icon: string | null; workspace_id: string; workspace_name: string; views: number; viewers: number }[]
  top_users: { id: string; name: string; avatar_url: string | null; views: number; actions: number; active_days: number; last_active: string | null }[]
  workspaces: { id: string; name: string; members: number; active: number; views: number; actions: number }[]
}
const days = ref<'7' | '30' | '90'>('30')
const { data, isLoading } = useQuery({
  queryKey: computed(() => ['insights', props.endpoint, days.value]),
  queryFn: () => api<Data>(`${props.endpoint}?days=${days.value}`),
  staleTime: 60_000,
})
const metric = ref<'active' | 'views' | 'actions'>('active')
const series = computed(() => (data.value?.daily ?? []).map((d) => ({ day: d.day, value: d[metric.value] })))
const metricLabel = computed(() => t(`insights.metrics.${metric.value}`))
const tiles = computed(() => {
  const s = data.value?.totals
  return [
    { key: 'active', icon: Users, value: s?.active_users, sub: s ? t('insights.ofMembers', { n: s.members }) : '' },
    { key: 'views', icon: Eye, value: s?.views },
    { key: 'edits', icon: PencilLine, value: s?.edits },
    { key: 'tasks', icon: ListChecks, value: s?.task_actions },
    { key: 'comments', icon: MessageSquare, value: s?.comments },
    { key: 'newUsers', icon: UserPlus, value: s?.new_users },
  ]
})
const weekdays = computed(() => Array.from({ length: 7 }, (_, i) => new Date(2024, 0, 1 + i).toLocaleDateString(dateLocale.value, { weekday: 'short' })))
const maxViews = computed(() => Math.max(1, ...(data.value?.top_pages ?? []).map((p) => p.views)))
const showTable = ref(false)
</script>

<template>
  <div class="ins">
    <div class="ins-bar">
      <UiSegmented v-model="days" size="md" :options="[{ value: '7', label: t('insights.days', { n: 7 }) }, { value: '30', label: t('insights.days', { n: 30 }) }, { value: '90', label: t('insights.days', { n: 90 }) }]" />
    </div>
    <div v-if="isLoading" class="ins-loading"><UiSpinner /></div>
    <template v-else-if="data">
      <div class="tiles">
        <div v-for="tl in tiles" :key="tl.key" class="tile">
          <div class="tile-head"><component :is="tl.icon" :size="14" /> {{ t(`insights.tiles.${tl.key}`) }}</div>
          <div class="tile-value tabular">{{ tl.value ?? 0 }}</div>
          <div v-if="tl.sub" class="tile-sub">{{ tl.sub }}</div>
        </div>
      </div>

      <section class="card">
        <header class="card-head">
          <h3>{{ t('insights.overTime') }}</h3>
          <div class="card-actions">
            <UiSegmented v-model="metric" :options="[{ value: 'active', label: t('insights.metrics.active') }, { value: 'views', label: t('insights.metrics.views') }, { value: 'actions', label: t('insights.metrics.actions') }]" />
            <UiButton size="xs" variant="ghost" :icon="showTable ? BarChart3 : Table2" :aria-label="showTable ? t('insights.showChart') : t('insights.showTable')" :title="showTable ? t('insights.showChart') : t('insights.showTable')" @click="showTable = !showTable" />
          </div>
        </header>
        <AdminAreaChart v-if="!showTable" :points="series" :label="metricLabel" :locale="dateLocale" />
        <div v-else class="tbl-wrap">
          <table class="tbl">
            <thead><tr><th>{{ t('insights.day') }}</th><th>{{ t('insights.metrics.active') }}</th><th>{{ t('insights.metrics.views') }}</th><th>{{ t('insights.metrics.actions') }}</th></tr></thead>
            <tbody><tr v-for="d in [...data.daily].reverse()" :key="d.day"><td>{{ new Date(`${d.day}T12:00:00`).toLocaleDateString(dateLocale) }}</td><td>{{ d.active }}</td><td>{{ d.views }}</td><td>{{ d.actions }}</td></tr></tbody>
          </table>
        </div>
      </section>

      <section class="card">
        <header class="card-head"><h3>{{ t('insights.whenActive') }}</h3><span class="faint small">{{ data.timezone }}</span></header>
        <AdminHeatmap :data="data.heatmap" :days="weekdays" :label="t('insights.heatmapLabel')" :unit="t('insights.actionsUnit')" />
      </section>

      <div class="two">
        <section class="card">
          <header class="card-head"><h3>{{ t('insights.topPages') }}</h3></header>
          <p v-if="!data.top_pages.length" class="faint empty">{{ t('insights.noViews') }}</p>
          <div v-for="p in data.top_pages" :key="p.id" class="bar-row">
            <NuxtLink :to="`/w/${p.workspace_id}/doc/${p.id}`" class="bar-label truncate">
              <span class="bar-icon">{{ p.icon || '' }}<FileText v-if="!p.icon" :size="13" /></span>{{ p.title || t('docs.untitled') }}
              <span v-if="instance" class="faint"> · {{ p.workspace_name }}</span>
            </NuxtLink>
            <span class="bar-track"><span class="bar-fill" :style="{ width: `${(p.views / maxViews) * 100}%` }" /></span>
            <span class="bar-val tabular" :title="t('insights.viewers', { n: p.viewers })">{{ p.views }}</span>
          </div>
        </section>
        <section class="card">
          <header class="card-head"><h3>{{ t('insights.topUsers') }}</h3></header>
          <p v-if="!data.top_users.length" class="faint empty">{{ t('insights.noActivity') }}</p>
          <table v-else class="tbl">
            <thead><tr><th>{{ t('insights.person') }}</th><th>{{ t('insights.metrics.actions') }}</th><th>{{ t('insights.metrics.views') }}</th><th>{{ t('insights.activeDays') }}</th><th>{{ t('insights.lastActive') }}</th></tr></thead>
            <tbody>
              <tr v-for="u in data.top_users" :key="u.id">
                <td><span class="who"><UiAvatar :name="u.name" :src="u.avatar_url" :size="20" /><span class="truncate">{{ u.name }}</span></span></td>
                <td class="tabular">{{ u.actions }}</td><td class="tabular">{{ u.views }}</td><td class="tabular">{{ u.active_days }}</td>
                <td class="faint">{{ relativeTime(u.last_active) }}</td>
              </tr>
            </tbody>
          </table>
        </section>
      </div>

      <section v-if="instance && data.workspaces.length" class="card">
        <header class="card-head"><h3>{{ t('insights.workspaces') }}</h3></header>
        <table class="tbl">
          <thead><tr><th>{{ t('insights.workspace') }}</th><th>{{ t('insights.members') }}</th><th>{{ t('insights.metrics.active') }}</th><th>{{ t('insights.metrics.views') }}</th><th>{{ t('insights.metrics.actions') }}</th></tr></thead>
          <tbody><tr v-for="w in data.workspaces" :key="w.id"><td>{{ w.name }}</td><td class="tabular">{{ w.members }}</td><td class="tabular">{{ w.active }}</td><td class="tabular">{{ w.views }}</td><td class="tabular">{{ w.actions }}</td></tr></tbody>
        </table>
      </section>
    </template>
  </div>
</template>

<style scoped>
.ins { display: flex; flex-direction: column; gap: 14px; }
.ins-bar { display: flex; justify-content: flex-end; }
.ins-loading { display: grid; place-items: center; padding: 48px; }
.tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 10px; }
.tile { padding: 12px 14px; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); }
.tile-head { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); }
.tile-value { font-size: 26px; font-weight: 600; letter-spacing: -.02em; margin-top: 6px; color: var(--text); }
.tile-sub { font-size: var(--fs-xs); color: var(--text-3); margin-top: 2px; }
.card { padding: 12px 16px 16px; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); min-width: 0; }
.card-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 10px; flex-wrap: wrap; }
.card-head h3 { margin: 0; font-size: var(--fs-md); font-weight: 600; }
.card-actions { display: flex; align-items: center; gap: 6px; }
.two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
.bar-row { display: grid; grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr) 40px; align-items: center; gap: 10px; height: 30px; }
.bar-label { font-size: var(--fs-sm); color: var(--text); display: flex; align-items: center; gap: 6px; }
.bar-icon { color: var(--text-3); display: inline-grid; place-items: center; width: 16px; }
.bar-track { height: 8px; }
.bar-fill { display: block; height: 100%; border-radius: 0 4px 4px 0; background: #2a78d6; min-width: 2px; }
:global(html.dark) .bar-fill { background: #3987e5; }
.bar-val { text-align: right; font-size: var(--fs-sm); color: var(--text-2); }
.tbl-wrap { max-height: 260px; overflow-y: auto; }
.tbl { width: 100%; border-collapse: collapse; font-size: var(--fs-sm); }
.tbl th { text-align: left; font-weight: 500; color: var(--text-3); padding: 6px 8px; border-bottom: 1px solid var(--border); }
.tbl td { padding: 6px 8px; border-bottom: 1px solid var(--border); color: var(--text); }
.tbl tr:last-child td { border-bottom: none; }
.who { display: flex; align-items: center; gap: 8px; min-width: 0; }
.empty { margin: 8px 0; font-size: var(--fs-sm); }
.small { font-size: var(--fs-xs); }
@media (max-width: 900px) { .two { grid-template-columns: 1fr; } }
</style>
