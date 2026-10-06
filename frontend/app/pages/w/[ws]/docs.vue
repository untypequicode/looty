<script setup lang="ts">
import { FileText, Plus, LayoutGrid, List, Search, Star, Upload, Download } from 'lucide-vue-next'
import { useLocalStorage } from '@vueuse/core'
import { useAccess } from '~/composables/useAccess'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import { useI18n } from '~/composables/useI18n'
import { motion } from '~/composables/useMotion'

definePageMeta({ layout: 'workspace' })

const { wsId, pages, member } = useWorkspace()
const { can } = useAccess(wsId)
const canWrite = computed(() => can.value.editDocs)
const { createAndOpen, toggleFavorite } = usePages()
const { t, relativeTime } = useI18n()
const view = useLocalStorage<'grid' | 'list'>('looty.docs.view', 'list')
const sort = ref<'updated' | 'created' | 'title' | 'popular'>('updated')
const q = ref('')
const importOpen = ref(false)
const exportOpen = ref(false)

const list = computed(() => {
  let l = [...(pages.data.value ?? [])]
  if (q.value.trim()) l = l.filter((p) => (p.title || '').toLowerCase().includes(q.value.trim().toLowerCase()))
  const by: Record<string, (a: any, b: any) => number> = {
    updated: (a, b) => +new Date(b.updated_at) - +new Date(a.updated_at),
    created: (a, b) => +new Date(b.created_at) - +new Date(a.created_at),
    title: (a, b) => (a.title || '').localeCompare(b.title || ''),
    popular: (a, b) => (b.total_view_count ?? 0) - (a.total_view_count ?? 0),
  }
  return l.sort(by[sort.value])
})
const parentTitle = (id?: string | null) => (id ? pages.data.value?.find((p) => p.id === id)?.title || t('docs.untitled') : null)

const root = ref<HTMLElement | null>(null)
watch([view, root], () => nextTick(() => root.value && motion.stagger(root.value.querySelectorAll('.d-item'), { y: 6, each: 0.015 })), { immediate: true })
useHead({ title: () => t('nav.allDocs') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('nav.allDocs'), icon: FileText }]">
      <UiSegmented v-model="view" :options="[{ value: 'list', icon: List, title: t('docs.listView') }, { value: 'grid', icon: LayoutGrid, title: t('docs.gridView') }]" />
      <UiTooltip v-if="canWrite" :text="t('transfer.importTitle')"><UiButton size="sm" variant="ghost" :icon="Upload" :aria-label="t('transfer.importTitle')" @click="importOpen = true" /></UiTooltip>
      <UiTooltip :text="t('transfer.exportWorkspace')"><UiButton size="sm" variant="ghost" :icon="Download" :aria-label="t('transfer.exportWorkspace')" @click="exportOpen = true" /></UiTooltip>
      <UiButton v-if="canWrite" size="sm" variant="primary" :icon="Plus" @click="createAndOpen()">{{ t('docs.new') }}</UiButton>
    </AppPageHeader>
    <DocImportDialog v-model:open="importOpen" />
    <DocExportDialog v-model:open="exportOpen" />
  
    <div ref="root" class="docs">
      <div class="docs-bar">
        <UiInput v-model="q" :icon="Search" size="sm" :placeholder="t('docs.filter')" class="docs-search" />
        <UiSegmented
          v-model="sort"
          :options="[{ value: 'updated', label: t('docs.sortUpdated') }, { value: 'created', label: t('docs.sortCreated') }, { value: 'popular', label: t('docs.sortPopular') }, { value: 'title', label: t('docs.sortTitle') }]"
        />
      </div>
  
      <UiEmpty v-if="!list.length && !pages.isLoading.value" :icon="FileText" :title="t('docs.empty')" :description="t('docs.emptyHint')">
        <UiButton v-if="canWrite" variant="primary" :icon="Plus" @click="createAndOpen()">{{ t('docs.new') }}</UiButton>
      </UiEmpty>
  
      <div v-else-if="view === 'grid'" class="d-grid">
        <NuxtLink v-for="p in list" :key="p.id" :to="links.doc(wsId, p.id)" class="d-item d-card">
          <div class="d-card-icon">{{ p.icon || '📄' }}</div>
          <div class="d-card-title">{{ p.title || t('docs.untitled') }}</div>
          <div v-if="parentTitle(p.parent_id)" class="d-card-parent truncate">{{ parentTitle(p.parent_id) }}</div>
          <div class="d-card-meta">
            <UiAvatar v-if="p.updated_by" :name="member(p.updated_by)?.name" :src="member(p.updated_by)?.avatar_url" :size="16" />
            {{ relativeTime(p.updated_at) }}
          </div>
        </NuxtLink>
      </div>
  
      <div v-else class="d-list">
        <NuxtLink v-for="p in list" :key="p.id" :to="links.doc(wsId, p.id)" class="d-item d-row">
          <span class="d-row-icon">{{ p.icon }}<FileText v-if="!p.icon" :size="15" /></span>
          <span class="d-row-title truncate">{{ p.title || t('docs.untitled') }}</span>
          <span v-if="parentTitle(p.parent_id)" class="d-row-parent truncate">{{ parentTitle(p.parent_id) }}</span>
          <button class="d-fav" :class="{ on: p.is_favorite }" @click.prevent.stop="toggleFavorite(p)"><Star :size="14" :fill="p.is_favorite ? 'currentColor' : 'none'" /></button>
          <span class="d-row-meta">
            <UiAvatar v-if="p.updated_by" :name="member(p.updated_by)?.name" :src="member(p.updated_by)?.avatar_url" :size="16" />
            {{ relativeTime(p.updated_at) }}
          </span>
        </NuxtLink>
      </div>
    </div>
  </div>
</template>

<style scoped>
.docs { flex: 1; overflow-y: auto; padding: 16px 24px 48px; }
.docs > * { max-width: 1040px; margin-left: auto; margin-right: auto; }
.docs-bar { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; flex-wrap: wrap; }
.docs-search { max-width: 280px; }
.d-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 10px; }
.d-card {
  display: flex; flex-direction: column; padding: 14px; min-height: 140px; border-radius: var(--r-lg);
  background: var(--bg-panel); box-shadow: var(--shadow-card); transition: transform var(--dur) var(--ease-out), background var(--dur-fast);
}
.d-card:hover { transform: translateY(-2px); background: var(--bg-card-hover); }
.d-card-icon { font-size: 22px; margin-bottom: 10px; }
.d-card-title { font-weight: 600; line-height: 1.35; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.d-card-parent { font-size: var(--fs-sm); color: var(--text-3); margin-top: 4px; }
.d-card-meta { margin-top: auto; padding-top: 10px; display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-3); }
.d-row { display: flex; align-items: center; gap: 10px; height: 38px; padding: 0 10px; border-radius: var(--r-sm); border-bottom: 1px solid var(--border); }
.d-row:hover { background: var(--bg-hover); }
.d-row-icon { width: 18px; display: grid; place-items: center; color: var(--text-3); flex: none; }
.d-row-title { font-weight: 500; flex: 1; min-width: 0; }
.d-row-parent { color: var(--text-3); font-size: var(--fs-sm); max-width: 200px; }
.d-row-meta { display: flex; align-items: center; gap: 6px; color: var(--text-3); font-size: var(--fs-sm); flex: none; width: 140px; justify-content: flex-end; }
.d-fav { color: var(--text-4); width: 24px; height: 24px; display: grid; place-items: center; border-radius: var(--r-xs); opacity: 0; }
.d-fav.on { color: var(--yellow); opacity: 1; }
.d-row:hover .d-fav { opacity: 1; }
@media (max-width: 639px) {
  .docs { padding: 12px 10px 32px; }
  .d-row-parent { display: none; }
  .d-row-meta { width: auto; }
  .d-fav { opacity: 1; }
}
</style>
