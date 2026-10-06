<script setup lang="ts">
import { ArrowLeft, FileText, Share2 } from 'lucide-vue-next'
import { useSharedPages } from '~/composables/useShared'
import { useI18n } from '~/composables/useI18n'

/** « Partagé avec moi » : documents d'autres espaces auxquels j'ai été invité. */
definePageMeta({ layout: 'default' })
const { t, relativeTime } = useI18n()
const { data, isLoading } = useSharedPages()
const groups = computed(() => {
  const m = new Map<string, { name: string; pages: NonNullable<typeof data.value> }>()
  for (const p of data.value ?? []) {
    if (!m.has(p.workspace_id)) m.set(p.workspace_id, { name: p.workspace_name, pages: [] })
    m.get(p.workspace_id)!.pages.push(p)
  }
  return [...m.values()]
})
useHead({ title: () => t('shared.title') })
</script>

<template>
  <div class="sh">
    <header class="sh-head">
      <UiButton variant="ghost" size="sm" :icon="ArrowLeft" to="/" :aria-label="t('common.back')" />
      <h1><Share2 :size="18" /> {{ t('shared.title') }}</h1>
    </header>
    <p class="faint">{{ t('shared.hint') }}</p>
    <div v-if="isLoading" class="sh-empty"><UiSpinner /></div>
    <UiEmpty v-else-if="!groups.length" :icon="Share2" :title="t('shared.empty')" />
    <section v-for="g in groups" :key="g.name" class="sh-group">
      <h2>{{ g.name }}</h2>
      <NuxtLink v-for="p in g.pages" :key="p.id" :to="`/shared/${p.id}`" class="sh-item">
        <span class="sh-icon">{{ p.icon || '' }}<FileText v-if="!p.icon" :size="15" /></span>
        <span class="sh-title truncate">{{ p.title || t('docs.untitled') }}</span>
        <span class="sh-role">{{ p.role === 'editor' ? t('share.editor') : t('share.viewer') }}</span>
        <span class="sh-time">{{ relativeTime(p.updated_at) }}</span>
      </NuxtLink>
    </section>
  </div>
</template>

<style scoped>
.sh { max-width: 760px; margin: 0 auto; padding: 40px 24px 80px; }
.sh-head { display: flex; align-items: center; gap: 8px; }
h1 { display: flex; align-items: center; gap: 8px; margin: 0; font-family: var(--font-display); font-size: 24px; }
.sh-group { margin-top: 24px; }
h2 { font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); margin: 0 0 6px; }
.sh-item { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: var(--r-md); }
.sh-item:hover { background: var(--bg-hover); }
.sh-icon { width: 20px; display: grid; place-items: center; color: var(--text-3); }
.sh-title { flex: 1; min-width: 0; font-weight: 500; }
.sh-role { font-size: var(--fs-xs); padding: 1px 8px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-2); }
.sh-time { font-size: var(--fs-sm); color: var(--text-3); white-space: nowrap; }
.sh-empty { display: grid; place-items: center; padding: 40px; }
</style>
