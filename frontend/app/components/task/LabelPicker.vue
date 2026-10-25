<script setup lang="ts">
import type { Label } from '~/types'
import { LABEL_COLORS } from '~/composables/useBoard'
import { useI18n } from '~/composables/useI18n'

const model = defineModel<string[]>({ required: true })
const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ labels: Label[]; canCreate?: boolean; virtualRect?: { x: number; y: number } | null }>()
const emit = defineEmits<{ (e: 'create', name: string, color: string): void }>()
const { t } = useI18n()
const items = computed(() => props.labels.map((l) => ({ value: l.id, label: l.name, color: l.color })))

function onCreate(name: string) {
  const color = LABEL_COLORS[props.labels.length % LABEL_COLORS.length]!
  emit('create', name, color)
}
</script>

<template>
  <UiCombobox
    :virtual-rect="virtualRect"
    v-model="model"
    v-model:open="open"
    :items="items"
    multiple
    :creatable="canCreate"
    :create-label="t('task.createLabel')"
    :placeholder="t('task.addLabels')"
    :width="240"
    @create="onCreate"
  >
    <template v-if="$slots.default" #default><slot /></template>
  </UiCombobox>
</template>
