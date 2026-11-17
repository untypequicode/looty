<script setup lang="ts">
import { ArrowLeft, GraduationCap, Plus } from 'lucide-vue-next'
import { useStudyActions, useStudySets, type StudyKind } from '~/composables/useStudy'
import { useWorkspaces } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'

/** Révisions : mes jeux (QCM, questionnaires, fiches, synthèses) et ceux de mes espaces. */
definePageMeta({ layout: 'default' })
const { t, relativeTime } = useI18n()
const { data, isLoading } = useStudySets()
const { user } = useAuth()
const workspaces = useWorkspaces()
const actions = useStudyActions()

const mine = computed(() => (data.value ?? []).filter((s) => s.owner_id === user.value?.id))
const shared = computed(() => (data.value ?? []).filter((s) => s.owner_id !== user.value?.id))

const open = ref(false)
const form = reactive({ title: '', kind: 'qcm' as StudyKind, workspace_id: '' })
const KINDS: StudyKind[] = ['qcm', 'questionnaire', 'flashcards', 'summary']
const wsItems = computed(() => [{ value: '', label: t('study.personalSpace') }, ...(workspaces.data.value ?? []).filter((w: any) => !w.personal).map((w: any) => ({ value: w.id, label: w.name }))])
const busy = ref(false)
async function create() {
  busy.value = true
  try {
    const id = await actions.create({ title: form.title.trim(), kind: form.kind, workspace_id: form.workspace_id || null })
    open.value = false
    await navigateTo(`/study/${id}?mode=edit`)
  } finally { busy.value = false }
}
useHead({ title: () => t('study.title') })
</script>

<template>
  <div class="st">
    <header class="st-head">
      <UiButton variant="ghost" size="sm" :icon="ArrowLeft" to="/" :aria-label="t('common.back')" />
      <h1><GraduationCap :size="18" /> {{ t('study.title') }}</h1>
      <UiButton variant="primary" size="sm" :icon="Plus" @click="form.title = ''; open = true">{{ t('study.new') }}</UiButton>
    </header>
    <div v-if="isLoading" class="st-empty"><UiSpinner /></div>
    <UiEmpty v-else-if="!data?.length" :icon="GraduationCap" :title="t('study.empty')" :description="t('study.emptyHint')" />
    <template v-for="(group, gi) in [{ label: t('study.mine'), sets: mine }, { label: t('study.shared'), sets: shared }]" :key="gi">
      <section v-if="group.sets.length" class="st-group">
        <h2>{{ group.label }}</h2>
        <NuxtLink v-for="s in group.sets" :key="s.id" :to="`/study/${s.id}`" class="st-item">
          <span class="st-title truncate">{{ s.title || t('study.untitled') }}</span>
          <span class="st-tag">{{ t(`study.kinds.${s.kind}`) }}</span>
          <span class="st-tag">{{ t(`study.vis.${s.visibility}`) }}</span>
          <span class="st-meta">{{ t('study.itemCount', { count: s.item_count }) }}<template v-if="s.last_max"> · {{ t('study.lastScore', { score: s.last_score ?? 0, max: s.last_max }) }}</template></span>
          <span class="st-time">{{ relativeTime(s.updated_at) }}</span>
        </NuxtLink>
      </section>
    </template>

    <UiDialog v-model:open="open" :title="t('study.new')" :width="460">
      <div class="st-form">
        <UiInput v-model="form.title" :placeholder="t('study.titlePlaceholder')" maxlength="200" :aria-label="t('study.titleLabel')" autofocus />
        <UiSegmented v-model="form.kind" :options="KINDS.map((k) => ({ value: k, label: t(`study.kinds.${k}`) }))" />
        <label class="st-ws">{{ t('study.space') }}
          <select v-model="form.workspace_id"><option v-for="w in wsItems" :key="w.value" :value="w.value">{{ w.label }}</option></select>
        </label>
        <div class="st-actions"><UiButton variant="primary" :disabled="!form.title.trim()" :loading="busy" @click="create">{{ t('study.create') }}</UiButton></div>
      </div>
    </UiDialog>
  </div>
</template>

<style scoped>
.st { max-width: 820px; margin: 0 auto; padding: 40px 24px 80px; }
.st-head { display: flex; align-items: center; gap: 8px; }
.st-head h1 { flex: 1; display: flex; align-items: center; gap: 8px; margin: 0; font-family: var(--font-display); font-size: 24px; }
.st-group { margin-top: 24px; }
h2 { font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); margin: 0 0 6px; }
.st-item { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: var(--r-md); }
.st-item:hover { background: var(--bg-hover); }
.st-title { flex: 1; min-width: 0; font-weight: 500; }
.st-tag { font-size: var(--fs-xs); padding: 1px 8px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-2); }
.st-meta, .st-time { font-size: var(--fs-sm); color: var(--text-3); white-space: nowrap; }
.st-empty { display: grid; place-items: center; padding: 40px; }
.st-form { display: flex; flex-direction: column; gap: 12px; }
.st-ws { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); color: var(--text-2); }
.st-actions { display: flex; justify-content: flex-end; }
</style>
