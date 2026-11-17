<script setup lang="ts">
import { BookCheck, Plus, Search, MoreHorizontal, LockKeyhole } from 'lucide-vue-next'
import { STUDY_KINDS, useStudySets, type StudyKind } from '~/composables/useStudy'
import { useStudyMenu } from '~/composables/useStudyMenu'
import { openStudyCreate } from '~/composables/useStudyCreate'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useAccess } from '~/composables/useAccess'
import { useI18n } from '~/composables/useI18n'
import { fold } from '~/utils/fold'

/** Entraînements de l'espace : tous, avec recherche et filtre par type ; chacun s'ouvre dans un onglet. */
definePageMeta({ layout: 'workspace' })
const { t, relativeTime } = useI18n()
const { wsId, isGuest, workspace } = useWorkspace()
const { can } = useAccess(wsId)
const { data, isLoading } = useStudySets(wsId)
const { items: menuFor } = useStudyMenu(wsId)

// Rôle en lecture seule : il peut quand même ranger des entraînements personnels.
const canCreate = computed(() => !isGuest.value && (can.value.editDocs || !workspace.data.value?.personal))
const q = ref('')
const kind = ref<'all' | StudyKind>('all')
const list = computed(() => {
  const needle = fold(q.value.trim())
  return (data.value ?? [])
    .filter((s) => (kind.value === 'all' || s.kind === kind.value) && (!needle || fold(s.title).includes(needle)))
    .sort((a, b) => +new Date(b.updated_at) - +new Date(a.updated_at))
})
const openMenu = ref<string | null>(null)
useHead({ title: () => t('study.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('study.title'), icon: BookCheck }]">
      <UiButton v-if="canCreate" size="sm" variant="primary" :icon="Plus" @click="openStudyCreate(wsId, { personal: !can.editDocs })">{{ t('study.new') }}</UiButton>
    </AppPageHeader>
    <div class="st">
      <div class="st-bar">
        <UiInput v-model="q" :icon="Search" size="sm" :placeholder="t('study.filter')" class="st-search" />
        <UiSegmented v-model="kind" :options="[{ value: 'all', label: t('study.allKinds') }, ...STUDY_KINDS.map((k) => ({ value: k, label: t(`study.kinds.${k}`) }))]" />
      </div>
      <div v-if="isLoading" class="st-center"><UiSpinner /></div>
      <UiEmpty v-else-if="!list.length" :icon="BookCheck" :title="data?.length ? t('study.noMatch') : t('study.empty')" :description="data?.length ? '' : t('study.emptyHint')">
        <UiButton v-if="canCreate && !data?.length" variant="primary" :icon="Plus" @click="openStudyCreate(wsId, { personal: !can.editDocs })">{{ t('study.new') }}</UiButton>
      </UiEmpty>
      <div v-else class="st-list">
        <NuxtLink v-for="s in list" :key="s.id" :to="links.studySet(wsId, s.id)" class="st-row" @contextmenu.prevent="openMenu = s.id">
          <BookCheck :size="15" class="st-icon" />
          <span class="st-title truncate">{{ s.title || t('study.untitled') }}</span>
          <LockKeyhole v-if="s.private_to" :size="12" class="st-icon" :aria-label="t('personalItem.badge')"><title>{{ t('personalItem.title') }}</title></LockKeyhole>
          <span class="st-tag">{{ t(`study.kinds.${s.kind}`) }}</span>
          <span class="st-meta">{{ t('study.itemCount', { count: s.item_count }) }}<template v-if="s.last_max"> · {{ t('study.lastScore', { score: s.last_score ?? 0, max: s.last_max }) }}</template></span>
          <span class="st-time">{{ relativeTime(s.updated_at) }}</span>
          <span class="st-act" @click.prevent.stop>
            <UiMenu :open="openMenu === s.id" :items="menuFor(s)" @update:open="(v: boolean) => (openMenu = v ? s.id : null)">
              <button class="st-more" :aria-label="t('common.more')"><MoreHorizontal :size="14" /></button>
            </UiMenu>
          </span>
        </NuxtLink>
      </div>
    </div>
  </div>
</template>

<style scoped>
.st { flex: 1; overflow-y: auto; padding: 16px 24px 48px; }
.st > * { max-width: 1040px; margin-left: auto; margin-right: auto; }
.st-bar { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; flex-wrap: wrap; }
.st-search { max-width: 280px; }
.st-center { display: grid; place-items: center; padding: 40px; }
.st-row { display: flex; align-items: center; gap: 10px; height: 40px; padding: 0 10px; border-radius: var(--r-sm); border-bottom: 1px solid var(--border); }
.st-row:hover { background: var(--bg-hover); }
.st-icon { color: var(--text-3); flex: none; }
.st-title { flex: 1; min-width: 0; font-weight: 500; }
.st-tag { font-size: var(--fs-xs); padding: 1px 8px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-2); white-space: nowrap; }
.st-meta, .st-time { font-size: var(--fs-sm); color: var(--text-3); white-space: nowrap; }
.st-act { display: flex; width: 24px; flex: none; }
.st-more { width: 24px; height: 24px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); opacity: 0; }
.st-row:hover .st-more, .st-row:focus-within .st-more { opacity: 1; }
.st-more:hover { background: var(--bg-active); color: var(--text); }
@media (max-width: 639px) {
  .st { padding: 12px 10px 32px; }
  .st-meta { display: none; }
  .st-more { opacity: 1; }
}
</style>
