<script setup lang="ts">
import { Lock, LockOpen, Eye, PencilLine } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useAccess, type Area } from '~/composables/useAccess'
import { useI18n } from '~/composables/useI18n'

/**
 * État d'édition d'un domaine dans l'en-tête des pages : rien en mode « écriture » ;
 * « Lecture · Modifier » en mode lecture ; « Verrouillé · Déverrouiller » en mode restreint
 * (déverrouillage valable jusqu'à la fermeture de la fenêtre).
 */
const props = defineProps<{ area: Area }>()
const { wsId } = useWorkspace()
const { t } = useI18n()
const { mode, editing, toggleEditing, allows } = useAccess(wsId)
const m = computed(() => mode(props.area))
const on = computed(() => editing(props.area))
const visible = computed(() => allows(props.area, 'edit') && m.value !== 'write')
const label = computed(() => {
  if (m.value === 'restricted') return on.value ? t('access.lockAgain') : t('access.unlock')
  return on.value ? t('access.stopEditing') : t('access.edit')
})
const hint = computed(() => (m.value === 'restricted' ? (on.value ? t('access.unlockedHint') : t('access.restrictedHint')) : on.value ? t('access.editingHint') : t('access.readHint')))
const icon = computed(() => (m.value === 'restricted' ? (on.value ? LockOpen : Lock) : on.value ? Eye : PencilLine))
</script>

<template>
  <UiTooltip v-if="visible" :text="hint">
    <UiButton size="sm" :variant="on ? 'ghost' : 'secondary'" :icon="icon" class="et" :class="[m, { on }]" @click="toggleEditing(area)">{{ label }}</UiButton>
  </UiTooltip>
</template>

<style scoped>
.et.restricted:not(.on) { color: var(--orange, #e2a03f); }
.et.on.restricted { color: var(--green); }
</style>
