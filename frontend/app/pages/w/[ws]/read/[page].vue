<script setup lang="ts">
import { useQuery } from '@tanstack/vue-query'
import type { Page } from '~/types'
import { api } from '~/composables/api'
import { useWorkspace, links, wsKeys } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { workspacePreamble } from '~/editor/mathScope'
import type { ReaderEntry } from '~/components/doc/Reader.vue'

/** Vue de lecture des cours : la page, le plan du cours (page racine et sous-pages). */
definePageMeta({ layout: 'default' })

const route = useRoute()
const { t } = useI18n()
const { wsId, pages, workspace } = useWorkspace()
const pageId = computed(() => route.params.page as string)

const { data: page, error } = useQuery({
  queryKey: computed(() => ['page', pageId.value]),
  queryFn: () => api<Page>(`/api/workspaces/${wsId.value}/pages/${pageId.value}`, { silent: true }),
  enabled: computed(() => !!pageId.value && !!wsId.value),
  staleTime: 0,
})
watch(pageId, (id) => { if (id) api(`/api/workspaces/${wsId.value}/pages/${id}/view`, { method: 'POST', silent: true }).catch(() => {}) }, { immediate: true })

// Préambule LaTeX de l'espace (hors de la mise en page « workspace »).
const latex = useQuery({ queryKey: computed(() => wsKeys.latex(wsId.value)), queryFn: () => api<{ preamble: string }>(`/api/workspaces/${wsId.value}/latex`, { silent: true }), enabled: computed(() => !!wsId.value) })
watch(() => latex.data.value?.preamble, (p) => { workspacePreamble.value = p ?? '' }, { immediate: true })

// --- Plan du cours : page racine de l'arborescence et ses descendants, dans l'ordre ---
const all = computed(() => pages.data.value ?? [])
const byId = computed(() => new Map(all.value.map((p) => [p.id, p])))
const root = computed(() => {
  let p = byId.value.get(pageId.value)
  while (p?.parent_id && byId.value.get(p.parent_id)) p = byId.value.get(p.parent_id)
  return p
})
const outline = computed<ReaderEntry[]>(() => {
  const out: ReaderEntry[] = []
  const walk = (p: Page, depth: number) => {
    out.push({ id: p.id, title: p.title, icon: p.icon, depth, to: links.read(wsId.value, p.id) })
    all.value.filter((c) => c.parent_id === p.id).sort((a, b) => a.position - b.position).forEach((c) => walk(c, depth + 1))
  }
  if (root.value) walk(root.value, 0)
  return out
})
const index = computed(() => outline.value.findIndex((e) => e.id === pageId.value))
const link = (e?: ReaderEntry) => (e ? { title: e.title || t('docs.untitled'), to: e.to } : null)
const prev = computed(() => (index.value > 0 ? link(outline.value[index.value - 1]) : null))
const next = computed(() => (index.value >= 0 ? link(outline.value[index.value + 1]) : null))
const crumb = computed(() => (root.value && root.value.id !== pageId.value ? root.value.title || t('docs.untitled') : workspace.data.value?.name ?? ''))

useHead({ title: () => (page.value ? `${page.value.icon ? `${page.value.icon} ` : ''}${page.value.title || t('docs.untitled')}` : t('reader.title')) })
</script>

<template>
  <DocReader
    :page="page" :error="!!error" :current-id="pageId" :crumb="crumb" :outline="outline" :prev="prev" :next="next"
    :back-to="links.doc(wsId, pageId)" :edit-to="page?.can_edit !== false ? links.doc(wsId, pageId) : null"
  />
</template>
