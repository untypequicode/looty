<script setup lang="ts">
import { ArrowLeft, Link2, Trash2, Lock } from 'lucide-vue-next'
import { isCardKind, useStudyActions, useStudyAttempts, useStudyRevisions, useStudySet, type StudyVisibility } from '~/composables/useStudy'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { useI18n } from '~/composables/useI18n'

/** Un jeu de révision : réviser, éditer (propriétaire, éditeurs) ou voir mes résultats. */
definePageMeta({ layout: 'default' })
const route = useRoute()
const { t, formatDateTime } = useI18n()
const id = computed(() => route.params.id as string)
// Jeton d'un lien de partage (?token=…) : donne seulement le droit de passer le jeu.
const token = computed(() => (typeof route.query.token === 'string' ? route.query.token : null))
const { data: set, isLoading, isError } = useStudySet(id, token)
const attempts = useStudyAttempts(id, token)
const actions = useStudyActions()
const revisions = useStudyRevisions(id)

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

const VIS: StudyVisibility[] = ['private', 'workspace', 'link']
const visOptions = computed(() => VIS.filter((v) => v !== 'workspace' || set.value?.workspace_id).map((v) => ({ value: v, label: t(`study.vis.${v}`) })))
const vis = computed({
  get: () => set.value?.visibility ?? 'private',
  set: (v: StudyVisibility) => { actions.setVisibility(id.value, v) },
})
function copyLink() {
  if (!set.value?.share_token) return
  navigator.clipboard.writeText(`${location.origin}/study/${id.value}?token=${set.value.share_token}`)
  toast.success(t('common.linkCopied'))
}
async function remove() {
  if (!set.value || !(await confirmDialog({ title: t('study.deleteSet'), message: t('study.deleteConfirm', { title: set.value.title }), danger: true }))) return
  await actions.remove(id.value)
  await navigateTo('/study')
}

const stats = computed(() => {
  const a = attempts.data.value ?? []
  const pct = a.map((x) => (x.max_score ? x.score / x.max_score : 0))
  return { count: a.length, avg: pct.length ? Math.round((100 * pct.reduce((s, x) => s + x, 0)) / pct.length) : 0, best: pct.length ? Math.round(100 * Math.max(...pct)) : 0 }
})
useHead({ title: () => set.value?.title || t('study.title') })
</script>

<template>
  <div class="sd">
    <header class="sd-head">
      <UiButton variant="ghost" size="sm" :icon="ArrowLeft" to="/study" :aria-label="t('common.back')" />
      <h1 class="truncate">{{ set?.title || t('study.untitled') }}</h1>
      <UiSegmented v-if="set" v-model="mode" :options="modes" />
    </header>
    <div v-if="isLoading" class="sd-center"><UiSpinner /></div>
    <UiEmpty v-else-if="isError || !set" :icon="Lock" :title="t('study.notFound')" />
    <template v-else>
      <p class="faint sd-note"><Lock :size="12" /> {{ t('study.privacyNote') }}</p>

      <template v-if="mode === 'use'">
        <StudyCards v-if="isCardKind(set.kind)" :set="set" :token="token" />
        <StudyPlayer v-else :key="set.id" :set="set" :token="token" />
      </template>

      <template v-else-if="mode === 'edit' && set.can_edit">
        <section v-if="set.is_owner" class="sd-share">
          <b>{{ t('study.visibility') }}</b>
          <UiSegmented v-model="vis" :options="visOptions" />
          <small class="faint">{{ t(`study.visHint.${vis}`) }}</small>
          <UiButton v-if="set.visibility === 'link' && set.share_token" size="sm" :icon="Link2" @click="copyLink">{{ t('study.copyLink') }}</UiButton>
          <UiButton size="sm" variant="danger" :icon="Trash2" @click="remove">{{ t('study.deleteSet') }}</UiButton>
        </section>
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
    </template>
  </div>
</template>

<style scoped>
.sd { max-width: 760px; margin: 0 auto; padding: 40px 24px 80px; display: flex; flex-direction: column; gap: 16px; }
.sd-head { display: flex; align-items: center; gap: 8px; }
.sd-head h1 { flex: 1; min-width: 0; margin: 0; font-family: var(--font-display); font-size: 24px; }
.sd-center { display: grid; place-items: center; padding: 40px; }
.sd-note { display: flex; align-items: center; gap: 6px; margin: 0; }
.sd-share { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 12px; border: 1px solid var(--border); border-radius: var(--r-md); }
.sd-cards { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
.sd-cards > div { display: flex; flex-direction: column; align-items: center; padding: 14px; border: 1px solid var(--border); border-radius: var(--r-md); }
.sd-cards b { font-size: 22px; font-family: var(--font-display); }
.sd-cards span { font-size: var(--fs-sm); color: var(--text-3); }
.sd-hist { list-style: none; margin: 12px 0 0; padding: 0; }
.sd-hist li { display: flex; justify-content: space-between; padding: 8px 4px; border-bottom: 1px solid var(--border); }
</style>
