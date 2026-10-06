<script setup lang="ts">
import { ArrowLeft, FileText, Lock, Check, Loader2 } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import type { Page } from '~/types'
import { api } from '~/composables/api'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { useSharedPages } from '~/composables/useShared'

/**
 * Document partagé avec un invité (personne extérieure à l'espace) : édition collaborative
 * si son accès le permet, liste des autres pages partagées (sous-pages comprises).
 */
definePageMeta({ layout: 'default' })
const route = useRoute()
const { t, relativeTime } = useI18n()
const { user } = useAuth()
const pageId = computed(() => route.params.page as string)
const { data: shared } = useSharedPages()
const entry = computed(() => shared.value?.find((p) => p.id === pageId.value))
const { data: page, error } = useQuery({
  queryKey: computed(() => ['shared-page', pageId.value]),
  queryFn: () => api<Page>(`/api/workspaces/${entry.value!.workspace_id}/pages/${pageId.value}`, { silent: true }),
  enabled: computed(() => !!entry.value),
})
const status = ref<'connecting' | 'synced' | 'offline' | 'denied'>('connecting')
const canEdit = computed(() => page.value?.can_edit === true)
const children = computed(() => (shared.value ?? []).filter((p) => p.parent_id === pageId.value))
const cursorUser = computed(() => ({ name: user.value?.name ?? '?', color: '#e2a03f' }))
async function saveHtml(html: string) {
  if (!entry.value) return
  await api(`/api/workspaces/${entry.value.workspace_id}/pages/${pageId.value}`, { method: 'PUT', body: { content: html }, silent: true }).catch(() => {})
}
useHead({ title: () => page.value?.title || t('shared.title') })
</script>

<template>
  <div class="sp">
    <header class="sp-bar">
      <UiButton variant="ghost" size="sm" :icon="ArrowLeft" to="/shared" :aria-label="t('shared.title')" />
      <span class="sp-ws truncate">{{ entry?.workspace_name }}</span>
      <span class="sp-guest">{{ t('pagePerms.guest') }}</span>
      <span class="spacer" />
      <span class="sp-status">
        <template v-if="!canEdit"><Lock :size="13" /> {{ t('editor.readOnly') }}</template>
        <template v-else-if="status === 'connecting'"><Loader2 :size="13" class="spin" /> {{ t('editor.connecting') }}</template>
        <template v-else><Check :size="13" /> {{ t('common.saved') }}</template>
      </span>
    </header>
    <UiEmpty v-if="(shared && !entry) || error" :icon="Lock" :title="t('docs.notFound')" :description="t('shared.noAccess')" />
    <article v-else-if="page" class="sp-doc">
      <p v-if="page.icon" class="sp-icon">{{ page.icon }}</p>
      <h1 class="sp-title">{{ page.title || t('docs.untitled') }}</h1>
      <p class="sp-meta">{{ t('reader.updated', { time: relativeTime(page.updated_at) }) }}</p>
      <DocEditor :key="pageId" :page-id="pageId" :initial-html="page.content" :can-edit="canEdit" :user="cursorUser" @html="saveHtml" @status="(s) => (status = s)" />
      <section v-if="children.length" class="sp-children">
        <h3>{{ t('editor.subDocuments') }}</h3>
        <NuxtLink v-for="c in children" :key="c.id" :to="`/shared/${c.id}`" class="sp-child"><FileText :size="14" /> {{ c.title || t('docs.untitled') }}</NuxtLink>
      </section>
    </article>
    <div v-else class="sp-doc"><div class="skeleton" style="height: 40px; width: 60%" /></div>
  </div>
</template>

<style scoped>
.sp { min-height: 100dvh; }
.sp-bar { position: sticky; top: 0; z-index: 5; display: flex; align-items: center; gap: 8px; height: 48px; padding: 0 12px; background: var(--bg); border-bottom: 1px solid var(--border); }
.sp-ws { color: var(--text-3); font-size: var(--fs-sm); }
.sp-guest { font-size: 10.5px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-full); background: color-mix(in srgb, var(--orange, #e2a03f) 20%, transparent); color: var(--orange, #e2a03f); }
.sp-status { display: inline-flex; align-items: center; gap: 5px; font-size: var(--fs-sm); color: var(--text-3); }
.spin { animation: spin 1s linear infinite; }
.sp-doc { max-width: 760px; margin: 0 auto; padding: 40px 24px 120px; }
.sp-icon { font-size: 48px; margin: 0; }
.sp-title { font-family: var(--font-display); font-size: 36px; letter-spacing: -.02em; margin: 6px 0 0; }
.sp-meta { color: var(--text-3); font-size: var(--fs-sm); margin: 10px 0 24px; }
.sp-children { margin-top: 40px; padding-top: 16px; border-top: 1px solid var(--border); display: flex; flex-direction: column; gap: 4px; }
.sp-children h3 { font-size: var(--fs-sm); text-transform: uppercase; color: var(--text-3); margin: 0 0 6px; }
.sp-child { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--r-sm); }
.sp-child:hover { background: var(--bg-hover); }
</style>
