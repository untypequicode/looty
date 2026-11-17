<script setup lang="ts">
import { Check, X } from 'lucide-vue-next'
import { useStudyActions, type StudyAttempt, type StudyCorrection, type StudySet } from '~/composables/useStudy'
import { useI18n } from '~/composables/useI18n'

/** Mode utilisation d'un QCM ou d'un questionnaire : réponses, correction, score. */
const props = defineProps<{ set: StudySet }>()
const { t } = useI18n()
const actions = useStudyActions()

const answers = reactive<Record<string, any>>({})
const result = ref<{ attempt: StudyAttempt; correction: StudyCorrection[] } | null>(null)
const started = ref(new Date().toISOString())
const busy = ref(false)

const correction = (id?: string) => result.value?.correction.find((c) => c.item_id === id)
function toggle(id: string, choice: string) {
  const cur: string[] = answers[id] ?? []
  answers[id] = cur.includes(choice) ? cur.filter((x) => x !== choice) : [...cur, choice]
}
function expectedText(it: StudySet['items'][number], c?: StudyCorrection) {
  const a = c?.answer ?? {}
  if (it.type === 'text') return (a.accepted ?? []).join(' / ')
  const ids: string[] = it.type === 'single' ? [a.choice] : a.choices ?? []
  return (it.choices ?? []).filter((x) => ids.includes(x.id)).map((x) => x.text).join(', ')
}
async function submit() {
  busy.value = true
  try {
    result.value = await actions.submitAttempt(props.set.id, { ...answers }, started.value)
  } finally { busy.value = false }
}
function restart() {
  for (const k of Object.keys(answers)) delete answers[k]
  result.value = null
  started.value = new Date().toISOString()
}
</script>

<template>
  <div class="pl">
    <div v-if="result" class="pl-score">{{ t('study.score', { score: result.attempt.score, max: result.attempt.max_score }) }}</div>
    <article v-for="(it, i) in set.items" :key="it.id" class="pl-q" :class="{ ok: correction(it.id)?.correct === true, ko: correction(it.id)?.correct === false }">
      <h3>{{ i + 1 }}. {{ it.prompt }}</h3>
      <template v-if="it.type === 'single' || it.type === 'multi'">
        <label v-for="c in it.choices ?? []" :key="c.id" class="pl-choice">
          <input v-if="it.type === 'single'" v-model="answers[it.id!]" type="radio" :value="c.id" :name="`q-${it.id}`" :disabled="!!result">
          <input v-else type="checkbox" :checked="(answers[it.id!] ?? []).includes(c.id)" :disabled="!!result" @change="toggle(it.id!, c.id)">
          <span>{{ c.text }}</span>
        </label>
      </template>
      <UiInput v-else v-model="answers[it.id!]" :placeholder="t('study.typeAnswer')" :disabled="!!result" :aria-label="t('study.yourAnswer')" />
      <div v-if="result" class="pl-fix">
        <span v-if="correction(it.id)?.correct === true" class="good"><Check :size="14" /> {{ t('study.right') }}</span>
        <span v-else-if="correction(it.id)?.correct === false" class="bad"><X :size="14" /> {{ t('study.wrong') }} · {{ t('study.expected') }} : {{ expectedText(it, correction(it.id)) }}</span>
        <span v-else class="faint">{{ t('study.unscored') }} · {{ expectedText(it, correction(it.id)) }}</span>
        <p v-if="correction(it.id)?.explanation" class="faint">{{ correction(it.id)!.explanation }}</p>
      </div>
    </article>
    <div class="pl-actions">
      <UiButton v-if="!result" variant="primary" :loading="busy" @click="submit">{{ t('study.submit') }}</UiButton>
      <UiButton v-else @click="restart">{{ t('study.restart') }}</UiButton>
    </div>
  </div>
</template>

<style scoped>
.pl { display: flex; flex-direction: column; gap: 14px; }
.pl-score { font-family: var(--font-display); font-size: 22px; }
.pl-q { padding: 14px; border: 1px solid var(--border); border-radius: var(--r-md); display: flex; flex-direction: column; gap: 8px; }
.pl-q.ok { border-color: var(--success, #2e9e5b); }
.pl-q.ko { border-color: var(--danger, #d64545); }
.pl-q h3 { margin: 0; font-size: var(--fs-md, 15px); }
.pl-choice { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--r-sm); cursor: pointer; }
.pl-choice:hover { background: var(--bg-hover); }
.pl-fix { font-size: var(--fs-sm); }
.pl-fix .good { color: var(--success, #2e9e5b); display: inline-flex; align-items: center; gap: 4px; }
.pl-fix .bad { color: var(--danger, #d64545); display: inline-flex; align-items: center; gap: 4px; }
.pl-actions { display: flex; justify-content: flex-end; }
</style>
