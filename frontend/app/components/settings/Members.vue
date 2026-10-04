<script setup lang="ts">
import { UserPlus, Copy, Trash2, Search, Link2, UserMinus, Activity } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Role, WorkspaceMember } from '~/types'
import { useWorkspace, wsKeys } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { presence } from '~/composables/useRealtime'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

const { wsId, members, role, isAdmin } = useWorkspace()
const { user } = useAuth()
const { t, relativeTime, formatDate } = useI18n()
const qc = useQueryClient()
const q = ref('')

const list = computed(() => (members.data.value ?? []).filter((m) => `${m.name} ${m.email}`.toLowerCase().includes(q.value.toLowerCase())))
const roleItems = computed(() => (['owner', 'admin', 'member', 'viewer'] as Role[])
  .filter((r) => role.value === 'owner' || (r !== 'owner' && r !== 'admin'))
  .map((r) => ({ value: r, label: t(`roles.${r}`), hint: t(`settings.members.roleHint.${r}`) })))
function canManage(m: WorkspaceMember) {
  if (!isAdmin.value || m.user_id === user.value?.id) return false
  return role.value === 'owner' || (m.role !== 'owner' && m.role !== 'admin')
}
async function setRole(m: WorkspaceMember, r: Role) {
  if (r === m.role) return
  await api(`/api/workspaces/${wsId.value}/members/${m.user_id}`, { method: 'PUT', body: { role: r } })
  qc.invalidateQueries({ queryKey: wsKeys.members(wsId.value) })
  toast.success(t('settings.members.roleChanged', { name: m.name, role: t(`roles.${r}`) }))
}
async function removeMember(m: WorkspaceMember) {
  if (!(await confirmDialog({ title: t('settings.members.removeTitle', { name: m.name }), message: t('settings.members.removeMessage'), danger: true, confirmLabel: t('common.remove') }))) return
  await api(`/api/workspaces/${wsId.value}/members/${m.user_id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: wsKeys.members(wsId.value) })
}
const roleOpen = ref<Record<string, boolean>>({})

// --- Invitations ---
interface Invite { id: string; role: Role; created_by_name?: string | null; expires_at?: string | null; max_uses?: number | null; uses: number; created_at: string }
const { data: invites } = useQuery({
  queryKey: computed(() => ['ws', wsId.value, 'invites']),
  queryFn: () => api<Invite[]>(`/api/workspaces/${wsId.value}/invites`),
  enabled: isAdmin,
})
const inviteOpen = ref(false)
const inviteRole = ref<Role>('member')
const inviteDays = ref('7')
const inviteUses = ref('')
const inviteLink = ref<string | null>(null)
const creating = ref(false)
async function createInvite() {
  creating.value = true
  try {
    const r = await api<{ token: string }>(`/api/workspaces/${wsId.value}/invites`, {
      method: 'POST',
      body: { role: inviteRole.value, expires_in_days: Number(inviteDays.value), max_uses: inviteUses.value ? Number(inviteUses.value) : null },
    })
    inviteLink.value = `${location.origin}/invite/${r.token}`
    qc.invalidateQueries({ queryKey: ['ws', wsId.value, 'invites'] })
  } finally { creating.value = false }
}
function copyLink() { navigator.clipboard.writeText(inviteLink.value!); toast.success(t('common.linkCopied')) }
async function revoke(i: Invite) {
  await api(`/api/workspaces/${wsId.value}/invites/${i.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['ws', wsId.value, 'invites'] })
}
function openInvite() { inviteLink.value = null; inviteRole.value = 'member'; inviteDays.value = '7'; inviteUses.value = ''; inviteOpen.value = true }
const inviteRoleItems = computed(() => roleItems.value.filter((r) => r.value !== 'owner'))
const expiryItems = computed(() => [
  { value: '1', label: t('settings.members.expiry.day') }, { value: '7', label: t('settings.members.expiry.week') },
  { value: '30', label: t('settings.members.expiry.month') }, { value: '0', label: t('settings.members.expiry.never') },
])
</script>

<template>
  <SettingsSection :description="t('settings.members.description', { count: members.data.value?.length ?? 0 })">
    <template #actions>
      <UiInput v-model="q" :icon="Search" size="sm" :placeholder="t('common.search')" style="width: 180px" />
      <UiButton v-if="isAdmin" size="sm" variant="primary" :icon="UserPlus" @click="openInvite">{{ t('workspace.inviteMembers') }}</UiButton>
    </template>
    <div v-for="m in list" :key="m.user_id" class="member">
      <UiAvatar :name="m.name" :src="m.avatar_url" :size="32" :online="!!presence[m.user_id]" />
      <div class="m-body">
        <div class="m-name truncate">{{ m.name }} <span v-if="m.user_id === user?.id" class="faint">({{ t('common.you') }})</span></div>
        <div class="m-mail truncate">{{ m.email }}</div>
      </div>
      <span class="m-joined">{{ m.joined_at ? t('settings.members.joined', { time: relativeTime(m.joined_at) }) : '' }}</span>
      <UiTooltip :text="t('presence.activity')"><UiButton size="sm" variant="ghost" :icon="Activity" :to="`/w/${wsId}/activity?user=${m.user_id}`" :aria-label="t('presence.activity')" /></UiTooltip>
      <UiCombobox v-if="canManage(m)" v-model:open="roleOpen[m.user_id]" :model-value="m.role" :items="roleItems" :width="280" placement="bottom-end" @update:model-value="(r) => setRole(m, r as Role)">
        <button class="role-btn">{{ t(`roles.${m.role}`) }}</button>
      </UiCombobox>
      <span v-else class="role-static">{{ t(`roles.${m.role}`) }}</span>
      <UiButton v-if="canManage(m)" size="sm" variant="ghost" :icon="UserMinus" :title="t('common.remove')" @click="removeMember(m)" />
    </div>
  </SettingsSection>

  <SettingsSection v-if="isAdmin" :title="t('settings.members.invites')" :description="t('settings.members.invitesHint')">
    <SettingsRow v-for="i in invites ?? []" :key="i.id" :label="t(`roles.${i.role}`)" :hint="[
      t('settings.members.createdBy', { name: i.created_by_name ?? '?', time: relativeTime(i.created_at) }),
      i.expires_at ? t('settings.members.expires', { date: formatDate(i.expires_at) }) : t('settings.members.noExpiry'),
      i.max_uses ? t('settings.members.uses', { uses: i.uses, max: i.max_uses }) : t('settings.members.usesUnlimited', { uses: i.uses }),
    ].join(' · ')">
      <UiButton size="sm" variant="ghost" :icon="Trash2" @click="revoke(i)">{{ t('common.revoke') }}</UiButton>
    </SettingsRow>
    <div v-if="!invites?.length" class="empty"><Link2 :size="16" /> {{ t('settings.members.noInvites') }}</div>
  </SettingsSection>

  <UiDialog v-model:open="inviteOpen" :title="t('workspace.inviteMembers')" :description="t('settings.members.inviteDialogHint')" :width="480">
    <template v-if="inviteLink">
      <div class="link"><input :value="inviteLink" readonly @focus="($event.target as HTMLInputElement).select()" /><UiButton size="sm" variant="primary" :icon="Copy" @click="copyLink">{{ t('common.copy') }}</UiButton></div>
      <p class="faint small">{{ t('settings.members.linkOnce') }}</p>
    </template>
    <div v-else class="form">
      <div><label class="field-label">{{ t('settings.members.role') }}</label>
        <UiCombobox v-model="inviteRole" :items="inviteRoleItems" :width="280"><button class="select">{{ t(`roles.${inviteRole}`) }}</button></UiCombobox>
      </div>
      <div><label class="field-label">{{ t('settings.members.expiration') }}</label>
        <UiCombobox v-model="inviteDays" :items="expiryItems" :width="220"><button class="select">{{ expiryItems.find((e) => e.value === inviteDays)?.label }}</button></UiCombobox>
      </div>
      <div><label class="field-label">{{ t('settings.members.maxUses') }}</label>
        <UiInput v-model="inviteUses" type="number" min="1" :placeholder="t('settings.members.unlimited')" style="width: 160px" />
      </div>
    </div>
    <template #footer>
      <UiButton v-if="inviteLink" variant="primary" @click="inviteOpen = false">{{ t('common.done') }}</UiButton>
      <UiButton v-else variant="primary" :loading="creating" @click="createInvite">{{ t('settings.members.createLink') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.member { display: flex; align-items: center; gap: 12px; padding: 10px 16px; }
.member + .member { border-top: 1px solid var(--border); }
.m-body { flex: 1; min-width: 0; }
.m-name { font-weight: 500; }
.m-mail { font-size: var(--fs-sm); color: var(--text-3); }
.m-joined { font-size: var(--fs-sm); color: var(--text-3); white-space: nowrap; }
.role-btn, .select { height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); font-size: var(--fs-sm); background: var(--bg-input); }
.select { min-width: 200px; text-align: left; }
.role-static { font-size: var(--fs-sm); color: var(--text-2); padding: 0 10px; }
.empty { display: flex; align-items: center; gap: 8px; padding: 18px 16px; color: var(--text-3); }
.form { display: flex; flex-direction: column; gap: 14px; }
.link { display: flex; gap: 8px; }
.link input { flex: 1; min-width: 0; height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-sm); outline: none; }
.small { font-size: var(--fs-sm); margin: 10px 0 0; }
@media (max-width: 639px) { .m-joined { display: none; } }
</style>
