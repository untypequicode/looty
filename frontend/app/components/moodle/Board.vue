<script setup lang="ts">
import { Maximize2, Replace } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { useMoodleData, type MoodleCourse, type MoodleDeadline } from '~/composables/useMoodle'
import { useMoodleBoard, PANEL_ICONS, type BoardPanel } from '~/composables/useMoodleBoard'

/**
 * Tableau de bord Moodle (GLOSSARY) : les panneaux du membre sur une grille qu'il dispose
 * librement. Chaque changement est enregistré tout de suite.
 */
const props = defineProps<{ ws: string }>()
const emit = defineEmits<{
  (e: 'open', url: string): void
  (e: 'course', id: number): void
  (e: 'conversation', id: number): void
  (e: 'task', d: MoodleDeadline): void
}>()
const { t } = useI18n()
const ws = toRef(props, 'ws')
const { panels, move, patch, remove } = useMoodleBoard(ws)
const courses = useMoodleData<MoodleCourse[]>(ws, 'courses', ref(true))
const courseOf = (id?: number) => (id ? courses.data.value?.find((c) => c.id === id) ?? null : null)

function titleOf(p: BoardPanel) {
  if (p.title) return p.title
  if (p.kind === 'course' && p.course) return courseOf(p.course)?.fullname ?? t('moodle.board.kinds.course')
  return t(`moodle.board.kinds.${p.kind}`)
}
</script>

<template>
  <AppPanelGrid class="mb" :items="panels" @change="move">
    <template #default="{ item: p, stacked }">
      <AppGridPanel
        :title="titleOf(p)"
        :icon="PANEL_ICONS[p.kind]"
        :fixed="stacked"
        renamable
        :menu="p.kind === 'course' && p.course ? [{ label: t('moodle.board.changeCourse'), icon: Replace, onSelect: () => patch(p.i, { course: undefined }) }] : []"
        @rename="(title) => patch(p.i, { title: title ?? undefined })"
        @remove="remove(p.i)"
      >
        <template v-if="p.kind === 'course' && p.course" #actions>
          <UiTooltip :text="t('moodle.board.openCourse')"><UiButton size="xs" variant="ghost" :icon="Maximize2" :aria-label="t('moodle.board.openCourse')" @click="emit('course', p.course)" /></UiTooltip>
        </template>
        <MoodlePanelDeadlines v-if="p.kind === 'deadlines'" :ws="ws" @open="emit('open', $event)" @task="emit('task', $event)" />
        <MoodlePanelCourses v-else-if="p.kind === 'courses'" :ws="ws" @course="emit('course', $event)" />
        <MoodlePanelMessages v-else-if="p.kind === 'messages'" :ws="ws" @conversation="emit('conversation', $event)" />
        <MoodlePanelNotifications v-else-if="p.kind === 'notifications'" :ws="ws" @open="emit('open', $event)" />
        <MoodlePanelProgress v-else-if="p.kind === 'progress'" :ws="ws" @course="emit('course', $event)" />
        <template v-else-if="p.kind === 'course'">
          <MoodleCourse v-if="p.course" :ws="ws" :course-id="p.course" :course="courseOf(p.course)" embedded @open="emit('open', $event)" />
          <div v-else class="mb-pick">
            <p class="mp-small">{{ t('moodle.board.pickCourse') }}</p>
            <button v-for="c in courses.data.value" :key="c.id" class="mb-course" @click="patch(p.i, { course: c.id })">
              <MoodleCourseBadge :course="c" :size="24" /><span class="truncate">{{ c.fullname }}</span>
            </button>
          </div>
        </template>
      </AppGridPanel>
    </template>
  </AppPanelGrid>
</template>

<style scoped>
.mb :deep(.mp-empty) { padding: 12px 0; margin: 0; font-size: var(--fs-sm); color: var(--text-3); }
.mb :deep(.mp-err) { color: var(--red); font-size: var(--fs-sm); margin: 0; }
.mb :deep(.mp-small) { font-size: var(--fs-sm); color: var(--text-3); margin: 4px 0 0; }
.mb-pick { display: flex; flex-direction: column; gap: 2px; }
.mb-course { display: flex; align-items: center; gap: 8px; padding: 5px 4px; border-radius: var(--r-md); text-align: left; color: var(--text); font-size: var(--fs-sm); }
.mb-course:hover { background: var(--bg-hover); }
</style>
