<script setup lang="ts">
import { useUi } from '~/composables/useUi'
import { useBoards } from '~/composables/useBoards'
import { useI18n } from '~/composables/useI18n'

const { createBoardOpen } = useUi()
const { createBoard } = useBoards()
const { t } = useI18n()
const name = ref('')
const withColumns = ref(true)
const loading = ref(false)

watch(createBoardOpen, (v) => { if (v) { name.value = ''; withColumns.value = true } })

async function submit() {
  if (!name.value.trim()) return
  loading.value = true
  try {
    await createBoard(name.value.trim(), withColumns.value)
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
      <label class="opt">
        <UiSwitch v-model="withColumns" />
        <span>
          <span class="opt-title">{{ t('board.defaultColumns') }}</span>
          <span class="opt-hint">{{ t('board.defaultColumnsHint') }}</span>
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
</style>
