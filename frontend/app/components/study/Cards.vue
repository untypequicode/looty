<script setup lang="ts">
import { useStudyActions, useStudyCards, type Rating, type StudySet } from '~/composables/useStudy'
import { useI18n } from '~/composables/useI18n'

/** Fiches : recto/verso, puis évaluation (répétition espacée propre à la personne). */
const props = defineProps<{ set: StudySet }>()
const { t } = useI18n()
const id = computed(() => props.set.id)
const states = useStudyCards(id)
const actions = useStudyActions()

const all = ref(false)
const flipped = ref(false)
const done = ref<Set<string>>(new Set())

const due = computed(() => {
  const now = Date.now()
  const byItem = new Map((states.data.value ?? []).map((s) => [s.item_id, s]))
  return props.set.items.filter((it) => {
    if (done.value.has(it.id!)) return false
    if (all.value) return true
    const s = byItem.get(it.id!)
    return !s || new Date(s.due_at).getTime() <= now
  })
})
const current = computed(() => due.value[0])

async function rate(r: Rating) {
  const it = current.value
  if (!it) return
  await actions.review(props.set.id, it.id!, r)
  done.value = new Set([...done.value, it.id!])
  flipped.value = false
}
const RATINGS: Rating[] = ['again', 'hard', 'good', 'easy']
</script>

<template>
  <div class="cd">
    <p class="faint">{{ t('study.dueCards', { count: due.length }) }}</p>
    <template v-if="current">
      <div class="cd-card" role="button" tabindex="0" :aria-label="t('study.flip')" @click="flipped = !flipped" @keydown.enter.prevent="flipped = !flipped" @keydown.space.prevent="flipped = !flipped">
        <span class="cd-side">{{ flipped ? t('study.back') : t('study.front') }}</span>
        <RichMarkdown class="cd-text" :src="(flipped ? current.answer?.back : current.prompt) ?? ''" mode="auto" />
      </div>
      <div class="cd-actions">
        <UiButton v-if="!flipped" variant="primary" @click="flipped = true">{{ t('study.flip') }}</UiButton>
        <template v-else>
          <UiButton v-for="r in RATINGS" :key="r" :variant="r === 'again' ? 'danger' : r === 'good' ? 'primary' : 'secondary'" @click="rate(r)">{{ t(`study.${r}`) }}</UiButton>
        </template>
      </div>
    </template>
    <div v-else class="cd-empty">
      <p>{{ t('study.noneDue') }}</p>
      <UiButton @click="all = true; done = new Set()">{{ t('study.reviewAll') }}</UiButton>
    </div>
  </div>
</template>

<style scoped>
.cd { display: flex; flex-direction: column; gap: 14px; align-items: stretch; }
.cd-card { min-height: 200px; display: flex; flex-direction: column; gap: 12px; align-items: center; justify-content: center; padding: 24px; border: 1px solid var(--border); border-radius: var(--r-lg, 12px); background: var(--bg-elevated, transparent); cursor: pointer; text-align: center; }
.cd-side { font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); }
.cd-text { font-size: 20px; max-width: 100%; }
.cd-actions { display: flex; gap: 8px; justify-content: center; flex-wrap: wrap; }
.cd-empty { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 24px; }
</style>
