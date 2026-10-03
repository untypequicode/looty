<script setup lang="ts">
import { useWorkspace } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'

const model = defineModel<string[]>({ required: true })
const open = defineModel<boolean>('open', { default: false })
const { members } = useWorkspace()
const { user } = useAuth()
defineProps<{ virtualRect?: { x: number; y: number } | null }>()
const { t } = useI18n()
const items = computed(() =>
  [...(members.data.value ?? [])]
    .sort((a, b) => (a.user_id === user.value?.id ? -1 : b.user_id === user.value?.id ? 1 : a.name.localeCompare(b.name)))
    .map((m) => ({ value: m.user_id, label: m.user_id === user.value?.id ? `${m.name} (${t('common.me')})` : m.name, keywords: m.email })),
)
const byId = computed(() => new Map((members.data.value ?? []).map((m) => [m.user_id, m])))
</script>

<template>
  <UiCombobox :virtual-rect="virtualRect" v-model="model" v-model:open="open" :items="items" multiple :placeholder="t('task.assignTo')" :width="260">
    <template v-if="$slots.default" #default><slot /></template>
    <template #item="{ item }">
      <UiAvatar :name="byId.get(item.value as string)?.name" :src="byId.get(item.value as string)?.avatar_url" :size="18" />
    </template>
  </UiCombobox>
</template>
