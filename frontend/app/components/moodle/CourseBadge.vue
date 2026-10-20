<script setup lang="ts">
import type { MoodleCourse } from '~/composables/useMoodle'

/** Pastille d'un cours Moodle (trois lettres sur une couleur tirée de son numéro). */
const props = withDefaults(defineProps<{ course: Pick<MoodleCourse, 'id' | 'shortname' | 'fullname'>; size?: number }>(), { size: 34 })
const hue = computed(() => (props.course.id * 67) % 360)
</script>

<template>
  <span class="cb" :style="{ width: `${size}px`, height: `${size}px`, background: `hsl(${hue} 55% 92%)`, color: `hsl(${hue} 45% 32%)`, fontSize: `${Math.round(size * 0.31)}px` }">
    {{ (course.shortname || course.fullname).slice(0, 3).toUpperCase() }}
  </span>
</template>

<style scoped>
.cb { border-radius: var(--r-md); display: grid; place-items: center; font-weight: 700; flex: none; }
</style>
