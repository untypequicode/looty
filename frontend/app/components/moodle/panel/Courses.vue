<script setup lang="ts">
import { Star } from 'lucide-vue-next'
import { errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useMoodleData, type MoodleCourse } from '~/composables/useMoodle'

/** Panneau « Cours » : les cours Moodle du membre ; un clic ouvre le cours dans Looty. */
const props = defineProps<{ ws: string }>()
const emit = defineEmits<{ (e: 'course', id: number): void }>()
const { t } = useI18n()
const courses = useMoodleData<MoodleCourse[]>(toRef(props, 'ws'), 'courses', ref(true))
</script>

<template>
  <div>
    <div v-if="courses.isLoading.value"><div v-for="i in 4" :key="i" class="skeleton" style="height: 44px; margin: 4px 0" /></div>
    <p v-else-if="courses.error.value" class="mp-err">{{ errorMessage(courses.error.value) }}</p>
    <p v-else-if="!courses.data.value?.length" class="mp-empty">{{ t('moodle.noCourses') }}</p>
    <a v-for="c in courses.data.value" :key="c.id" :href="`?course=${c.id}`" class="crs" @click.prevent="emit('course', c.id)">
      <MoodleCourseBadge :course="c" />
      <span class="crs-body">
        <span class="crs-name truncate">{{ c.fullname }} <Star v-if="c.favourite" :size="11" class="crs-fav" /></span>
        <span v-if="c.category" class="crs-cat">{{ c.category }}</span>
      </span>
      <MoodleProgressBar v-if="c.progress != null" :value="c.progress" />
    </a>
  </div>
</template>

<style scoped>
.crs { display: flex; align-items: center; gap: 10px; padding: 7px 6px; border-radius: var(--r-md); text-decoration: none; color: var(--text); }
.crs:hover { background: var(--bg-hover); }
.crs-body { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.crs-name { font-weight: 500; }
.crs-fav { color: #e0a100; fill: currentColor; vertical-align: -1px; }
.crs-cat { font-size: var(--fs-xs); color: var(--text-3); }
</style>
