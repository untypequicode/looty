<script setup lang="ts">
import { GraduationCap, ChevronRight } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import type { MoodleCourseGrades } from '~/composables/useMoodle'

/** Carnet de notes d'un cours Moodle, lu à la demande (replié par défaut). */
const props = defineProps<{ ws: string; courseId: number }>()
const { t, formatDate } = useI18n()
const open = ref(false)
const { data, isLoading, error } = useQuery({
  queryKey: computed(() => ['moodle', props.ws, 'grades', props.courseId]),
  queryFn: () => api<MoodleCourseGrades>(`/api/workspaces/${props.ws}/moodle/courses/${props.courseId}/grades`, { silent: true }),
  enabled: open,
  staleTime: 120_000,
  retry: false,
})
const outOf = (i: { min: number | null; max: number | null }) => (i.max != null ? ` / ${Number(i.max).toLocaleString()}` : '')
</script>

<template>
  <section class="mc-sec cg">
    <button class="cg-head" :aria-expanded="open" @click="open = !open">
      <GraduationCap :size="16" class="cg-ic" />
      <span class="cg-title">{{ t('moodle.grades.title') }}</span>
      <b v-if="data?.total?.grade" class="cg-total">{{ data.total.grade }}{{ outOf(data.total) }}</b>
      <ChevronRight :size="15" class="cg-chev" :class="{ open }" />
    </button>
    <div v-if="open" class="cg-body">
      <div v-if="isLoading"><div v-for="i in 3" :key="i" class="skeleton" style="height: 24px; margin: 4px 0" /></div>
      <p v-else-if="error" class="md-err">{{ errorMessage(error) }}</p>
      <p v-else-if="!data?.items.some((i) => i.grade)" class="muted">{{ t('moodle.grades.noneInCourse') }}</p>
      <template v-else>
        <div v-for="i in data!.items" :key="i.id" class="cg-item" :class="{ cat: i.type === 'category' }">
          <div class="cg-row">
            <span class="cg-name truncate">{{ i.name ?? t('moodle.grades.untitled') }}</span>
            <span v-if="i.graded_at" class="cg-date">{{ formatDate(i.graded_at) }}</span>
            <span v-if="i.percentage" class="cg-pct">{{ i.percentage }}</span>
            <b class="cg-grade">{{ i.grade ?? '–' }}<span v-if="i.grade" class="cg-max">{{ outOf(i) }}</span></b>
          </div>
          <!-- eslint-disable-next-line vue/no-v-html : HTML nettoyé côté serveur -->
          <div v-if="i.feedback" class="cg-feedback" v-html="i.feedback" />
        </div>
      </template>
    </div>
  </section>
</template>

<style scoped>
.cg { padding: 4px 10px; }
.cg-head { display: flex; align-items: center; gap: 10px; width: 100%; padding: 8px 4px; text-align: left; border-radius: var(--r-sm); }
.cg-head:hover { background: var(--bg-hover); }
.cg-ic { color: var(--text-3); flex: none; }
.cg-title { flex: 1; font-weight: 600; }
.cg-total { font-variant-numeric: tabular-nums; }
.cg-chev { color: var(--text-3); transition: transform var(--dur-fast); }
.cg-chev.open { transform: rotate(90deg); }
.cg-body { padding: 2px 4px 8px; }
.cg-item { padding: 6px 0; border-top: 1px solid var(--border); }
.cg-item.cat .cg-name { font-weight: 600; }
.cg-row { display: flex; align-items: center; gap: 10px; font-size: var(--fs-sm); }
.cg-name { flex: 1; min-width: 0; }
.cg-date, .cg-pct { color: var(--text-3); font-size: var(--fs-xs, 11px); }
.cg-grade { font-variant-numeric: tabular-nums; min-width: 64px; text-align: right; }
.cg-max { color: var(--text-3); font-weight: 400; }
.cg-feedback { margin-top: 4px; font-size: var(--fs-sm); color: var(--text-2); }
.cg-feedback :deep(p) { margin: 0; }
</style>
