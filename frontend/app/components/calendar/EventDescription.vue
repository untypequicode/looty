<script setup lang="ts">
import { FileText, FileX } from 'lucide-vue-next'
import { useWorkspace, links } from '~/composables/useWorkspace'

/**
 * Description d'un événement (texte brut, partagée en iCalendar) : adresses web cliquables,
 * liens vers les documents Looty affichés comme mentions de page (titre à jour).
 */
const props = defineProps<{ text: string }>()
const { pages, wsId } = useWorkspace()
const URL_RE = /(https?:\/\/[^\s<>"')\]]+|\/w\/[0-9a-f-]{36}\/doc\/[0-9a-f-]{36})/gi
const DOC_RE = /\/w\/[0-9a-f-]{36}\/doc\/([0-9a-f-]{36})/i

type Part = { kind: 'text'; value: string } | { kind: 'url'; value: string } | { kind: 'page'; id: string; href: string }
const parts = computed<Part[]>(() => {
  const out: Part[] = []
  let last = 0
  for (const m of props.text.matchAll(URL_RE)) {
    const i = m.index ?? 0
    if (i > last) out.push({ kind: 'text', value: props.text.slice(last, i) })
    const url = m[0].replace(/[.,;:]+$/, '')
    const doc = url.match(DOC_RE)
    out.push(doc ? { kind: 'page', id: doc[1]!, href: links.doc(wsId.value, doc[1]!) } : { kind: 'url', value: url })
    last = i + url.length
  }
  if (last < props.text.length) out.push({ kind: 'text', value: props.text.slice(last) })
  return out
})
const page = (id: string) => (pages.data.value ?? []).find((p) => p.id === id)
</script>

<template>
  <p class="ed">
    <template v-for="(p, i) in parts" :key="i">
      <template v-if="p.kind === 'text'">{{ p.value }}</template>
      <a v-else-if="p.kind === 'url'" :href="p.value" target="_blank" rel="noopener noreferrer">{{ p.value }}</a>
      <NuxtLink v-else :to="p.href" class="ed-page" :class="{ missing: pages.data.value && !page(p.id) }">
        <span v-if="page(p.id)?.icon">{{ page(p.id)!.icon }}</span>
        <component :is="pages.data.value && !page(p.id) ? FileX : FileText" v-else :size="12" />
        {{ page(p.id)?.title || $t('docs.untitled') }}
      </NuxtLink>
    </template>
  </p>
</template>

<style scoped>
.ed { margin: 4px 0 0; white-space: pre-wrap; word-break: break-word; font-size: var(--fs-sm); color: var(--text-2); }
.ed a { color: var(--accent); }
.ed-page { display: inline-flex; align-items: center; gap: 3px; padding: 0 4px; border-radius: 4px; background: var(--accent-soft); font-weight: 500; }
.ed-page.missing { color: var(--text-3); background: var(--bg-code); text-decoration: line-through; }
</style>
