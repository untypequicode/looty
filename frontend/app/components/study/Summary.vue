<script setup lang="ts">
import { Check, Circle } from 'lucide-vue-next'
import type { StudySet } from '~/composables/useStudy'
import { useI18n } from '~/composables/useI18n'

/**
 * Synthèse : lecture des sections (titre = recto de l'élément, contenu Markdown = verso), avec sommaire.
 * « Lu » est un repère personnel gardé dans ce navigateur : rien n'est envoyé au serveur.
 */
const props = defineProps<{ set: StudySet }>()
const { t } = useI18n()

const key = computed(() => `looty.study.read.${props.set.id}`)
const read = ref<Set<string>>(new Set())
onMounted(() => {
  try { read.value = new Set(JSON.parse(localStorage.getItem(key.value) ?? '[]')) } catch { read.value = new Set() }
})
function toggle(id: string) {
  const next = new Set(read.value)
  if (!next.delete(id)) next.add(id)
  read.value = next
  try { localStorage.setItem(key.value, JSON.stringify([...next])) } catch { /* stockage indisponible : repère non gardé */ }
}

const sections = computed(() => props.set.items.map((it, i) => ({ id: it.id ?? String(i), anchor: `sum-${i + 1}`, title: it.prompt, body: it.answer?.back ?? '' })))
const done = computed(() => sections.value.filter((s) => read.value.has(s.id)).length)
</script>

<template>
  <div class="sm">
    <p v-if="!sections.length" class="faint">{{ t('study.noItems') }}</p>
    <template v-else>
      <nav class="sm-toc" :aria-label="t('study.summaryToc')">
        <b>{{ t('study.summaryToc') }}</b>
        <ol>
          <li v-for="s in sections" :key="s.id"><a :href="`#${s.anchor}`" :class="{ done: read.has(s.id) }"><RichMarkdown :src="s.title" mode="inline" /></a></li>
        </ol>
        <small class="faint">{{ t('study.summaryProgress', { done, total: sections.length }) }}</small>
      </nav>
      <section v-for="s in sections" :id="s.anchor" :key="s.id" class="sm-sec">
        <h2><RichMarkdown :src="s.title" mode="inline" /></h2>
        <RichMarkdown :src="s.body" />
        <UiButton size="xs" variant="ghost" :icon="read.has(s.id) ? Check : Circle" :aria-pressed="read.has(s.id)" @click="toggle(s.id)">{{ read.has(s.id) ? t('study.summaryRead') : t('study.summaryMarkRead') }}</UiButton>
      </section>
    </template>
  </div>
</template>

<style scoped>
.sm { display: flex; flex-direction: column; gap: 20px; }
.sm-toc { display: flex; flex-direction: column; gap: 6px; padding: 12px 14px; border: 1px solid var(--border); border-radius: var(--r-md); }
.sm-toc ol { margin: 0; padding-left: 20px; }
.sm-toc a { color: var(--text); text-decoration: none; }
.sm-toc a:hover { text-decoration: underline; }
.sm-toc a.done { color: var(--text-3); }
.sm-sec { display: flex; flex-direction: column; gap: 8px; padding-bottom: 16px; border-bottom: 1px solid var(--border); scroll-margin-top: 16px; }
.sm-sec h2 { margin: 0; font-family: var(--font-display); font-size: 19px; }
</style>
