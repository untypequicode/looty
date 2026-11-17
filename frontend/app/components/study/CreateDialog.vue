<script setup lang="ts">
import { studyCreateRequest } from '~/composables/useStudyCreate'
import { STUDY_KINDS, useStudyActions, type StudyKind } from '~/composables/useStudy'
import { useAccess } from '~/composables/useAccess'
import { useWorkspaces, links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'

/** « Nouvel entraînement » : titre, type et, hors espace perso, personnel ou partagé. */
const { t } = useI18n()
const actions = useStudyActions()
const workspaces = useWorkspaces()

const open = computed({ get: () => !!studyCreateRequest.value, set: (v) => { if (!v) studyCreateRequest.value = null } })
const ws = computed(() => studyCreateRequest.value?.ws ?? '')
const { can } = useAccess(ws)
// Espace perso : tout est déjà personnel. Rôle en lecture seule : personnel seulement.
const inPersonalSpace = computed(() => !!workspaces.data.value?.find((w: any) => w.id === ws.value)?.personal)
const personalOnly = computed(() => !can.value.editDocs)
const form = reactive({ title: '', kind: 'qcm' as StudyKind, personal: false })
watch(studyCreateRequest, (r) => {
  if (!r) return
  form.title = ''
  form.kind = 'qcm'
  form.personal = r.personal
})
const busy = ref(false)
async function create() {
  const r = studyCreateRequest.value
  if (!r || !form.title.trim()) return
  busy.value = true
  try {
    const id = await actions.create({ title: form.title.trim(), kind: form.kind, workspace_id: r.ws, parent_id: r.parentId, personal: personalOnly.value || form.personal })
    studyCreateRequest.value = null
    await navigateTo(links.studySet(r.ws, id, 'edit'))
  } finally { busy.value = false }
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('study.new')" :width="460">
    <form class="sc" @submit.prevent="create">
      <UiInput v-model="form.title" :placeholder="t('study.titlePlaceholder')" maxlength="200" :aria-label="t('study.titleLabel')" autofocus />
      <UiSegmented v-model="form.kind" :options="STUDY_KINDS.map((k) => ({ value: k, label: t(`study.kinds.${k}`) }))" />
      <label v-if="!inPersonalSpace" class="sc-personal">
        <span>
          <b>{{ t('personalItem.badge') }}</b>
          <small class="faint">{{ t('study.personalHint') }}</small>
        </span>
        <UiSwitch :model-value="personalOnly || form.personal" :disabled="personalOnly" :label="t('personalItem.badge')" @update:model-value="(v: boolean) => (form.personal = v)" />
      </label>
    </form>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :disabled="!form.title.trim()" :loading="busy" @click="create">{{ t('study.create') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.sc { display: flex; flex-direction: column; gap: 12px; }
.sc-personal { display: flex; align-items: center; justify-content: space-between; gap: 12px; font-size: var(--fs-sm); }
.sc-personal span { display: flex; flex-direction: column; gap: 2px; }
</style>
