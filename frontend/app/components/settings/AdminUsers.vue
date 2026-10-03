<script setup lang="ts">
import { Search, MoreHorizontal, ShieldCheck, ShieldOff, Ban, CheckCircle2, Trash2 } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api } from '~/composables/api'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

interface AdminUser { id: string; name: string; email: string; avatar_url?: string | null; is_super_admin: boolean; is_suspended: boolean; created_at: string; workspace_count: number }
const { t, formatDate } = useI18n()
const { user: me } = useAuth()
const qc = useQueryClient()
const { data: users, isLoading } = useQuery({ queryKey: ['admin', 'users'], queryFn: () => api<AdminUser[]>('/api/admin/users') })
const q = ref('')
const filter = ref<'all' | 'admins' | 'suspended'>('all')
const list = computed(() => (users.value ?? []).filter((u) => {
  if (filter.value === 'admins' && !u.is_super_admin) return false
  if (filter.value === 'suspended' && !u.is_suspended) return false
  return `${u.name} ${u.email}`.toLowerCase().includes(q.value.toLowerCase())
}))
const refresh = () => qc.invalidateQueries({ queryKey: ['admin', 'users'] })

async function setAdmin(u: AdminUser, on: boolean) {
  if (!(await confirmDialog({ title: on ? t('settings.admin.promoteTitle', { name: u.name }) : t('settings.admin.demoteTitle', { name: u.name }), message: on ? t('settings.admin.promoteMessage') : undefined, confirmLabel: t('common.confirm'), danger: on }))) return
  await api(`/api/admin/users/${u.id}/role`, { method: 'PUT', body: { is_super_admin: on } })
  refresh()
}
async function setSuspended(u: AdminUser, on: boolean) {
  if (on && !(await confirmDialog({ title: t('settings.admin.suspendTitle', { name: u.name }), message: t('settings.admin.suspendMessage'), danger: true, confirmLabel: t('settings.admin.suspend') }))) return
  await api(`/api/admin/users/${u.id}/suspend`, { method: 'PUT', body: { is_suspended: on } })
  toast.success(on ? t('settings.admin.suspendedToast') : t('settings.admin.reactivatedToast'))
  refresh()
}
async function remove(u: AdminUser) {
  if (!(await confirmDialog({ title: t('settings.admin.deleteUserTitle', { name: u.name }), message: t('settings.admin.deleteUserMessage'), danger: true, confirmLabel: t('common.delete'), typeToConfirm: u.email }))) return
  await api(`/api/admin/users/${u.id}`, { method: 'DELETE' })
  refresh()
}
const menus = ref<Record<string, boolean>>({})
function items(u: AdminUser): MenuItem[] {
  return [
    u.is_super_admin
      ? { label: t('settings.admin.demote'), icon: ShieldOff, onSelect: () => setAdmin(u, false) }
      : { label: t('settings.admin.promote'), icon: ShieldCheck, onSelect: () => setAdmin(u, true) },
    u.is_suspended
      ? { label: t('settings.admin.reactivate'), icon: CheckCircle2, onSelect: () => setSuspended(u, false) }
      : { label: t('settings.admin.suspend'), icon: Ban, onSelect: () => setSuspended(u, true) },
    { divider: true },
    { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => remove(u) },
  ]
}
</script>

<template>
  <SettingsSection :description="t('settings.admin.usersHint', { count: users?.length ?? 0 })">
    <template #actions>
      <UiSegmented v-model="filter" :options="[{ value: 'all', label: t('inbox.all') }, { value: 'admins', label: t('settings.admin.admins') }, { value: 'suspended', label: t('settings.admin.suspendedTab') }]" />
      <UiInput v-model="q" :icon="Search" size="sm" :placeholder="t('common.search')" style="width: 180px" />
    </template>
    <div v-if="isLoading" class="skeleton" style="height: 120px; margin: 12px" />
    <div v-for="u in list" :key="u.id" class="u-row" :class="{ suspended: u.is_suspended }">
      <UiAvatar :name="u.name" :src="u.avatar_url" :size="30" />
      <div class="u-body">
        <div class="u-name truncate">
          {{ u.name }}
          <span v-if="u.is_super_admin" class="badge accent">{{ t('settings.admin.adminBadge') }}</span>
          <span v-if="u.is_suspended" class="badge red">{{ t('settings.admin.suspendedBadge') }}</span>
        </div>
        <div class="u-mail truncate">{{ u.email }}</div>
      </div>
      <span class="u-meta">{{ t('settings.admin.wsCount', { count: u.workspace_count }) }}</span>
      <span class="u-meta">{{ formatDate(u.created_at) }}</span>
      <UiMenu v-if="u.id !== me?.id" v-model:open="menus[u.id]" :items="items(u)" placement="bottom-end">
        <UiButton size="sm" variant="ghost" :icon="MoreHorizontal" />
      </UiMenu>
      <span v-else class="u-you faint">{{ t('common.you') }}</span>
    </div>
    <div v-if="!isLoading && !list.length" class="empty">{{ t('common.noResults') }}</div>
  </SettingsSection>
</template>

<style scoped>
.u-row { display: flex; align-items: center; gap: 12px; padding: 10px 16px; }
.u-row + .u-row { border-top: 1px solid var(--border); }
.u-row.suspended { opacity: 0.6; }
.u-body { flex: 1; min-width: 0; }
.u-name { font-weight: 500; display: flex; align-items: center; gap: 6px; }
.u-mail { font-size: var(--fs-sm); color: var(--text-3); }
.u-meta { font-size: var(--fs-sm); color: var(--text-3); white-space: nowrap; width: 100px; text-align: right; }
.u-you { width: 30px; font-size: var(--fs-sm); }
.badge { font-size: 10.5px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-full); }
.badge.accent { background: var(--accent-soft); color: var(--accent-text); }
.badge.red { background: var(--red-soft); color: var(--red); }
.empty { padding: 18px 16px; color: var(--text-3); }
@media (max-width: 767px) { .u-meta { display: none; } }
</style>
