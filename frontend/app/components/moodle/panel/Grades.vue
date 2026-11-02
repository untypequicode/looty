<script setup lang="ts">
import { errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useMoodleData, type MoodleGrade } from '~/composables/useMoodle'
import { useMoodleCourses } from '~/composables/useMoodleCourses'

/** Panneau « Notes » : la note globale de chaque cours Moodle qui en a une. */
const props = defineProps<{ ws: string }>()
const emit = defineEmits<{ (e: 'course', id: number): void }>()
const { t } = useI18n()
const ws = toRef(props, 'ws')
const mc = useMoodleCourses(ws)
const grades = useMoodleData<MoodleGrade[]>(ws, 'grades', ref(true))
const rows = computed(() =>
  (grades.data.value ?? [])
    .filter((g) => g.grade)
    .map((g) => ({ g, course: mc.byId(g.course_id) }))
    .filter((r) => r.course)
    .sort((a, b) => mc.nameOf(a.course!).localeCompare(mc.nameOf(b.course!))),
)
</script>

<template>
  <div>
    <div v-if="grades.isLoading.value"><div v-for="i in 4" :key="i" class="skeleton" style="height: 28px; margin: 4px 0" /></div>
    <p v-else-if="grades.error.value" class="mp-err">{{ errorMessage(grades.error.value) }}</p>
    <p v-else-if="!rows.length" class="mp-empty">{{ t('moodle.grades.none') }}</p>
    <button v-for="r in rows" :key="r.g.course_id" class="gr" @click="emit('course', r.g.course_id)">
      <MoodleCourseBadge :course="r.course!" :size="22" />
      <span class="gr-name truncate">{{ mc.nameOf(r.course!) }}</span>
      <span v-if="r.g.rank" class="gr-rank">{{ t('moodle.grades.rank', { rank: r.g.rank }) }}</span>
      <b class="gr-grade">{{ r.g.grade }}</b>
    </button>
  </div>
</template>

<style scoped>
.gr { width: 100%; display: flex; align-items: center; gap: 8px; padding: 5px 4px; border-radius: var(--r-md); text-align: left; color: var(--text); }
.gr:hover { background: var(--bg-hover); }
.gr-name { flex: 1; min-width: 0; font-size: var(--fs-sm); }
.gr-rank { font-size: var(--fs-xs, 11px); color: var(--text-3); }
.gr-grade { font-variant-numeric: tabular-nums; font-size: var(--fs-sm); }
</style>
