<script setup lang="ts">
import { Lock, Check, ChevronDown } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import type { Page } from '~/types'
import { api } from '~/composables/api'
import { useWorkspaces, wsKeys } from '~/composables/useWorkspace'
import { usePersonalSpace } from '~/composables/usePersonal'
import { moveRequest, useMovePage } from '~/composables/useMovePage'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'

/** « Déplacer vers… » : une page et ses sous-pages changent d'espace (perso compris). */
const { t } = useI18n()
const { user } = useAuth()
const { data: workspaces } = useWorkspaces()
const personal = usePersonalSpace()
const { move } = useMovePage()

const open = computed({ get: () => !!moveRequest.value, set: (v) => { if (!v) moveRequest.value = null } })
const target = ref('')
const parent = ref<string | null>(null)
const parentOpen = ref(false)
const busy = ref(false)
watch(moveRequest, () => { target.value = ''; parent.value = null })

/** Espaces où l'on peut créer des documents : l'espace perso en tête. */
const spaces = computed(() => {
  const src = moveRequest.value?.ws
  const list = (workspaces.value ?? []).filter((w) => w.id !== src && w.role !== 'guest' && w.role !== 'viewer' && !w.personal)
  const perso = personal.id.value && personal.id.value !== src ? [{ id: personal.id.value, name: t('personal.title'), personal: true }] : []
  return [...perso, ...list.map((w) => ({ id: w.id, name: w.name, personal: false }))]
})

// Emplacement dans l'espace choisi : à la racine ou sous une page modifiable.
const targetPages = useQuery({
  queryKey: computed(() => wsKeys.pages(target.value)),
  queryFn: () => api<Page[]>(`/api/workspaces/${target.value}/pages`),
  enabled: computed(() => !!target.value),
})
const parentItems = computed(() => [
  { value: null, label: t('move.root') },
  ...(targetPages.data.value ?? []).filter((p) => p.can_edit !== false).map((p) => ({ value: p.id as string | null, label: `${p.icon ? `${p.icon} ` : ''}${p.title || t('docs.untitled')}` })),
])
const parentLabel = computed(() => parentItems.value.find((i) => i.value === parent.value)?.label ?? t('move.root'))

async function submit() {
  const req = moveRequest.value
  if (!req || !target.value) return
  busy.value = true
  try {
    await move(req.ws, req.id, target.value, parent.value)
    moveRequest.value = null
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('move.title', { name: moveRequest?.title || t('docs.untitled') })" :description="t('move.hint')">
    <div class="mv">
      <div class="mv-label">{{ t('move.to') }}</div>
      <div class="mv-spaces" role="radiogroup">
        <button v-for="s in spaces" :key="s.id" class="mv-space" :class="{ on: target === s.id }" role="radio" :aria-checked="target === s.id" @click="target = s.id; parent = null">
          <UiAvatar v-if="s.personal" :name="user?.name" :src="user?.avatar_url" :size="20" />
          <span v-else class="mv-logo">{{ s.name[0]?.toUpperCase() }}</span>
          <span class="truncate mv-name">{{ s.name }}</span>
          <Lock v-if="s.personal" :size="12" class="faint" />
          <Check v-if="target === s.id" :size="15" class="mv-check" />
        </button>
        <p v-if="!spaces.length" class="faint">{{ t('move.noSpace') }}</p>
      </div>
      <template v-if="target">
        <div class="mv-label">{{ t('move.where') }}</div>
        <UiCombobox v-model:open="parentOpen" :model-value="parent" :items="parentItems" :width="360" :placeholder="t('move.searchPage')" @update:model-value="(v) => (parent = v as string | null)">
          <button class="mv-select"><span class="truncate">{{ parentLabel }}</span><ChevronDown :size="14" class="faint" /></button>
        </UiCombobox>
      </template>
      <p class="mv-note faint">{{ t('move.note') }}</p>
    </div>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="busy" :disabled="!target" @click="submit">{{ t('move.submit') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.mv { display: flex; flex-direction: column; gap: 8px; }
.mv-label { font-size: var(--fs-sm); font-weight: 500; color: var(--text-2); margin-top: 4px; }
.mv-spaces { display: flex; flex-direction: column; gap: 2px; max-height: 240px; overflow-y: auto; }
.mv-space { display: flex; align-items: center; gap: 10px; height: 36px; padding: 0 10px; border-radius: var(--r-sm); text-align: left; border: 1px solid transparent; }
.mv-space:hover { background: var(--bg-hover); }
.mv-space.on { background: var(--accent-soft); border-color: color-mix(in srgb, var(--accent) 35%, transparent); }
.mv-name { flex: 1; }
.mv-check { color: var(--accent); }
.mv-logo { width: 20px; height: 20px; border-radius: 5px; flex: none; display: grid; place-items: center; font-size: 11px; font-weight: 700; color: #fff; background: linear-gradient(135deg, var(--accent), #b18cff); }
.mv-select { display: flex; align-items: center; justify-content: space-between; gap: 8px; width: 100%; height: 34px; padding: 0 10px; border: 1px solid var(--border); border-radius: var(--r-sm); background: var(--bg-input); }
.mv-note { font-size: var(--fs-sm); margin: 6px 0 0; }
</style>
