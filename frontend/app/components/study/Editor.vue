<script setup lang="ts">
import { Plus, Trash2, ArrowUp, ArrowDown, RotateCcw, Eye } from 'lucide-vue-next'
import { ITEM_TYPES, type StudyRevision, type StudyItem, type StudyItemType, type StudyKind } from '~/composables/useStudy'
import { useI18n } from '~/composables/useI18n'

/** Mode édition d'un jeu : titre, éléments (ajout, ordre, suppression) selon leur type. */
interface Draft { id?: string; type: StudyItemType; prompt: string; choices: { id: string; text: string }[]; correct: string; multi: string[]; accepted: string; back: string; explanation: string; preview?: boolean }

const props = defineProps<{ title: string; kind: StudyKind; items: StudyItem[]; saving?: boolean; revisions?: StudyRevision[]; restoring?: string | null }>()
const emit = defineEmits<{ (e: 'save', v: { title: string; kind: StudyKind; items: unknown[] }): void; (e: 'restore', id: string): void }>()
const { t, formatDateTime } = useI18n()


const LETTERS = 'abcdefghij'
const title = ref(props.title)
const kind = ref<StudyKind>(props.kind)
const drafts = ref<Draft[]>(props.items.map(fromItem))

function fromItem(it: StudyItem): Draft {
  const a = it.answer ?? {}
  return {
    id: it.id, type: it.type, prompt: it.prompt, choices: (it.choices ?? []).map((c) => ({ ...c })),
    correct: a.choice ?? '', multi: a.choices ?? [], accepted: (a.accepted ?? []).join('\n'), back: a.back ?? '', explanation: it.explanation ?? '',
  }
}
function toItem(d: Draft) {
  const base = { ...(d.id ? { id: d.id } : {}), type: d.type, prompt: d.prompt.trim(), explanation: d.explanation }
  if (d.type === 'single') return { ...base, choices: d.choices, answer: { choice: d.correct } }
  if (d.type === 'multi') return { ...base, choices: d.choices, answer: { choices: d.multi } }
  if (d.type === 'text') return { ...base, answer: { accepted: d.accepted.split('\n').map((s) => s.trim()).filter(Boolean) } }
  return { ...base, answer: { back: d.back } }
}

function add(type: StudyItemType) {
  const choices = type === 'single' || type === 'multi' ? [{ id: 'a', text: '' }, { id: 'b', text: '' }] : []
  drafts.value.push({ type, prompt: '', choices, correct: 'a', multi: [], accepted: '', back: '', explanation: '' })
}
function move(i: number, d: -1 | 1) {
  const j = i + d
  if (j < 0 || j >= drafts.value.length) return
  const a = drafts.value
  ;[a[i], a[j]] = [a[j]!, a[i]!]
}
function remove(i: number) { drafts.value.splice(i, 1) }
function addChoice(d: Draft) {
  if (d.choices.length < 10) d.choices.push({ id: LETTERS[d.choices.length]!, text: '' })
}
function removeChoice(d: Draft, ci: number) {
  if (d.choices.length <= 2) return
  const [gone] = d.choices.splice(ci, 1)
  d.multi = d.multi.filter((x) => x !== gone!.id)
  if (d.correct === gone!.id) d.correct = d.choices[0]!.id
}
function toggleMulti(d: Draft, id: string) {
  d.multi = d.multi.includes(id) ? d.multi.filter((x) => x !== id) : [...d.multi, id]
}

const valid = computed(() => !!title.value.trim() && drafts.value.every((d) => d.prompt.trim()
  && (d.type === 'flashcard' ? d.back.trim() : d.type === 'text' ? true : d.choices.every((c) => c.text.trim()) && (d.type === 'single' ? !!d.correct : d.multi.length > 0))))
const allowed = computed(() => ITEM_TYPES[kind.value])
const isCards = (d: Draft) => d.type === 'flashcard'
// Synthèse : chaque « fiche » est une section (titre + contenu Markdown), même format d'élément.
const isSummary = computed(() => kind.value === 'summary')
function submit() { emit('save', { title: title.value.trim(), kind: kind.value, items: drafts.value.map(toItem) }) }
</script>

<template>
  <div class="ed">
    <UiInput v-model="title" :placeholder="t('study.titlePlaceholder')" maxlength="200" :aria-label="t('study.titleLabel')" />
    <p v-if="!drafts.length" class="faint">{{ t('study.noItems') }}</p>
    <article v-for="(d, i) in drafts" :key="d.id ?? i" class="ed-item">
      <header class="ed-head">
        <b>{{ i + 1 }}. {{ isSummary ? t('study.section') : t(`study.types.${d.type}`) }}</b>
        <span class="ed-tools">
          <UiButton size="xs" :variant="d.preview ? 'secondary' : 'ghost'" :icon="Eye" :aria-pressed="!!d.preview" @click="d.preview = !d.preview">{{ t('study.preview') }}</UiButton>
          <UiButton size="xs" variant="ghost" :icon="ArrowUp" :disabled="i === 0" :aria-label="t('study.moveUp')" @click="move(i, -1)" />
          <UiButton size="xs" variant="ghost" :icon="ArrowDown" :disabled="i === drafts.length - 1" :aria-label="t('study.moveDown')" @click="move(i, 1)" />
          <UiButton size="xs" variant="ghost" :icon="Trash2" :aria-label="t('study.removeItem')" @click="remove(i)" />
        </span>
      </header>
      <UiInput v-if="isSummary" v-model="d.prompt" :placeholder="t('study.sectionTitle')" maxlength="200" :aria-label="t('study.sectionTitle')" />
      <UiInput v-else v-model="d.prompt" multiline :rows="2" :placeholder="isCards(d) ? t('study.front') : t('study.prompt')" :aria-label="isCards(d) ? t('study.front') : t('study.prompt')" />
      <template v-if="d.type === 'single' || d.type === 'multi'">
        <div v-for="(c, ci) in d.choices" :key="c.id" class="ed-choice">
          <input v-if="d.type === 'single'" v-model="d.correct" type="radio" :value="c.id" :name="`c-${i}`" :aria-label="t('study.correct')">
          <input v-else type="checkbox" :checked="d.multi.includes(c.id)" :aria-label="t('study.correct')" @change="toggleMulti(d, c.id)">
          <UiInput v-model="c.text" size="sm" :placeholder="t('study.choice', { n: ci + 1 })" :aria-label="t('study.choice', { n: ci + 1 })" />
          <UiButton size="xs" variant="ghost" :icon="Trash2" :disabled="d.choices.length <= 2" :aria-label="t('common.remove')" @click="removeChoice(d, ci)" />
        </div>
        <UiButton size="xs" variant="ghost" :icon="Plus" :disabled="d.choices.length >= 10" @click="addChoice(d)">{{ t('study.addChoice') }}</UiButton>
      </template>
      <template v-else-if="d.type === 'text'">
        <UiInput v-model="d.accepted" multiline :rows="2" :placeholder="t('study.accepted')" :aria-label="t('study.accepted')" />
        <small class="faint">{{ t('study.acceptedHint') }}</small>
      </template>
      <UiInput v-else-if="isSummary" v-model="d.back" multiline :rows="8" maxlength="4000" :placeholder="t('study.sectionBody')" :aria-label="t('study.sectionBody')" />
      <UiInput v-else v-model="d.back" multiline :rows="3" :placeholder="t('study.back')" :aria-label="t('study.back')" />
      <UiInput v-if="!isCards(d)" v-model="d.explanation" size="sm" :placeholder="t('study.explanation')" :aria-label="t('study.explanation')" />
      <small class="faint">{{ t('study.markdownHint') }}</small>
      <div v-if="d.preview" class="ed-preview" :aria-label="t('study.preview')">
        <RichMarkdown :src="d.prompt" :mode="isSummary ? 'inline' : 'auto'" class="ed-pv-prompt" />
        <ul v-if="d.type === 'single' || d.type === 'multi'">
          <li v-for="c in d.choices" :key="c.id"><RichMarkdown :src="c.text" mode="inline" /></li>
        </ul>
        <RichMarkdown v-if="d.type === 'flashcard' && d.back" :src="d.back" />
        <RichMarkdown v-if="!isCards(d) && d.explanation" :src="d.explanation" class="faint" />
      </div>
    </article>
    <div class="ed-add">
      <UiButton v-for="ty in allowed" :key="ty" size="sm" :icon="Plus" @click="add(ty)">{{ isSummary ? t('study.addSection') : `${t('study.addItem')} · ${t(`study.types.${ty}`)}` }}</UiButton>
    </div>
    <div class="ed-save"><UiButton variant="primary" :disabled="!valid" :loading="saving" @click="submit">{{ t('study.save') }}</UiButton></div>
    <section v-if="revisions?.length" class="ed-rev">
      <b>{{ t('study.revisions') }}</b>
      <small class="faint">{{ t('study.revisionsHint') }}</small>
      <ul>
        <li v-for="r in revisions" :key="r.id">
          <span>{{ formatDateTime(r.created_at) }} · {{ t(`study.revSource.${r.source}`) }} · {{ t('study.revItems', { count: r.item_count }) }}<template v-if="r.title"> · {{ r.title }}</template></span>
          <UiButton size="xs" :icon="RotateCcw" :loading="restoring === r.id" @click="emit('restore', r.id)">{{ t('study.restore') }}</UiButton>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.ed { display: flex; flex-direction: column; gap: 12px; }
.ed-item { display: flex; flex-direction: column; gap: 8px; padding: 12px; border: 1px solid var(--border); border-radius: var(--r-md); background: var(--bg-elevated, transparent); }
.ed-head { display: flex; align-items: center; justify-content: space-between; }
.ed-tools { display: inline-flex; gap: 2px; }
.ed-choice { display: flex; align-items: center; gap: 8px; }
.ed-choice > :nth-child(2) { flex: 1; }
.ed-preview { display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; border-radius: var(--r-md); background: var(--bg-hover); }
.ed-preview ul { margin: 0; padding-left: 20px; }
.ed-pv-prompt { font-weight: 600; }
.ed-add { display: flex; flex-wrap: wrap; gap: 8px; }
.ed-save { display: flex; justify-content: flex-end; }
.ed-rev { display: flex; flex-direction: column; gap: 6px; padding-top: 12px; border-top: 1px solid var(--border); }
.ed-rev ul { list-style: none; margin: 0; padding: 0; }
.ed-rev li { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 6px 0; border-bottom: 1px solid var(--border); }
</style>
