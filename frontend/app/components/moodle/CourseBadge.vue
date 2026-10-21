<script setup lang="ts">
import type { MoodleCourse } from '~/composables/useMoodle'
import { useMemberPrefs } from '~/composables/usePrefs'
import { useWorkspaceId } from '~/composables/useWorkspace'
import type { CourseCustom, MoodleCoursesPrefs } from '~/composables/useMoodleCourses'

/**
 * Pastille d'un cours Moodle : trois lettres (ou l'icône choisie) sur la couleur choisie
 * pour ce cours, sinon une couleur tirée de son numéro (personnalisation, GLOSSARY).
 */
const props = withDefaults(defineProps<{ course: Pick<MoodleCourse, 'id' | 'shortname' | 'fullname'>; size?: number; /** Aperçu : réglages à montrer à la place de ceux enregistrés. */ override?: CourseCustom }>(), { size: 34 })
const { prefs } = useMemberPrefs<{ moodleCourses?: MoodleCoursesPrefs }>(useWorkspaceId())
const custom = computed(() => props.override ?? prefs.value?.moodleCourses?.custom?.[props.course.id])
const hue = computed(() => (props.course.id * 67) % 360)
const label = computed(() => custom.value?.icon || (custom.value?.short || props.course.shortname || props.course.fullname).slice(0, 3).toUpperCase())
const style = computed(() => {
  const color = custom.value?.color
  return {
    width: `${props.size}px`, height: `${props.size}px`,
    background: color ? `color-mix(in srgb, ${color} 18%, var(--bg-panel))` : `hsl(${hue.value} 55% 92%)`,
    color: color ? `color-mix(in srgb, ${color} 80%, var(--text))` : `hsl(${hue.value} 45% 32%)`,
    fontSize: `${Math.round(props.size * (custom.value?.icon ? 0.5 : 0.31))}px`,
  }
})
</script>

<template>
  <span class="cb" :style="style">{{ label }}</span>
</template>

<style scoped>
.cb { border-radius: var(--r-md); display: grid; place-items: center; font-weight: 700; flex: none; }
</style>
