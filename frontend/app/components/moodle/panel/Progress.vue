<script setup lang="ts">
import { errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useMoodleData, type MoodleCourse } from '~/composables/useMoodle'

/** Panneau « Progression » : l'avancement de chaque cours suivi, du moins avancé au plus avancé. */
const props = defineProps<{ ws: string }>()
const emit = defineEmits<{ (e: 'course', id: number): void }>()
const { t } = useI18n()
const courses = useMoodleData<MoodleCourse[]>(toRef(props, 'ws'), 'courses', ref(true))
const tracked = computed(() => (courses.data.value ?? []).filter((c) => c.progress != null).sort((a, b) => a.progress! - b.progress!))
</script>

<template>
  <div>
    <div v-if="courses.isLoading.value"><div v-for="i in 4" :key="i" class="skeleton" style="height: 28px; margin: 4px 0" /></div>
    <p v-else-if="courses.error.value" class="mp-err">{{ errorMessage(courses.error.value) }}</p>
    <p v-else-if="!tracked.length" class="mp-empty">{{ t('moodle.board.noProgress') }}</p>
    <button v-for="c in tracked" :key="c.id" class="pr" @click="emit('course', c.id)">
      <MoodleCourseBadge :course="c" :size="22" />
      <span class="pr-name truncate">{{ c.fullname }}</span>
      <MoodleProgressBar :value="c.progress!" :width="80" />
    </button>
  </div>
</template>

<style scoped>
.pr { width: 100%; display: flex; align-items: center; gap: 8px; padding: 5px 4px; border-radius: var(--r-md); text-align: left; color: var(--text); }
.pr:hover { background: var(--bg-hover); }
.pr-name { flex: 1; min-width: 0; font-size: var(--fs-sm); }
</style>
