<script setup lang="ts">
import type { Board } from '~/types'
import { columnProgress } from '~/composables/useBoard'
import { useI18n } from '~/composables/useI18n'

const model = defineModel<string>({ required: true })
const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ board: Board }>()
const { t } = useI18n()
const items = computed(() => props.board.columns.map((c, i) => ({ value: c.id, label: c.name, shortcut: String(i + 1) })))
</script>

<template>
  <UiCombobox v-model="model" v-model:open="open" :items="items" :placeholder="t('task.changeStatus')" :width="230">
    <template v-if="$slots.default" #default><slot /></template>
    <template #item="{ item }"><TaskStatusIcon v-bind="columnProgress(board, item.value as string)" :size="15" /></template>
  </UiCombobox>
</template>
