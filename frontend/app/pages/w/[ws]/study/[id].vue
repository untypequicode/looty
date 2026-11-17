<script setup lang="ts">
import { BookCheck, MoreHorizontal, Lock, LockKeyhole } from 'lucide-vue-next'
import { isCardKind, useStudyActions, useStudyAttempts, useStudyRevisions, useStudySet } from '~/composables/useStudy'
import { useStudyMenu } from '~/composables/useStudyMenu'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { useI18n } from '~/composables/useI18n'

/** Un entraînement, dans un onglet de l'espace : s'entraîner, éditer (droit d'édition) ou voir mes résultats. */
definePageMeta({ layout: 'workspace' })
const route = useRoute()
const { t, formatDateTime } = useI18n()
const { wsId } = useWorkspace()
const id = computed(() => route.params.id as string)
const { data: set, isLoading, isError } = useStudySet(id)
const attempts = useStudyAttempts(id)
const actions = useStudyActions()
const revisions = useStudyRevisions(id)
const { items: menuFor } = useStudyMenu(wsId)

// L'entraînement a changé d'espace (déplacé, lien ancien) : on le suit.
watch(() => set.value?.workspace_id, (ws) => { if (ws && ws !== wsId.value) navigateTo(links.studySet(ws, id.value), { replace: true }) })

type Mode = 'use' | 'edit' | 'stats'
const mode = ref<Mode>(route.query.mode === 'edit' ? 'edit' : 'use')
const modes = computed(() => [
  { value: 'use' as Mode, label: t('study.modeUse') },
  ...(set.value?.can_edit ? [{ value: 'edit' as Mode, label: t('study.modeEdit') }] : []),
  ...(isCardKind(set.value?.kind ?? 'qcm') ? [] : [{ value: 'stats' as Mode, label: t('study.modeStats') }]),
])

const saving = ref(false)
const editorKey = ref(0) // remonte l'éditeur après une restauration
const restoring = ref<string | null>(null)
/** Remet une version (après confirmation) ; le toast propose d'annuler en remettant l'état remplacé. */
async function restoreRev(revId: string, undo = false) {
  if (!undo && !(await confirmDialog({ title: t('study.restore'), message: t('study.restoreConfirm') }))) return
  restoring.value = revId
  try {
    const undoId = await actions.restore(id.value, revId)
    editorKey.value++
    toast.success(t(undo ? 'study.restoreUndone' : 'study.restored'), undo ? {} : { duration: 10000, action: { label: t('study.undo'), run: () => { void restoreRev(undoId, true) } } })
  } finally { restoring.value = null }
}
async function save(v: { title: string; kind: any; items: unknown[] }) {
  saving.value = true
  try {
    await actions.save(id.value, v)
    toast.success(t('study.saved'))
  } finally { saving.value = false }
}

const menuOpen = ref(false)
// Supprimé depuis cette page : retour à la liste de l'espace.
const menu = computed(() => (set.value ? menuFor(set.value, { removed: () => navigateTo(links.study(wsId.value)) }) : []))

const stats = computed(() => {
  const a = attempts.data.value ?? []
  const pct = a.map((x) => (x.max_score ? x.score / x.max_score : 0))
  return { count: a.length, avg: pct.length ? Math.round((100 * pct.reduce((s, x) => s + x, 0)) / pct.length) : 0, best: pct.length ? Math.round(100 * Math.max(...pct)) : 0 }
})
useHead({ title: () => set.value?.title || t('study.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('study.title'), icon: BookCheck, to: links.study(wsId) }, { label: set?.title || t('study.untitled') }]">
      <UiSegmented v-if="set" v-model="mode" :options="modes" />
      <UiMenu v-if="set && menu.length" v-model:open="menuOpen" :items="menu" placement="bottom-end">
        <UiButton variant="ghost" size="sm" :icon="MoreHorizontal" :aria-label="t('common.more')" />
      </UiMenu>
    </AppPageHeader>
    <div class="sd">
      <div v-if="isLoading" class="sd-center"><UiSpinner /></div>
      <UiEmpty v-else-if="isError || !set" :icon="Lock" :title="t('study.notFound')" />
      <template v-else>
        <p class="faint sd-note"><Lock :size="12" /> {{ t('study.privacyNote') }}<template v-if="set.private_to"> · <LockKeyhole :size="12" /> {{ t('personalItem.badge') }}</template></p>

        <template v-if="mode === 'use'">
          <StudySummary v-if="set.kind === 'summary'" :set="set" />
          <StudyCards v-else-if="isCardKind(set.kind)" :set="set" />
          <StudyPlayer v-else :key="set.id" :set="set" />
        </template>

        <template v-else-if="mode === 'edit' && set.can_edit">
          <StudyEditor :key="set.id + set.items.length + ':' + editorKey" :title="set.title" :kind="set.kind" :items="set.items" :saving="saving" :revisions="revisions.data.value" :restoring="restoring" @save="save" @restore="restoreRev($event)" />
        </template>

        <section v-else class="sd-stats">
          <div class="sd-cards">
            <div><b>{{ stats.count }}</b><span>{{ t('study.history') }}</span></div>
            <div><b>{{ stats.avg }}%</b><span>{{ t('study.average') }}</span></div>
            <div><b>{{ stats.best }}%</b><span>{{ t('study.best') }}</span></div>
          </div>
          <p v-if="!attempts.data.value?.length" class="faint">{{ t('study.noAttempts') }}</p>
          <ul v-else class="sd-hist">
            <li v-for="a in attempts.data.value" :key="a.id"><span>{{ formatDateTime(a.started_at) }}</span><b>{{ a.score }}/{{ a.max_score }}</b></li>
          </ul>
        </section>

        <StudyLinks :set-id="set.id" :ws="set.workspace_id" :can-edit="set.can_edit" />
      </template>
    </div>
  </div>
</template>

<style scoped>
.sd { flex: 1; overflow-y: auto; width: 100%; max-width: 760px; margin: 0 auto; padding: 24px 24px 80px; display: flex; flex-direction: column; gap: 16px; }
.sd-center { display: grid; place-items: center; padding: 40px; }
.sd-note { display: flex; align-items: center; gap: 6px; margin: 0; }
.sd-cards { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
.sd-cards > div { display: flex; flex-direction: column; align-items: center; padding: 14px; border: 1px solid var(--border); border-radius: var(--r-md); }
.sd-cards b { font-size: 22px; font-family: var(--font-display); }
.sd-cards span { font-size: var(--fs-sm); color: var(--text-3); }
.sd-hist { list-style: none; margin: 12px 0 0; padding: 0; }
.sd-hist li { display: flex; justify-content: space-between; padding: 8px 4px; border-bottom: 1px solid var(--border); }
</style>
