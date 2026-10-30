<script setup lang="ts">
import { PenTool, Plus, Search } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useAccess } from '~/composables/useAccess'
import { usePages, pagePath } from '~/composables/usePages'
import { fold } from '~/utils/fold'

definePageMeta({ layout: 'workspace' })

/**
 * Canevas de l'espace : pages ouvertes en canevas ou dont le canevas n'est pas vide.
 * Un canevas est une page comme les autres (droits, commentaires, arborescence).
 */
interface CanvasItem { id: string; title: string; icon: string | null; parent_id: string | null; default_view: 'doc' | 'canvas'; canvas_count: number; canvas_preview: string | null; updated_at: string }
const { t, relativeTime } = useI18n()
const { wsId, pages } = useWorkspace()
const { allows } = useAccess(wsId)
const { createPage } = usePages()
const { data, isLoading } = useQuery({
  queryKey: computed(() => ['ws', wsId.value, 'canvases']),
  queryFn: () => api<CanvasItem[]>(`/api/workspaces/${wsId.value}/canvases`),
  enabled: computed(() => !!wsId.value),
})
const search = ref('')
const shown = computed(() => {
  const q = fold(search.value.trim())
  return (data.value ?? []).filter((c) => !q || fold(c.title).includes(q))
})
const canCreate = computed(() => allows('docs', 'edit'))
const where = (c: CanvasItem) => pagePath(pages.data.value ?? [], c.parent_id ?? '').map((p) => p.title || t('docs.untitled')).join(' / ')
const preview = (svg: string | null) => (svg ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}` : '')
const open = (c: CanvasItem) => `${links.doc(wsId.value, c.id)}${c.default_view === 'canvas' ? '' : '?view=canvas'}`

const creating = ref(false)
async function create() {
  creating.value = true
  try {
    const p = await createPage({ title: t('canvas.untitled'), defaultView: 'canvas' })
    navigateTo(links.doc(wsId.value, p.id))
  } finally { creating.value = false }
}
useHead({ title: () => t('canvas.section') })
</script>

<template>
  <div class="cl">
    <AppPageHeader :crumbs="[{ label: t('canvas.section'), icon: PenTool }]">
      <div class="cl-search"><Search :size="14" /><input v-model="search" :placeholder="t('canvas.search')" :aria-label="t('canvas.search')" /></div>
      <UiButton v-if="canCreate" size="sm" variant="primary" :icon="Plus" :loading="creating" @click="create">{{ t('canvas.new') }}</UiButton>
    </AppPageHeader>
    <div class="cl-body">
      <p class="cl-hint">{{ t('canvas.sectionHint') }}</p>
      <div v-if="isLoading" class="cl-grid"><div v-for="i in 6" :key="i" class="skeleton" style="height: 220px" /></div>
      <UiEmpty v-else-if="!shown.length" :icon="PenTool" :title="t('canvas.empty')" :description="t('canvas.emptyHint')">
        <UiButton v-if="canCreate" variant="primary" :icon="Plus" @click="create">{{ t('canvas.new') }}</UiButton>
      </UiEmpty>
      <div v-else class="cl-grid">
        <NuxtLink v-for="c in shown" :key="c.id" :to="open(c)" class="cl-card">
          <div class="cl-thumb">
            <img v-if="c.canvas_preview" :src="preview(c.canvas_preview)" alt="" loading="lazy" />
            <PenTool v-else :size="28" :stroke-width="1.3" class="cl-thumb-empty" />
          </div>
          <div class="cl-info">
            <div class="cl-title"><span v-if="c.icon">{{ c.icon }}</span><span class="truncate">{{ c.title || t('docs.untitled') }}</span></div>
            <div class="cl-meta truncate">{{ t('canvas.elements', { count: c.canvas_count }) }} · {{ relativeTime(c.updated_at) }}<template v-if="where(c)"> · {{ where(c) }}</template></div>
          </div>
        </NuxtLink>
        <button v-if="canCreate" class="cl-card cl-new" @click="create"><Plus :size="20" /> {{ t('canvas.new') }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.cl { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.cl-body { flex: 1; overflow-y: auto; padding: 18px 24px 48px; }
.cl-hint { margin: 0 0 16px; color: var(--text-3); font-size: var(--fs-sm); }
.cl-search { display: flex; align-items: center; gap: 6px; height: 28px; padding: 0 9px; border-radius: var(--r-md); background: var(--bg-hover); color: var(--text-3); }
.cl-search input { border: 0; background: transparent; color: var(--text); width: 180px; font-size: var(--fs-sm); }
.cl-search input:focus { outline: none; }
.cl-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 16px; }
.cl-card { display: flex; flex-direction: column; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); overflow: hidden; color: var(--text); text-decoration: none; transition: transform .15s ease; min-width: 0; }
.cl-card:hover { transform: translateY(-2px); }
.cl-thumb { height: 160px; display: grid; place-items: center; padding: 12px; background: radial-gradient(circle, color-mix(in srgb, var(--text) 9%, transparent) 1px, transparent 1.2px) 0 0 / 14px 14px, var(--bg); border-bottom: 1px solid var(--border); }
.cl-thumb img { max-width: 100%; max-height: 100%; object-fit: contain; }
.cl-thumb-empty { color: var(--text-4, var(--text-3)); }
.cl-info { padding: 10px 12px 12px; display: flex; flex-direction: column; gap: 3px; }
.cl-title { display: flex; gap: 6px; font-weight: 600; min-width: 0; }
.cl-meta { font-size: var(--fs-sm); color: var(--text-3); }
.cl-new { align-items: center; justify-content: center; flex-direction: row; gap: 8px; min-height: 220px; background: transparent; box-shadow: none; border: 1.5px dashed var(--border-strong); color: var(--text-3); }
@media (max-width: 639px) { .cl-body { padding: 14px; } .cl-search input { width: 110px; } }
</style>
