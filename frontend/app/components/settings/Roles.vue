<script setup lang="ts">
import { Plus, Pencil, Trash2, Shield, Crown } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { useWorkspace, wsKeys } from '~/composables/useWorkspace'
import { useAccess, useRoles, permSummary, AREAS, type WorkspaceRole, type Permissions, type LevelName, type Area } from '~/composables/useAccess'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

/**
 * Rôles de l'espace : les préréglages (propriétaire, administrateur, membre, lecteur) se
 * personnalisent comme les rôles créés ; chaque domaine a son niveau, plus trois droits.
 */
const { wsId, role } = useWorkspace()
const { can } = useAccess(wsId)
const { t } = useI18n()
const qc = useQueryClient()
const { data: roles } = useRoles(wsId)

const LEVELS: Record<Area, LevelName[]> = {
  docs: ['none', 'read', 'comment', 'edit'],
  boards: ['none', 'read', 'comment', 'validate', 'edit', 'manage'],
  calendar: ['none', 'read', 'edit'],
}
const FLAGS = ['invite', 'share', 'groups', 'admin'] as const
const COLORS = ['#e2a03f', '#eb5757', '#5e6ad2', '#8a8f98', '#4cb782', '#26b5ce', '#bb87fc', '#e87ba4', '#f2994a', '#2a78d6']
const label = (r: WorkspaceRole) => (r.preset ? t(`roles.${r.preset}`) : r.name)
const editable = (r: WorkspaceRole) => can.value.admin && r.preset !== 'owner' && (role.value === 'owner' || !r.permissions.admin)

const open = ref(false)
const editing = ref<WorkspaceRole | null>(null)
const form = reactive<{ name: string; color: string; permissions: Permissions }>({ name: '', color: COLORS[2]!, permissions: { docs: 'read', boards: 'read', calendar: 'read', invite: false, share: false, admin: false, groups: false } })
function edit(r: WorkspaceRole | null) {
  editing.value = r
  form.name = r ? label(r) : ''
  form.color = r?.color ?? COLORS[Math.floor(Math.random() * COLORS.length)]!
  form.permissions = r ? { ...r.permissions } : { docs: 'read', boards: 'read', calendar: 'read', invite: false, share: false, admin: false, groups: false }
  open.value = true
}
const saving = ref(false)
async function save() {
  saving.value = true
  try {
    const body = { name: editing.value?.preset ? undefined : form.name, color: form.color, permissions: form.permissions }
    if (editing.value) await api(`/api/workspaces/${wsId.value}/access/roles/${editing.value.id}`, { method: 'PATCH', body })
    else await api(`/api/workspaces/${wsId.value}/access/roles`, { method: 'POST', body: { ...body, name: form.name } })
    qc.invalidateQueries({ queryKey: ['ws', wsId.value, 'roles'] })
    qc.invalidateQueries({ queryKey: wsKeys.members(wsId.value) })
    toast.success(t('common.saved'))
    open.value = false
  } finally { saving.value = false }
}
async function remove(r: WorkspaceRole) {
  if (!(await confirmDialog({ title: t('rolesUi.deleteTitle', { name: r.name }), message: t('rolesUi.deleteHint'), danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/workspaces/${wsId.value}/access/roles/${r.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['ws', wsId.value, 'roles'] })
  qc.invalidateQueries({ queryKey: wsKeys.members(wsId.value) })
}
</script>

<template>
  <SettingsSection :description="t('rolesUi.description')">
    <template #actions>
      <UiButton v-if="can.admin" size="sm" variant="primary" :icon="Plus" @click="edit(null)">{{ t('rolesUi.new') }}</UiButton>
    </template>
    <div v-for="r in roles ?? []" :key="r.id" class="role">
      <span class="r-badge" :style="{ background: r.color }"><component :is="r.preset === 'owner' ? Crown : Shield" :size="13" /></span>
      <div class="r-body">
        <div class="r-name">{{ label(r) }} <span v-if="r.preset" class="r-tag">{{ t('rolesUi.preset') }}</span></div>
        <div class="r-perms">{{ permSummary(r.permissions, t) }}</div>
      </div>
      <span class="r-count">{{ t('rolesUi.members', { n: r.member_count }) }}</span>
      <UiButton v-if="editable(r)" size="sm" variant="ghost" :icon="Pencil" :aria-label="t('common.edit')" @click="edit(r)" />
      <UiButton v-if="editable(r) && !r.preset" size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove(r)" />
    </div>
  </SettingsSection>

  <UiDialog v-model:open="open" :title="editing ? t('rolesUi.editTitle', { name: label(editing) }) : t('rolesUi.new')" :width="560">
    <div class="rf">
      <div class="rf-row">
        <UiInput v-model="form.name" :disabled="!!editing?.preset" :placeholder="t('rolesUi.namePlaceholder')" maxlength="60" class="rf-name" />
        <div class="rf-colors">
          <button v-for="c in COLORS" :key="c" class="sw" :class="{ on: form.color === c }" :style="{ background: c }" :aria-label="c" @click="form.color = c" />
        </div>
      </div>
      <div v-for="a in AREAS" :key="a" class="rf-area">
        <div class="rf-label"><b>{{ t(`access.areas.${a}`) }}</b><small>{{ t(`rolesUi.levelHint.${a}.${form.permissions[a]}`) }}</small></div>
        <UiSegmented v-model="form.permissions[a]" size="md" :options="LEVELS[a].map((l) => ({ value: l, label: t(`access.levels.${l}`) }))" />
      </div>
      <label v-for="f in FLAGS" :key="f" class="rf-flag">
        <UiSwitch v-model="form.permissions[f]" :disabled="f === 'admin' && role !== 'owner'" />
        <span><b>{{ t(`rolesUi.flags.${f}`) }}</b><small>{{ t(`rolesUi.flags.${f}Hint`) }}</small></span>
      </label>
    </div>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="saving" :disabled="!form.name.trim()" @click="save">{{ t('common.save') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.role { display: flex; align-items: center; gap: 12px; padding: 10px 16px; }
.role + .role { border-top: 1px solid var(--border); }
.r-badge { width: 28px; height: 28px; border-radius: 8px; display: grid; place-items: center; color: #fff; flex-shrink: 0; }
.r-body { flex: 1; min-width: 0; }
.r-name { font-weight: 500; display: flex; align-items: center; gap: 6px; }
.r-tag { font-size: 10.5px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-3); }
.r-perms { font-size: var(--fs-sm); color: var(--text-3); }
.r-count { font-size: var(--fs-sm); color: var(--text-3); white-space: nowrap; }
.rf { display: flex; flex-direction: column; gap: 14px; }
.rf-row { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
.rf-name { flex: 1; min-width: 180px; }
.rf-colors { display: flex; gap: 5px; }
.sw { width: 18px; height: 18px; border-radius: 50%; }
.sw.on { box-shadow: 0 0 0 2px var(--bg-elevated), 0 0 0 4px var(--text-2); }
.rf-area { display: flex; flex-direction: column; gap: 6px; }
.rf-label { display: flex; flex-direction: column; gap: 1px; }
.rf-label small, .rf-flag small { color: var(--text-3); font-size: var(--fs-sm); }
.rf-flag { display: flex; align-items: flex-start; gap: 10px; cursor: pointer; }
.rf-flag span { display: flex; flex-direction: column; gap: 1px; }
@media (max-width: 639px) { .r-count { display: none; } }
</style>
