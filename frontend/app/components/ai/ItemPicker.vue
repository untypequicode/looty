<script setup lang="ts">
import type { Component } from 'vue'
import { FileText, BookCheck, CheckSquare, Kanban, CalendarDays, FolderCode, Search } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { refDebounced } from '@vueuse/core'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspaces } from '~/composables/useWorkspace'
import type { AiAttachKind, AiAttachment } from '~/composables/useAi'
import type { SearchResult } from '~/types'

/**
 * Palette de recherche pour joindre un élément à un message : pages, entraînements, tâches, tableaux,
 * événements et projets de tous vos espaces (la même recherche que la palette de commandes, par espace).
 */
const open = defineModel<boolean>('open', { default: false })
const query = defineModel<string>('query', { default: '' })
const emit = defineEmits<{ pick: [AiAttachment] }>()
const { t } = useI18n()

const KINDS: Record<AiAttachKind, { icon: Component; types: SearchResult['type'][] }> = {
  page: { icon: FileText, types: ['page'] },
  study: { icon: BookCheck, types: ['study'] },
  task: { icon: CheckSquare, types: ['task'] },
  board: { icon: Kanban, types: ['board'] },
  event: { icon: CalendarDays, types: ['event'] },
  project: { icon: FolderCode, types: ['project'] },
}
const kindOf = (r: SearchResult): AiAttachKind | null => (Object.entries(KINDS).find(([, v]) => v.types.includes(r.type))?.[0] as AiAttachKind | undefined) ?? null

const filter = ref<'all' | AiAttachKind>('all')
const filters = computed(() => [{ value: 'all' as const, label: t('ai.picker.all') }, ...(Object.keys(KINDS) as AiAttachKind[]).map((k) => ({ value: k, icon: KINDS[k].icon, title: t(`ai.picker.kind.${k}`) }))])

const { data: workspaces } = useWorkspaces()
const debounced = refDebounced(query, 180)
const input = ref<{ focus: () => void } | null>(null)

interface Hit { r: SearchResult; kind: AiAttachKind; ws: string; wsName: string }
const { data: hits, isFetching } = useQuery({
  queryKey: computed(() => ['ai', 'attach-search', debounced.value.trim(), (workspaces.value ?? []).map((w) => w.id).join(',')]),
  queryFn: async (): Promise<Hit[]> => {
    const q = debounced.value.trim()
    const out = await Promise.all((workspaces.value ?? []).map(async (w) => {
      try {
        const r = await api<{ results: SearchResult[] }>(`/api/workspaces/${w.id}/search?q=${encodeURIComponent(q)}`, { silent: true })
        return r.results.flatMap((x) => { const kind = kindOf(x); return kind ? [{ r: x, kind, ws: w.id, wsName: w.name }] : [] })
      } catch { return [] }
    }))
    return out.flat()
  },
  enabled: computed(() => open.value && !!debounced.value.trim() && !!workspaces.value?.length),
})

const shown = computed(() => (hits.value ?? []).filter((h) => filter.value === 'all' || h.kind === filter.value).slice(0, 40))
const active = ref(0)
watch([shown, open], () => { active.value = 0 })
watch(open, (v) => { if (v) { filter.value = 'all'; nextTick(() => setTimeout(() => input.value?.focus(), 30)) } })

function path(h: Hit) {
  const place = h.wsName
  return h.r.type === 'event' && h.r.date ? `${place} · ${new Date(h.r.date).toLocaleDateString()}` : place
}
function choose(h: Hit | undefined) {
  if (!h) return
  emit('pick', { kind: h.kind, id: h.r.id, title: h.r.title, snippet: h.kind === 'page' || h.kind === 'study' ? undefined : (h.r.snippet ?? undefined), ws: h.ws })
  open.value = false
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') { e.preventDefault(); active.value = (active.value + 1) % Math.max(1, shown.value.length) }
  else if (e.key === 'ArrowUp') { e.preventDefault(); active.value = (active.value - 1 + shown.value.length) % Math.max(1, shown.value.length) }
  else if (e.key === 'Enter' && !e.isComposing) { e.preventDefault(); choose(shown.value[active.value]) }
  else if (e.key === 'Escape') open.value = false
}
</script>

<template>
  <UiPopover v-model:open="open" :width="380" placement="top-start">
    <template #trigger><slot /></template>
    <div class="pk" @keydown="onKey">
      <UiInput ref="input" v-model="query" :icon="Search" size="sm" :placeholder="t('ai.picker.search')" />
      <UiSegmented v-model="filter" :options="filters" class="pk-filter" />
      <div class="pk-list" role="listbox">
        <p v-if="!debounced.trim()" class="pk-hint">{{ t('ai.picker.hint') }}</p>
        <div v-else-if="isFetching && !shown.length" class="pk-hint"><UiSpinner /></div>
        <p v-else-if="!shown.length" class="pk-hint">{{ t('ai.picker.none') }}</p>
        <button v-for="(h, i) in shown" :key="`${h.ws}:${h.r.id}`" class="pk-row" :class="{ on: i === active }" role="option" :aria-selected="i === active" @mouseenter="active = i" @click="choose(h)">
          <component :is="KINDS[h.kind].icon" :size="14" class="pk-icon" />
          <span class="pk-text">
            <span class="pk-title truncate">{{ h.r.title || t('docs.untitled') }}</span>
            <span class="pk-path truncate">{{ path(h) }}</span>
          </span>
          <span class="pk-kind">{{ t(`ai.picker.kind.${h.kind}`) }}</span>
        </button>
      </div>
    </div>
  </UiPopover>
</template>

<style scoped>
.pk { display: flex; flex-direction: column; gap: 8px; padding: 8px; }
.pk-filter { align-self: flex-start; }
.pk-list { display: flex; flex-direction: column; max-height: 300px; overflow-y: auto; }
.pk-hint { color: var(--text-3); font-size: var(--fs-sm); padding: 8px; margin: 0; display: flex; justify-content: center; }
.pk-row { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; color: var(--text); }
.pk-row.on { background: var(--bg-hover); }
.pk-icon { flex: none; color: var(--text-3); }
.pk-text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.pk-title { font-size: var(--fs-sm); }
.pk-path { font-size: var(--fs-xs); color: var(--text-3); }
.pk-kind { flex: none; font-size: var(--fs-xs); color: var(--text-4); }
</style>
