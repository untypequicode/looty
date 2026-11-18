<script setup lang="ts">
import { Check, Ban, PenLine } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import type { AiSuggestion } from '~/composables/useAi'

/**
 * Modification proposée par l'assistant (mode « suggérer ») : le diff, la page visée, puis
 * accepter ou refuser. Rien n'est écrit tant que la personne n'a pas accepté (ADR 0010).
 */
const props = defineProps<{ suggestion: AiSuggestion; pageTitle?: string; pageHref?: string | null; busy?: boolean }>()
const emit = defineEmits<{ accept: []; reject: [] }>()
const { t } = useI18n()
const uid = useId()
</script>

<template>
  <section class="sg" role="group" :aria-labelledby="`sg-${uid}`">
    <header class="sg-head">
      <PenLine :size="13" class="sg-icon" />
      <span :id="`sg-${uid}`" class="sg-title">{{ t('ai.chat.suggestion') }}</span>
      <NuxtLink v-if="pageHref" :to="pageHref" class="sg-page truncate">{{ pageTitle || t('docs.untitled') }}</NuxtLink>
      <span v-else-if="pageTitle" class="sg-page truncate">{{ pageTitle }}</span>
    </header>
    <pre class="sg-diff" tabindex="0" :aria-label="t('ai.chat.suggestion')"><span v-for="(l, i) in suggestion.diff" :key="i" :class="{ add: l.op === '+', del: l.op === '-' }">{{ l.op === ' ' ? '  ' : `${l.op} ` }}{{ l.text }}
</span></pre>
    <footer class="sg-actions">
      <UiButton size="sm" variant="primary" :icon="Check" :disabled="busy" :aria-describedby="`sg-${uid}`" @click="emit('accept')">{{ t('ai.chat.accept') }}</UiButton>
      <UiButton size="sm" variant="ghost" :icon="Ban" :disabled="busy" :aria-describedby="`sg-${uid}`" @click="emit('reject')">{{ t('ai.chat.reject') }}</UiButton>
    </footer>
  </section>
</template>

<style scoped>
.sg { border: 1px solid var(--border-strong); border-radius: var(--r-md); background: var(--bg-elevated); padding: 10px 12px; max-width: 92%; display: flex; flex-direction: column; gap: 8px; }
.sg-head { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); min-width: 0; }
.sg-icon { flex: none; color: var(--accent); }
.sg-title { font-weight: 600; flex: none; }
.sg-page { color: var(--text-3); min-width: 0; }
a.sg-page:hover { color: var(--text); text-decoration: underline; }
.sg-diff { margin: 0; max-height: 240px; overflow: auto; padding: 6px 8px; border-radius: var(--r-sm); background: var(--bg); font-size: var(--fs-xs); font-family: var(--font-mono, monospace); white-space: pre-wrap; }
.sg-diff:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 1px; }
.sg-diff .add { background: color-mix(in srgb, var(--green, #3a3) 22%, transparent); display: block; }
.sg-diff .del { background: color-mix(in srgb, var(--red, #d33) 22%, transparent); display: block; }
.sg-actions { display: flex; align-items: center; gap: 6px; }
</style>
