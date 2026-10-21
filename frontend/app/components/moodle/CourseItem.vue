<script setup lang="ts">
import { Star, MoreHorizontal, EyeOff } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useI18n } from '~/composables/useI18n'
import type { MoodleCourse } from '~/composables/useMoodle'
import type { CourseCustom } from '~/composables/useMoodleCourses'
import type { CoursesView } from '~/composables/useMoodleBoard'

/**
 * Un cours Moodle dans une liste du tableau de bord : carte (image du cours), ligne ou
 * ligne compacte. Le clic ouvre le cours ; le menu regroupe les actions (étoile, masquer,
 * personnaliser, déplacer vers un dossier).
 */
const props = defineProps<{ course: MoodleCourse; view: CoursesView; name: string; custom: CourseCustom; menu: MenuItem[]; draggable?: boolean }>()
const emit = defineEmits<{ (e: 'open'): void; (e: 'favourite'): void; (e: 'dragstart'): void; (e: 'dragend'): void; (e: 'drop'): void }>()
const { t } = useI18n()
const menuOpen = ref(false)
const over = ref(false)
const hue = computed(() => (props.course.id * 67) % 360)
const accent = computed(() => props.custom.color ?? `hsl(${hue.value} 55% 55%)`)
const showImage = computed(() => !!props.course.image && !imageFailed.value)
const imageFailed = ref(false)

function onDrop() { over.value = false; emit('drop') }
</script>

<template>
  <div
    class="ci" :class="[view, { over, hidden: course.hidden }]" :draggable="draggable" :style="{ '--accent': accent }"
    @dragstart="(e) => { e.dataTransfer?.setData('text/plain', String(course.id)); emit('dragstart') }" @dragend="emit('dragend')"
    @dragover.prevent="over = true" @dragleave="over = false" @drop.prevent.stop="onDrop"
  >
    <a :href="`?course=${course.id}`" class="ci-main" :title="name !== course.fullname ? course.fullname : undefined" @click.prevent="emit('open')">
      <div v-if="view === 'cards'" class="ci-cover">
        <img v-if="showImage" :src="course.image!" alt="" loading="lazy" @error="imageFailed = true" />
        <MoodleCourseBadge v-else :course="course" :size="44" class="ci-cover-badge" />
      </div>
      <MoodleCourseBadge v-else :course="course" :size="view === 'compact' ? 22 : 34" />
      <span class="ci-body">
        <span class="ci-name"><span class="ci-text">{{ name }}</span><EyeOff v-if="course.hidden" :size="12" class="ci-hid" /></span>
        <span v-if="course.category && view !== 'compact'" class="ci-cat truncate">{{ course.category }}</span>
        <MoodleProgressBar v-if="view === 'cards' && course.progress != null" :value="course.progress" :width="90" />
      </span>
      <MoodleProgressBar v-if="view === 'list' && course.progress != null" :value="course.progress" />
    </a>
    <span class="ci-tools" :class="{ on: course.favourite || menuOpen }">
      <UiTooltip :text="course.favourite ? t('moodle.crs.unstar') : t('moodle.crs.star')">
        <button class="ci-star" :class="{ on: course.favourite }" :aria-label="course.favourite ? t('moodle.crs.unstar') : t('moodle.crs.star')" @click.stop="emit('favourite')"><Star :size="14" /></button>
      </UiTooltip>
      <UiMenu v-model:open="menuOpen" :items="menu" placement="bottom-end">
        <UiButton size="xs" variant="ghost" :icon="MoreHorizontal" :aria-label="t('common.more')" />
      </UiMenu>
    </span>
  </div>
</template>

<style scoped>
.ci { position: relative; border-radius: var(--r-md); }
.ci.over { outline: 2px dashed var(--accent-color, var(--border-strong)); outline-offset: -2px; }
.ci.hidden { opacity: 0.6; }
.ci-main { display: flex; align-items: center; gap: 10px; text-decoration: none; color: var(--text); min-width: 0; }
.ci-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.ci-name { display: flex; align-items: center; gap: 4px; font-weight: 500; }
.ci-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
.ci-hid { color: var(--text-3); flex: none; }
.ci-cat { font-size: var(--fs-xs); color: var(--text-3); }
.ci-tools { position: absolute; top: 4px; right: 4px; display: flex; align-items: center; gap: 2px; opacity: 0; transition: opacity var(--dur-fast); }
.ci:hover .ci-tools, .ci-tools.on, .ci:focus-within .ci-tools { opacity: 1; }
@media (pointer: coarse) { .ci-tools { opacity: 1; } }
.ci-star { display: grid; place-items: center; width: 24px; height: 24px; border-radius: var(--r-sm); color: var(--text-3); }
.ci-star:hover { background: var(--bg-hover); }
.ci-star.on { color: #e0a100; }
.ci-star.on :deep(svg) { fill: currentColor; }

.ci.list .ci-main, .ci.compact .ci-main { padding: 6px 60px 6px 6px; border-radius: var(--r-md); }
.ci.list:hover, .ci.compact:hover { background: var(--bg-hover); }
.ci.list .ci-tools, .ci.compact .ci-tools { top: 50%; transform: translateY(-50%); }
.ci.compact .ci-main { padding-block: 3px; gap: 8px; }
.ci.compact { font-size: var(--fs-sm); }

.ci.cards { border: 1px solid var(--border); background: var(--bg-elevated); overflow: hidden; border-top: 3px solid var(--accent); }
.ci.cards:hover { border-color: var(--border-strong); border-top-color: var(--accent); }
.ci.cards .ci-main { flex-direction: column; align-items: stretch; gap: 0; }
.ci-cover { height: 72px; display: grid; place-items: center; background: color-mix(in srgb, var(--accent) 14%, var(--bg-panel)); overflow: hidden; }
.ci-cover img { width: 100%; height: 100%; object-fit: cover; }
.ci.cards .ci-body { padding: 8px 10px 10px; }
.ci.cards .ci-text { white-space: normal; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; }
.ci.cards .ci-tools { background: color-mix(in srgb, var(--bg-panel) 85%, transparent); border-radius: var(--r-md); }
</style>
