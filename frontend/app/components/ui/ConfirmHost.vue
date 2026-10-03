<script setup lang="ts">
import { confirmState } from '~/composables/useConfirm'

const typed = ref('')
const open = computed({
  get: () => !!confirmState.value,
  set: (v) => { if (!v) answer(false) },
})
watch(confirmState, () => (typed.value = ''))
const canConfirm = computed(() => !confirmState.value?.typeToConfirm || typed.value.trim() === confirmState.value.typeToConfirm)

function answer(v: boolean) {
  const s = confirmState.value
  confirmState.value = null
  s?.resolve(v)
}
</script>

<template>
  <UiDialog v-model:open="open" :title="confirmState?.title" :width="420">
    <p v-if="confirmState?.message" class="msg">{{ confirmState.message }}</p>
    <template v-if="confirmState?.typeToConfirm">
      <label class="field-label">{{ $t('common.typeToConfirm', { text: confirmState.typeToConfirm }) }}</label>
      <UiInput v-model="typed" autofocus @keydown.enter="canConfirm && answer(true)" />
    </template>
    <template #footer>
      <UiButton variant="ghost" @click="answer(false)">{{ $t('common.cancel') }}</UiButton>
      <UiButton :variant="confirmState?.danger ? 'danger' : 'primary'" :disabled="!canConfirm" autofocus @click="answer(true)">
        {{ confirmState?.confirmLabel || $t('common.confirm') }}
      </UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.msg { margin: 0 0 12px; color: var(--text-2); line-height: 1.55; }
</style>
