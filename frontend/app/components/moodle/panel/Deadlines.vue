<script setup lang="ts">
import { ListPlus } from 'lucide-vue-next'
import { errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useMoodleData, type MoodleDeadline } from '~/composables/useMoodle'
import { useMoodleCourses } from '~/composables/useMoodleCourses'

/** Panneau « Échéances » : devoirs et tests à venir (et en retard), à transformer en tâches. */
const props = defineProps<{ ws: string; /** Montrer aussi les échéances des cours masqués. */ includeHidden?: boolean }>()
const emit = defineEmits<{ (e: 'open', url: string): void; (e: 'task', d: MoodleDeadline): void }>()
const { t, locale, relativeTime } = useI18n()
const deadlines = useMoodleData<MoodleDeadline[]>(toRef(props, 'ws'), 'deadlines', ref(true))
const mc = useMoodleCourses(toRef(props, 'ws'))
const hiddenIds = computed(() => new Set(mc.all.value.filter((c) => c.hidden).map((c) => c.id)))
const list = computed(() => (deadlines.data.value ?? []).filter((d) => props.includeHidden || !hiddenIds.value.has(d.course_id)))
const dayFmt = (d: string) => new Date(d).toLocaleDateString(locale.value, { weekday: 'short', day: 'numeric', month: 'short' })
const timeFmt = (d: string) => new Date(d).toLocaleTimeString(locale.value, { hour: '2-digit', minute: '2-digit' })
const soon = (d: string) => new Date(d).getTime() - Date.now() < 2 * 86400_000
</script>

<template>
  <div>
    <div v-if="deadlines.isLoading.value"><div v-for="i in 3" :key="i" class="skeleton" style="height: 52px; margin: 4px 0" /></div>
    <p v-else-if="deadlines.error.value" class="mp-err">{{ errorMessage(deadlines.error.value) }}</p>
    <p v-else-if="!list.length" class="mp-empty">{{ t('moodle.noDeadlines') }}</p>
    <div v-for="d in list" :key="d.id" class="dl" :class="{ overdue: d.overdue, soon: !d.overdue && soon(d.due) }">
      <div class="dl-date"><b>{{ dayFmt(d.due) }}</b><span>{{ timeFmt(d.due) }}</span></div>
      <div class="dl-body">
        <a :href="d.url" target="_blank" rel="noopener" class="dl-name" @click.prevent="emit('open', d.url)">{{ d.name }}</a>
        <div class="dl-meta">{{ mc.nameOf({ id: d.course_id, fullname: d.course }) }} · <span :class="{ 'dl-late': d.overdue }">{{ d.overdue ? t('moodle.overdue') : relativeTime(d.due) }}</span></div>
      </div>
      <UiTooltip :text="t('moodle.addTask')"><UiButton size="xs" variant="ghost" :icon="ListPlus" :aria-label="t('moodle.addTask')" @click="emit('task', d)" /></UiTooltip>
    </div>
    <p v-if="deadlines.data.value" class="mp-small">{{ t('moodle.calendarNote') }}</p>
  </div>
</template>

<style scoped>
.dl { display: flex; align-items: center; gap: 12px; padding: 8px 6px; border-radius: var(--r-md); }
.dl:hover { background: var(--bg-hover); }
.dl-date { display: flex; flex-direction: column; align-items: center; min-width: 64px; font-size: var(--fs-sm); color: var(--text-2); line-height: 1.25; }
.dl-date span { color: var(--text-3); font-size: var(--fs-xs); }
.dl.soon .dl-date b { color: #c26a00; }
.dl.overdue .dl-date b, .dl-late { color: var(--red); }
.dl-body { flex: 1; min-width: 0; }
.dl-name { color: var(--text); font-weight: 500; text-decoration: none; }
.dl-name:hover { text-decoration: underline; }
.dl-meta { font-size: var(--fs-sm); color: var(--text-3); }
</style>
