<script setup lang="ts">
import { FileText, Share2, PencilLine, Eye } from 'lucide-vue-next'
import type { Page } from '~/types'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'

/**
 * Accueil d'un invité : les pages de l'espace partagées avec lui (et leurs sous-pages),
 * dans l'interface habituelle de l'espace.
 */
const { wsId, workspace, pages } = useWorkspace()
const { t, relativeTime } = useI18n()
const list = computed(() => pages.data.value ?? [])
const ids = computed(() => new Set(list.value.map((p) => p.id)))
/** Pages partagées directement (leur parent n'est pas visible), puis leurs sous-pages. */
const roots = computed(() => list.value.filter((p) => !p.parent_id || !ids.value.has(p.parent_id)))
const childrenOf = (p: Page) => list.value.filter((c) => c.parent_id === p.id)
const recent = computed(() => [...list.value].sort((a, b) => (b.updated_at ?? '').localeCompare(a.updated_at ?? '')).slice(0, 6))
useHead({ title: () => workspace.data.value?.name ?? '' })
</script>

<template>
  <div class="gh">
    <AppPageHeader :crumbs="[{ label: t('guest.homeTitle'), icon: Share2 }]" />
    <div class="gh-body">
      <header class="gh-head">
        <h1>{{ workspace.data.value?.name }}</h1>
        <p class="gh-sub"><span class="gh-badge">{{ t('pagePerms.guest') }}</span> {{ t('guest.homeHint') }}</p>
      </header>
      <div v-if="pages.isLoading.value" class="gh-skel"><div v-for="i in 3" :key="i" class="skeleton" style="height: 64px" /></div>
      <UiEmpty v-else-if="!list.length" :icon="Share2" :title="t('guest.noneTitle')" :description="t('guest.noneHint')" />
      <template v-else>
        <section class="gh-sec">
          <h2>{{ t('guest.sharedWithYou') }}</h2>
          <div class="gh-grid">
            <NuxtLink v-for="p in roots" :key="p.id" :to="links.doc(wsId, p.id)" class="gh-card">
              <span class="gh-icon">{{ p.icon || '' }}<FileText v-if="!p.icon" :size="18" /></span>
              <span class="gh-card-body">
                <b class="truncate">{{ p.title || t('docs.untitled') }}</b>
                <span class="gh-meta">
                  <component :is="p.can_edit ? PencilLine : Eye" :size="12" /> {{ p.can_edit ? t('guest.canEdit') : t('guest.canView') }}
                  <template v-if="childrenOf(p).length"> · {{ t('guest.subpages', { count: childrenOf(p).length }) }}</template>
                </span>
              </span>
            </NuxtLink>
          </div>
        </section>
        <section v-if="list.length > roots.length" class="gh-sec">
          <h2>{{ t('guest.recent') }}</h2>
          <NuxtLink v-for="p in recent" :key="p.id" :to="links.doc(wsId, p.id)" class="gh-row">
            <span class="gh-icon sm">{{ p.icon || '' }}<FileText v-if="!p.icon" :size="14" /></span>
            <span class="truncate">{{ p.title || t('docs.untitled') }}</span>
            <time>{{ relativeTime(p.updated_at) }}</time>
          </NuxtLink>
        </section>
      </template>
    </div>
  </div>
</template>

<style scoped>
.gh { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.gh-body { flex: 1; overflow-y: auto; padding: 28px 32px 60px; max-width: 980px; width: 100%; margin: 0 auto; display: flex; flex-direction: column; gap: 24px; }
.gh-head h1 { margin: 0; font-family: var(--font-display); font-size: 28px; letter-spacing: -.02em; }
.gh-sub { margin: 6px 0 0; color: var(--text-3); display: flex; align-items: center; gap: 8px; }
.gh-badge { font-size: 10.5px; font-weight: 600; padding: 1px 7px; border-radius: var(--r-full); background: color-mix(in srgb, #e2a03f 20%, transparent); color: #b87a1d; }
.gh-skel { display: grid; gap: 10px; }
.gh-sec h2 { margin: 0 0 10px; font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); font-weight: 600; }
.gh-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 10px; }
.gh-card { display: flex; align-items: center; gap: 12px; padding: 14px; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); color: var(--text); text-decoration: none; min-width: 0; }
.gh-card:hover { background: var(--bg-hover); }
.gh-card-body { display: flex; flex-direction: column; min-width: 0; gap: 2px; }
.gh-icon { width: 34px; height: 34px; flex: none; display: grid; place-items: center; border-radius: var(--r-md); background: var(--bg-hover); font-size: 18px; color: var(--text-3); }
.gh-icon.sm { width: 22px; height: 22px; font-size: 14px; background: none; }
.gh-meta { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-sm); color: var(--text-3); }
.gh-row { display: flex; align-items: center; gap: 8px; padding: 7px 8px; border-radius: var(--r-md); color: var(--text); text-decoration: none; }
.gh-row:hover { background: var(--bg-hover); }
.gh-row time { margin-left: auto; font-size: var(--fs-sm); color: var(--text-3); flex: none; }
@media (max-width: 767px) { .gh-body { padding: 18px 14px 40px; } }
</style>
