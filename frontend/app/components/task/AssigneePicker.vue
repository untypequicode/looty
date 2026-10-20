<script setup lang="ts">
import { useWorkspace } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { useGroupChoices } from '~/composables/useGroups'

const model = defineModel<string[]>({ required: true })
const open = defineModel<boolean>('open', { default: false })
const { wsId, members } = useWorkspace()
const { user } = useAuth()
defineProps<{ virtualRect?: { x: number; y: number } | null }>()
const { t } = useI18n()
// Groupes : les choisir ajoute leurs membres (la tâche ne garde que les personnes).
const groupChoices = useGroupChoices(wsId, () => model.value)
const people = computed(() =>
  [...(members.data.value ?? [])]
    .sort((a, b) => (a.user_id === user.value?.id ? -1 : b.user_id === user.value?.id ? 1 : a.name.localeCompare(b.name)))
    .map((m) => ({ value: m.user_id, label: m.user_id === user.value?.id ? `${m.name} (${t('common.me')})` : m.name, keywords: m.email })),
)
const items = computed(() => {
  const groups = groupChoices.items.value
  return groups.length ? [...people.value.map((p) => ({ ...p, section: t('memberGroups.people') })), ...groups] : people.value
})
const selection = computed({
  get: () => model.value,
  set: (v: string[]) => { model.value = groupChoices.expand(v) },
})
const byId = computed(() => new Map((members.data.value ?? []).map((m) => [m.user_id, m])))
</script>

<template>
  <UiCombobox :virtual-rect="virtualRect" v-model="selection" v-model:open="open" :items="items" multiple :placeholder="t('task.assignTo')" :width="260">
    <template v-if="$slots.default" #default><slot /></template>
    <template #item="{ item }">
      <UiAvatar v-if="byId.has(item.value as string)" :name="byId.get(item.value as string)?.name" :src="byId.get(item.value as string)?.avatar_url" :size="18" />
      <span v-else class="ap-group" :style="{ background: item.color }">@</span>
    </template>
  </UiCombobox>
</template>

<style scoped>
.ap-group { width: 18px; height: 18px; border-radius: 50%; flex: none; display: grid; place-items: center; color: #fff; font-size: 10px; font-weight: 700; }
</style>
