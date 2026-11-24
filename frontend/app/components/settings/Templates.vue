<script setup lang="ts">
import { LayoutTemplate, Trash2 } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useWorkspace } from '~/composables/useWorkspace'
import { useTemplates, templatesKey } from '~/composables/useTemplates'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

/** Modèles de l'espace (GLOSSARY : Modèle) : ceux que les administrateurs ont enregistrés, et les modèles fournis. */
const { wsId, isAdmin } = useWorkspace()
const { data } = useTemplates(wsId)
const { t } = useI18n()
const qc = useQueryClient()
const own = computed(() => (data.value ?? []).filter((x) => !x.builtin))
const provided = computed(() => (data.value ?? []).filter((x) => x.builtin))

async function remove(x: { id: string; name: string }) {
  if (!(await confirmDialog({ title: t('tpl.deleteTitle', { name: x.name }), message: t('tpl.deleteMessage'), confirmLabel: t('common.delete'), danger: true }))) return
  await api(`/api/workspaces/${wsId.value}/templates/${x.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: templatesKey(wsId.value) })
  toast.success(t('tpl.deleted'))
}
</script>

<template>
  <SettingsSection :description="t('tpl.description')">
    <div v-if="!own.length" class="empty"><LayoutTemplate :size="16" /> {{ t('tpl.empty') }}</div>
    <SettingsRow v-for="x in own" :key="x.id" :label="x.name" :hint="t(x.kind === 'board' ? 'tpl.boards' : 'tpl.pages')">
      <UiButton v-if="isAdmin" size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove(x)" />
    </SettingsRow>
  </SettingsSection>
  <SettingsSection v-if="provided.length" :title="t('tpl.builtin')">
    <SettingsRow v-for="x in provided" :key="x.id" :label="x.name" :hint="x.columns?.length ? x.columns.join(' · ') : t('tpl.pages')" />
  </SettingsSection>
</template>

<style scoped>
.empty { display: flex; align-items: center; gap: 8px; padding: 18px 16px; color: var(--text-3); }
</style>
