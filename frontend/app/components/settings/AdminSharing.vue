<script setup lang="ts">
import { Trash2, Plus, X } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

/** Instance : qui peut être trouvé pour un partage, et groupes de comptes utilisables comme cibles de partage. */
const { t } = useI18n()
const qc = useQueryClient()
interface Group { id: string; name: string; member_count: number }
interface Member { id: string; name: string; email: string }
const groups = useQuery({ queryKey: ['admin-share-groups'], queryFn: () => api<Group[]>('/api/admin/share/groups') })
const dir = useQuery({ queryKey: ['admin-share-directory'], queryFn: () => api<{ scope: string }>('/api/admin/share/directory') })
const scope = computed(() => dir.data.value?.scope ?? 'all')
async function setScope(v: string) {
  try {
    await api('/api/admin/share/directory', { method: 'PUT', body: { scope: v } })
    await dir.refetch()
    toast.success(t('adminSharing.saved'))
  } catch (e) { toast.error(errorMessage(e)) }
}
const scopes = computed(() => [{ value: 'all', label: t('adminSharing.directoryAll') }, { value: 'shared', label: t('adminSharing.directoryShared') }, { value: 'email', label: t('adminSharing.directoryEmail') }])

const newName = ref('')
async function create() {
  const name = newName.value.trim()
  if (!name) return
  try {
    await api('/api/admin/share/groups', { method: 'POST', body: { name } })
    newName.value = ''
    await groups.refetch()
  } catch (e) { toast.error(errorMessage(e)) }
}
async function remove(g: Group) {
  if (!(await confirmDialog({ title: t('adminSharing.deleteTitle', { name: g.name }), message: t('adminSharing.deleteBody'), confirmLabel: t('adminSharing.delete'), danger: true }))) return
  await api(`/api/admin/share/groups/${g.id}`, { method: 'DELETE' })
  await groups.refetch()
}

const current = ref<string | null>(null)
const members = ref<Member[]>([])
async function show(g: Group) {
  current.value = current.value === g.id ? null : g.id
  if (current.value) members.value = await api<Member[]>(`/api/admin/share/groups/${g.id}/members`)
}
const q = ref('')
const hits = ref<Member[]>([])
async function search() {
  hits.value = q.value.trim().length >= 2 ? await api<Member[]>(`/api/users/search?q=${encodeURIComponent(q.value.trim())}`, { silent: true }).catch(() => []) : []
}
watch(q, search)
async function addMember(u: Member) {
  if (!current.value) return
  await api(`/api/admin/share/groups/${current.value}/members`, { method: 'POST', body: { user_id: u.id } })
  q.value = ''
  members.value = await api<Member[]>(`/api/admin/share/groups/${current.value}/members`)
  qc.invalidateQueries({ queryKey: ['admin-share-groups'] })
}
async function removeMember(u: Member) {
  if (!current.value) return
  await api(`/api/admin/share/groups/${current.value}/members/${u.id}`, { method: 'DELETE' })
  members.value = members.value.filter((m) => m.id !== u.id)
  qc.invalidateQueries({ queryKey: ['admin-share-groups'] })
}
</script>

<template>
  <SettingsSection :title="t('adminSharing.directory')" :description="t('adminSharing.directoryHint')">
    <SettingsRow v-for="s in scopes" :key="s.value" :label="s.label">
      <input type="radio" name="dir-scope" :value="s.value" :checked="scope === s.value" :aria-label="s.label" @change="setScope(s.value)" />
    </SettingsRow>
  </SettingsSection>
  <SettingsSection :title="t('adminSharing.groups')" :description="t('adminSharing.groupsHint')">
    <SettingsRow :label="t('adminSharing.groupName')">
      <UiInput v-model="newName" size="sm" :aria-label="t('adminSharing.groupName')" @keydown.enter.prevent="create" />
      <UiButton size="sm" variant="primary" :icon="Plus" :disabled="!newName.trim()" @click="create">{{ t('adminSharing.create') }}</UiButton>
    </SettingsRow>
    <p v-if="!groups.data.value?.length" class="empty">{{ t('adminSharing.noGroups') }}</p>
    <template v-for="g in groups.data.value" :key="g.id">
      <SettingsRow :label="g.name" :hint="String(g.member_count)">
        <UiButton size="sm" @click="show(g)">{{ current === g.id ? '–' : '+' }}</UiButton>
        <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('adminSharing.delete')" @click="remove(g)" />
      </SettingsRow>
      <div v-if="current === g.id" class="members">
        <p v-if="!members.length" class="empty">{{ t('adminSharing.noMembers') }}</p>
        <div v-for="m in members" :key="m.id" class="m"><span>{{ m.name }} <small>{{ m.email }}</small></span>
          <UiButton size="xs" variant="ghost" :icon="X" :aria-label="t('adminSharing.removeMember', { name: m.name })" @click="removeMember(m)" /></div>
        <UiInput v-model="q" size="sm" :placeholder="t('adminSharing.addMember')" :aria-label="t('adminSharing.addMember')" />
        <button v-for="u in hits" :key="u.id" class="hit" @click="addMember(u)">{{ u.name }} <small>{{ u.email }}</small></button>
      </div>
    </template>
  </SettingsSection>
</template>

<style scoped>
.empty { margin: 0; padding: 14px 16px; color: var(--text-3); }
.members { padding: 8px 16px 14px; display: flex; flex-direction: column; gap: 6px; border-top: 1px solid var(--border); }
.m { display: flex; align-items: center; justify-content: space-between; }
small { color: var(--text-3); }
.hit { text-align: left; padding: 6px 8px; border-radius: var(--r-sm); }
.hit:hover { background: var(--bg-hover); }
</style>
