<script setup lang="ts">
import { ArrowLeft, FileText, Share2, Search, RefreshCw, Users, CornerLeftUp } from 'lucide-vue-next'
import { useSharedPages } from '~/composables/useShare'
import { groupShared, pathLabel, type SharedItem } from '~/utils/shareAccess'
import { useI18n } from '~/composables/useI18n'

/** « Partagé avec moi » : pages d'autres espaces auxquelles j'ai accès, par espace, les plus récentes d'abord. */
definePageMeta({ layout: 'default' })
const { t, relativeTime } = useI18n()
const { data, isLoading, refetch, isFetching } = useSharedPages()
const query = ref('')
const groups = computed(() => groupShared((data.value ?? []) as SharedItem[], query.value))
const origin = (i: SharedItem) => (i.origin === 'group' ? t('shared.viaGroup', { name: i.via_group ?? '' }) : i.origin === 'inherited' ? (i.from ? t('shared.inherited', { title: i.from.title }) : '') : i.shared_by ? t('shared.sharedBy', { name: i.shared_by }) : t('shared.directly'))
useHead({ title: () => t('shared.title') })
</script>

<template>
  <div class="sh">
    <header class="sh-head">
      <UiButton variant="ghost" size="sm" :icon="ArrowLeft" to="/" :aria-label="t('common.back')" />
      <h1><Share2 :size="18" /> {{ t('shared.title') }}</h1>
      <UiButton class="sh-refresh" variant="ghost" size="sm" :icon="RefreshCw" :loading="isFetching" :aria-label="t('shared.refresh')" @click="refetch()" />
    </header>
    <p class="faint">{{ t('shared.hint') }}</p>
    <UiInput v-model="query" :icon="Search" size="sm" :placeholder="t('shared.searchPlaceholder')" :aria-label="t('shared.searchPlaceholder')" class="sh-search" />
    <div v-if="isLoading" class="sh-empty"><UiSpinner /></div>
    <UiEmpty v-else-if="!data?.length" :icon="Share2" :title="t('shared.empty')" :description="t('shared.requestAccessHint')" />
    <p v-else-if="!groups.length" class="faint">{{ t('shared.noMatch') }}</p>
    <section v-for="g in groups" :key="g.workspace_id" class="sh-group" :aria-label="g.name">
      <h2>{{ g.name }}</h2>
      <NuxtLink v-for="p in g.items" :key="p.id" :to="`/w/${p.workspace_id}/doc/${p.id}`" class="sh-item">
        <span class="sh-icon">{{ p.icon || '' }}<FileText v-if="!p.icon" :size="15" /></span>
        <span class="sh-main">
          <span class="sh-title truncate">{{ p.title || t('docs.untitled') }}</span>
          <span class="sh-sub truncate">
            <Users v-if="p.origin === 'group'" :size="11" /><CornerLeftUp v-else-if="p.origin === 'inherited'" :size="11" />
            {{ origin(p) }}<template v-if="pathLabel(p)"> · {{ pathLabel(p) }}</template>
          </span>
        </span>
        <span class="sh-role">{{ t(`sharing.level.${p.level}`) }}</span>
        <time class="sh-time" :datetime="p.updated_at">{{ relativeTime(p.updated_at) }}</time>
      </NuxtLink>
    </section>
  </div>
</template>

<style scoped>
.sh { max-width: 760px; margin: 0 auto; padding: 40px 24px 80px; }
.sh-head { display: flex; align-items: center; gap: 8px; }
.sh-refresh { margin-left: auto; }
.sh-search { margin-top: 12px; }
h1 { display: flex; align-items: center; gap: 8px; margin: 0; font-family: var(--font-display); font-size: 24px; }
.sh-group { margin-top: 24px; }
h2 { font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); margin: 0 0 6px; }
.sh-item { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: var(--r-md); }
.sh-item:hover, .sh-item:focus-visible { background: var(--bg-hover); }
.sh-icon { width: 20px; display: grid; place-items: center; color: var(--text-3); }
.sh-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.sh-title { font-weight: 500; }
.sh-sub { font-size: var(--fs-xs); color: var(--text-3); display: flex; align-items: center; gap: 4px; }
.sh-role { font-size: var(--fs-xs); padding: 1px 8px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-2); white-space: nowrap; }
.sh-time { font-size: var(--fs-sm); color: var(--text-3); white-space: nowrap; }
.sh-empty { display: grid; place-items: center; padding: 40px; }
@media (max-width: 640px) { .sh { padding: 24px 12px 80px; } .sh-time { display: none; } }
</style>
