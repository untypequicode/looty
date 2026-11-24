<script setup lang="ts">
import { useUi } from '~/composables/useUi'
import { useBoards } from '~/composables/useBoards'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace } from '~/composables/useWorkspace'
import { useTemplates } from '~/composables/useTemplates'

const { createBoardOpen, createBoardIn } = useUi()
const { createBoard } = useBoards()
const { t } = useI18n()
const name = ref('')
const withColumns = ref(true)
/** Tableau personnel : visible de soi seul (pas dans l'espace perso, déjà privé). */
const personal = ref(false)
const { workspace, wsId } = useWorkspace()
const canBePersonal = computed(() => !createBoardIn.value && !workspace.data.value?.personal)
const loading = ref(false)
/** Modèle de tableau choisi (vide : tableau vide, avec ou sans colonnes par défaut). */
const templateId = ref('')
const targetWs = computed(() => createBoardIn.value || wsId.value)
const { data: allTemplates } = useTemplates(targetWs, () => createBoardOpen.value)
const templates = computed(() => (allTemplates.value ?? []).filter((x) => x.kind === 'board'))
const chosen = computed(() => templates.value.find((x) => x.id === templateId.value))

watch(createBoardOpen, (v) => { if (v) { name.value = ''; withColumns.value = true; personal.value = false; templateId.value = '' } else createBoardIn.value = '' })

async function submit() {
  if (!name.value.trim()) return
  loading.value = true
  try {
    await createBoard(name.value.trim(), withColumns.value, createBoardIn.value || undefined, personal.value && canBePersonal.value, templateId.value || undefined)
    createBoardOpen.value = false
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <UiDialog v-model:open="createBoardOpen" :title="t('board.new')" :description="t('board.newHint')">
    <form class="form" @submit.prevent="submit">
      <UiInput v-model="name" :placeholder="t('board.namePlaceholder')" size="lg" autofocus />
      <div v-if="templates.length" class="tpls">
        <span class="opt-title">{{ t('tpl.useTemplate') }}</span>
        <div class="tpl-list">
          <button type="button" class="tpl" :class="{ on: !templateId }" @click="templateId = ''">{{ t('tpl.blank') }}</button>
          <button v-for="x in templates" :key="x.id" type="button" class="tpl" :class="{ on: templateId === x.id }" @click="templateId = x.id">{{ x.name }}</button>
        </div>
        <span v-if="chosen?.columns?.length" class="opt-hint">{{ chosen.columns.join(' · ') }}</span>
      </div>
      <label v-if="!templateId" class="opt">
        <UiSwitch v-model="withColumns" />
        <span>
          <span class="opt-title">{{ t('board.defaultColumns') }}</span>
          <span class="opt-hint">{{ t('board.defaultColumnsHint') }}</span>
        </span>
      </label>
      <label v-if="canBePersonal" class="opt">
        <UiSwitch v-model="personal" />
        <span>
          <span class="opt-title">{{ t('personalItem.create') }}</span>
          <span class="opt-hint">{{ t('personalItem.createHint') }}</span>
        </span>
      </label>
    </form>
    <template #footer>
      <UiButton variant="ghost" @click="createBoardOpen = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="loading" :disabled="!name.trim()" @click="submit">{{ t('common.create') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.form { display: flex; flex-direction: column; gap: 16px; }
.opt { display: flex; gap: 12px; align-items: flex-start; cursor: pointer; }
.opt-title { display: block; font-weight: 500; }
.opt-hint { display: block; color: var(--text-3); font-size: var(--fs-sm); margin-top: 2px; }
.tpls { display: flex; flex-direction: column; gap: 8px; }
.tpl-list { display: flex; flex-wrap: wrap; gap: 6px; }
.tpl { padding: 5px 12px; border-radius: var(--r-md); border: 1px solid var(--border-strong); background: var(--bg-input); }
.tpl:hover { border-color: var(--text-3); }
.tpl.on { border-color: var(--accent); font-weight: 500; }
</style>
