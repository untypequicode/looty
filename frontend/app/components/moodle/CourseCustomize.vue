<script setup lang="ts">
import { useI18n } from '~/composables/useI18n'
import type { MoodleCourse } from '~/composables/useMoodle'
import { COURSE_COLORS, type CourseCustom } from '~/composables/useMoodleCourses'

/** Personnalisation d'un cours (GLOSSARY) : nom court, couleur et icône, pour ce membre seulement. */
const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ course: MoodleCourse | null; custom: CourseCustom }>()
const emit = defineEmits<{ (e: 'save', value: CourseCustom): void }>()
const { t } = useI18n()
const draft = reactive<CourseCustom>({})
watch(() => [open.value, props.course?.id], () => { if (open.value) Object.assign(draft, { short: '', color: '', icon: '' }, props.custom) })
const preview = computed(() => ({ ...(props.course ?? { id: 0, shortname: '', fullname: '' }) }))
function save() { emit('save', { short: draft.short?.trim(), color: draft.color, icon: draft.icon?.trim() }); open.value = false }
function reset() { emit('save', {}); open.value = false }
</script>

<template>
  <UiDialog v-model:open="open" :title="t('moodle.crs.customize')" :description="course?.fullname" :width="420">
    <div class="cz">
      <label class="cz-field">
        <span>{{ t('moodle.crs.shortName') }}</span>
        <UiInput v-model="draft.short" :placeholder="course?.shortname || course?.fullname" maxlength="40" />
      </label>
      <div class="cz-field">
        <span>{{ t('moodle.crs.color') }}</span>
        <div class="cz-colors">
          <button class="cz-none" :class="{ on: !draft.color }" :aria-label="t('common.none')" @click="draft.color = ''">∅</button>
          <button
            v-for="c in COURSE_COLORS" :key="c" class="cz-color" :class="{ on: draft.color === c }" :style="{ background: c }" :aria-label="c" @click="draft.color = c"
          />
        </div>
      </div>
      <label class="cz-field">
        <span>{{ t('moodle.crs.icon') }}</span>
        <UiInput v-model="draft.icon" :placeholder="t('moodle.crs.iconHint')" maxlength="4" />
      </label>
      <div class="cz-preview">
        <MoodleCourseBadge :course="{ ...preview, shortname: draft.short || preview.shortname }" :size="40" :override="{ ...draft }" />
        <small>{{ t('moodle.crs.customizeHint') }}</small>
      </div>
      <div class="cz-actions">
        <UiButton variant="ghost" @click="reset">{{ t('moodle.crs.resetCustom') }}</UiButton>
        <span class="grow" />
        <UiButton @click="open = false">{{ t('common.cancel') }}</UiButton>
        <UiButton variant="primary" @click="save">{{ t('common.save') }}</UiButton>
      </div>
    </div>
  </UiDialog>
</template>

<style scoped>
.cz { display: flex; flex-direction: column; gap: 14px; }
.cz-field { display: flex; flex-direction: column; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); }
.cz-colors { display: flex; gap: 8px; flex-wrap: wrap; }
.cz-color, .cz-none { width: 26px; height: 26px; border-radius: 50%; border: 2px solid transparent; display: grid; place-items: center; color: var(--text-3); }
.cz-none { border-color: var(--border); }
.cz-color.on, .cz-none.on { border-color: var(--text); box-shadow: 0 0 0 2px var(--bg-panel) inset; }
.cz-preview { display: flex; align-items: center; gap: 12px; color: var(--text-3); }
.cz-actions { display: flex; align-items: center; gap: 8px; }
.grow { flex: 1; }
</style>
