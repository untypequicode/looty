<script setup lang="ts">
import { FileText, Kanban, CalendarDays, Check } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useAccess, type Area } from '~/composables/useAccess'
import { useI18n } from '~/composables/useI18n'

/**
 * Première visite d'un espace : « Sur quoi travaillez-vous ? ». Les domaines cochés passent
 * en mode lecture (un clic pour modifier), les autres en mode restreint (déverrouillage
 * temporaire). Modifiable ensuite dans les réglages de l'espace.
 */
const { wsId, workspace } = useWorkspace()
const { t } = useI18n()
const { chooseAreas, allows, modesChosen, prefsLoaded, perms } = useAccess(wsId)
const areas = computed(() => ([
  { key: 'docs' as Area, icon: FileText, label: t('access.areas.docs'), hint: t('access.areas.docsHint') },
  { key: 'boards' as Area, icon: Kanban, label: t('access.areas.boards'), hint: t('access.areas.boardsHint') },
  { key: 'calendar' as Area, icon: CalendarDays, label: t('access.areas.calendar'), hint: t('access.areas.calendarHint') },
]).filter((a) => allows(a.key, 'edit')))
const open = computed(() => prefsLoaded.value && !!perms.value && !modesChosen.value && areas.value.length > 0)
const picked = ref<Area[]>([])
watch(areas, (a) => { if (!picked.value.length) picked.value = a.map((x) => x.key) }, { immediate: true })
function toggle(a: Area) { picked.value = picked.value.includes(a) ? picked.value.filter((x) => x !== a) : [...picked.value, a] }
const saving = ref(false)
async function save() {
  saving.value = true
  try { await chooseAreas(picked.value) } finally { saving.value = false }
}
</script>

<template>
  <UiDialog :open="open" :title="t('access.workOnTitle', { name: workspace.data.value?.name ?? '' })" :description="t('access.workOnHint')" :width="520" :closable="false">
    <div class="wo">
      <button v-for="a in areas" :key="a.key" class="wo-card" :class="{ on: picked.includes(a.key) }" :aria-pressed="picked.includes(a.key)" @click="toggle(a.key)">
        <component :is="a.icon" :size="20" />
        <span class="wo-text"><b>{{ a.label }}</b><small>{{ a.hint }}</small></span>
        <span class="wo-check"><Check v-if="picked.includes(a.key)" :size="14" /></span>
      </button>
      <p class="wo-note">{{ t('access.workOnNote') }}</p>
    </div>
    <template #footer>
      <UiButton variant="primary" :loading="saving" @click="save">{{ t('access.workOnSave') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.wo { display: flex; flex-direction: column; gap: 8px; }
.wo-card { display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-radius: var(--r-md); text-align: left; box-shadow: inset 0 0 0 1px var(--border-strong); color: var(--text-2); }
.wo-card:hover { background: var(--bg-hover); }
.wo-card.on { box-shadow: inset 0 0 0 1.5px var(--accent); background: var(--accent-soft); color: var(--text); }
.wo-text { flex: 1; display: flex; flex-direction: column; gap: 2px; }
.wo-text small { color: var(--text-3); font-size: var(--fs-sm); }
.wo-check { width: 20px; height: 20px; border-radius: 50%; display: grid; place-items: center; box-shadow: inset 0 0 0 1.5px var(--border-strong); color: #fff; }
.wo-card.on .wo-check { background: var(--accent); box-shadow: none; }
.wo-note { margin: 6px 0 0; font-size: var(--fs-sm); color: var(--text-3); }
</style>
