<script setup lang="ts">
import { UserPlus, Copy, Trash2, Search, Link2, UserMinus, Activity, Mail, Check } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Role, WorkspaceMember } from '~/types'
import { useWorkspace, wsKeys } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { presence } from '~/composables/useRealtime'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { useAccess, useRoles, permSummary, type WorkspaceRole } from '~/composables/useAccess'
import { useDebounceFn } from '@vueuse/core'

const { wsId, members, role, isAdmin } = useWorkspace()
const { can } = useAccess(wsId)
const { data: roles } = useRoles(wsId)
/** Rôles attribuables : jamais le propriétaire ; les rôles d'administration par le propriétaire seulement. */
const assignable = computed(() => (roles.value ?? []).filter((r) => r.preset !== 'owner' && (role.value === 'owner' || !r.permissions.admin)))
const roleLabel = (r: WorkspaceRole) => (r.preset ? t(`roles.${r.preset}`) : r.name)
const roleChoices = computed(() => assignable.value.map((r) => ({ value: r.id, label: roleLabel(r), hint: permSummary(r.permissions, t) })))
const memberRole = (m: WorkspaceMember) => (roles.value ?? []).find((r) => r.id === m.role_id)
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
async function setRole(m: WorkspaceMember, roleId: string) {
  if (roleId === m.role_id) return
  await api(`/api/workspaces/${wsId.value}/members/${m.user_id}`, { method: 'PUT', body: { role_id: roleId } })
  qc.invalidateQueries({ queryKey: wsKeys.members(wsId.value) })
  const r = roles.value?.find((x) => x.id === roleId)
  toast.success(t('settings.members.roleChanged', { name: m.name, role: r ? roleLabel(r) : '' }))
}

// --- Ajout direct (compte existant) ou invitation par e-mail ---
const addOpen = ref(false)
const addQuery = ref('')
const addRole = ref<string | null>(null)
const addHits = ref<{ id: string; name: string; email: string; avatar_url?: string | null }[]>([])
const adding = ref(false)
const search = useDebounceFn(async () => {
  const q = addQuery.value.trim()
  addHits.value = q.length >= 2 ? await api(`/api/users/search?q=${encodeURIComponent(q)}&exclude_ws=${wsId.value}`, { silent: true }).catch(() => []) : []
}, 250)
watch(addQuery, search)
const looksLikeEmail = computed(() => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(addQuery.value.trim()))
function openAdd() {
  addQuery.value = ''
  addHits.value = []
  addRole.value = roles.value?.find((r) => r.preset === 'member')?.id ?? null
  addOpen.value = true
}
async function addPerson(body: { user_id?: string; email?: string }) {
  adding.value = true
  try {
    const r = await api<{ added?: string; invited?: string; email_sent?: boolean; link?: string }>(`/api/workspaces/${wsId.value}/access/members/add`, { method: 'POST', body: { ...body, role_id: addRole.value } })
    if (r.added) toast.success(t('settings.members.added'))
    else if (r.email_sent) toast.success(t('settings.members.invitedByEmail', { email: r.invited }))
    else if (r.link) { navigator.clipboard.writeText(r.link).catch(() => {}); toast(t('settings.members.inviteNoSmtp')) }
    qc.invalidateQueries({ queryKey: wsKeys.members(wsId.value) })
    qc.invalidateQueries({ queryKey: ['ws', wsId.value, 'invites'] })
    addOpen.value = false
  } finally { adding.value = false }
}
async function removeMember(m: WorkspaceMember) {
  if (!(await confirmDialog({ title: t('settings.members.removeTitle', { name: m.name }), message: t('settings.members.removeMessage'), danger: true, confirmLabel: t('common.remove') }))) return
  await api(`/api/workspaces/${wsId.value}/members/${m.user_id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: wsKeys.members(wsId.value) })
}
const roleOpen = ref<Record<string, boolean>>({})

// --- Invitations ---
interface Invite { id: string; role: Role; role_id?: string | null; created_by_name?: string | null; expires_at?: string | null; max_uses?: number | null; uses: number; created_at: string }
const { data: invites } = useQuery({
  queryKey: computed(() => ['ws', wsId.value, 'invites']),
  queryFn: () => api<Invite[]>(`/api/workspaces/${wsId.value}/invites`),
  enabled: isAdmin,
})
const inviteOpen = ref(false)
const inviteRole = ref<string | null>(null)
const inviteDays = ref('7')
const inviteUses = ref('')
const inviteLink = ref<string | null>(null)
const creating = ref(false)
async function createInvite() {
  creating.value = true
  try {
    const r = await api<{ token: string }>(`/api/workspaces/${wsId.value}/invites`, {
      method: 'POST',
      body: { role_id: inviteRole.value, expires_in_days: Number(inviteDays.value), max_uses: inviteUses.value ? Number(inviteUses.value) : null },
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
function openInvite() { inviteLink.value = null; inviteRole.value = roles.value?.find((r) => r.preset === 'member')?.id ?? null; inviteDays.value = '7'; inviteUses.value = ''; inviteOpen.value = true }
const inviteRoleItems = roleChoices
const roleName = (id: string | null | undefined, fallback?: string) => { const r = roles.value?.find((x) => x.id === id); return r ? roleLabel(r) : fallback ? t(`roles.${fallback}`) : '' }
const expiryItems = computed(() => [
  { value: '1', label: t('settings.members.expiry.day') }, { value: '7', label: t('settings.members.expiry.week') },
  { value: '30', label: t('settings.members.expiry.month') }, { value: '0', label: t('settings.members.expiry.never') },
])
</script>

<template>
  <SettingsSection :description="t('settings.members.description', { count: members.data.value?.length ?? 0 })">
    <template #actions>
      <UiInput v-model="q" :icon="Search" size="sm" :placeholder="t('common.search')" style="width: 180px" />
      <UiButton v-if="can.invite" size="sm" :icon="Link2" @click="openInvite">{{ t('settings.members.inviteLink') }}</UiButton>
      <UiButton v-if="can.invite" size="sm" variant="primary" :icon="UserPlus" @click="openAdd">{{ t('settings.members.addPeople') }}</UiButton>
    </template>
    <div v-for="m in list" :key="m.user_id" class="member">
      <UiAvatar :name="m.name" :src="m.avatar_url" :size="32" :online="!!presence[m.user_id]" />
      <div class="m-body">
        <div class="m-name truncate">{{ m.name }} <span v-if="m.user_id === user?.id" class="faint">({{ t('common.you') }})</span></div>
        <div class="m-mail truncate">{{ m.email }}</div>
      </div>
      <span class="m-joined">{{ m.joined_at ? t('settings.members.joined', { time: relativeTime(m.joined_at) }) : '' }}</span>
      <UiTooltip :text="t('presence.activity')"><UiButton size="sm" variant="ghost" :icon="Activity" :to="`/w/${wsId}/activity?user=${m.user_id}`" :aria-label="t('presence.activity')" /></UiTooltip>
      <span v-if="m.group_tasks === false" class="m-opt" :title="t('access.groupTasksOff')">{{ t('access.groupTasksOffShort') }}</span>
      <UiCombobox v-if="canManage(m)" v-model:open="roleOpen[m.user_id]" :model-value="m.role_id ?? null" :items="roleChoices" :width="340" placement="bottom-end" @update:model-value="(r) => setRole(m, r as string)">
        <button class="role-btn"><i class="r-dot" :style="{ background: m.role_color ?? undefined }" />{{ memberRole(m) ? roleLabel(memberRole(m)!) : t(`roles.${m.role}`) }}</button>
      </UiCombobox>
      <span v-else class="role-static"><i class="r-dot" :style="{ background: m.role_color ?? undefined }" />{{ memberRole(m) ? roleLabel(memberRole(m)!) : t(`roles.${m.role}`) }}</span>
      <UiButton v-if="canManage(m)" size="sm" variant="ghost" :icon="UserMinus" :title="t('common.remove')" @click="removeMember(m)" />
    </div>
  </SettingsSection>

  <SettingsSection v-if="can.invite" :title="t('settings.members.invites')" :description="t('settings.members.invitesHint')">
    <SettingsRow v-for="i in invites ?? []" :key="i.id" :label="roleName(i.role_id, i.role)" :hint="[
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
        <UiCombobox v-model="inviteRole" :items="inviteRoleItems" :width="340"><button class="select">{{ roleName(inviteRole) }}</button></UiCombobox>
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

  <UiDialog v-model:open="addOpen" :title="t('settings.members.addPeople')" :description="t('settings.members.addHint')" :width="480">
    <div class="form">
      <UiInput v-model="addQuery" :icon="Search" :placeholder="t('settings.members.addPlaceholder')" autofocus />
      <div class="hits">
        <button v-for="h in addHits" :key="h.id" class="hit" :disabled="adding" @click="addPerson({ user_id: h.id })">
          <UiAvatar :name="h.name" :src="h.avatar_url" :size="26" />
          <span class="hit-body"><b class="truncate">{{ h.name }}</b><small class="truncate">{{ h.email }}</small></span>
          <span class="hit-act"><UserPlus :size="14" /> {{ t('settings.members.add') }}</span>
        </button>
        <button v-if="looksLikeEmail && !addHits.some((h) => h.email.toLowerCase() === addQuery.trim().toLowerCase())" class="hit" :disabled="adding" @click="addPerson({ email: addQuery.trim() })">
          <span class="hit-ic"><Mail :size="15" /></span>
          <span class="hit-body"><b class="truncate">{{ addQuery.trim() }}</b><small>{{ t('settings.members.inviteByEmail') }}</small></span>
          <span class="hit-act"><Check :size="14" /> {{ t('settings.members.sendInvite') }}</span>
        </button>
        <p v-if="addQuery.trim().length >= 2 && !addHits.length && !looksLikeEmail" class="faint small">{{ t('settings.members.noAccountFound') }}</p>
      </div>
      <div><label class="field-label">{{ t('settings.members.role') }}</label>
        <UiCombobox v-model="addRole" :items="roleChoices" :width="340"><button class="select">{{ roleName(addRole) }}</button></UiCombobox>
      </div>
    </div>
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
.role-static { font-size: var(--fs-sm); color: var(--text-2); padding: 0 10px; display: inline-flex; align-items: center; }
.role-btn { display: inline-flex; align-items: center; }
.r-dot { width: 8px; height: 8px; border-radius: 50%; margin-right: 6px; background: var(--text-4); }
.m-opt { font-size: 11px; color: var(--text-3); padding: 1px 6px; border-radius: var(--r-full); background: var(--bg-active); white-space: nowrap; }
.hits { display: flex; flex-direction: column; gap: 2px; min-height: 8px; }
.hit { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; }
.hit:hover { background: var(--bg-hover); }
.hit-ic { width: 26px; height: 26px; display: grid; place-items: center; border-radius: 50%; background: var(--bg-active); }
.hit-body { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.hit-body small { color: var(--text-3); font-size: var(--fs-sm); }
.hit-act { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-sm); color: var(--accent); }
.empty { display: flex; align-items: center; gap: 8px; padding: 18px 16px; color: var(--text-3); }
.form { display: flex; flex-direction: column; gap: 14px; }
.link { display: flex; gap: 8px; }
.link input { flex: 1; min-width: 0; height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-sm); outline: none; }
.small { font-size: var(--fs-sm); margin: 10px 0 0; }
@media (max-width: 639px) { .m-joined { display: none; } }
</style>
