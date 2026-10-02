<script setup lang="ts">
import type { Priority } from '~/types'
import { PRIORITIES } from '~/composables/useBoard'
import { useI18n } from '~/composables/useI18n'

const model = defineModel<Priority>({ required: true })
const open = defineModel<boolean>('open', { default: false })
defineProps<{ disabled?: boolean }>()
const { t } = useI18n()
const items = computed(() => PRIORITIES.map((p) => ({ value: p, label: t(`priority.${p}`), shortcut: String(p) })))
</script>

<template>
  <UiCombobox v-model="model" v-model:open="open" :items="items" :placeholder="t('task.setPriority')" :width="220">
    <template v-if="$slots.default" #default><slot /></template>
    <template #item="{ item }"><TaskPriorityIcon :priority="item.value as number" :size="15" /></template>
  </UiCombobox>
</template>
