<script setup lang="ts">
import { Search, Trash2 } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'

interface AdminWs { id: string; name: string; members_count: number; boards_count: number; pages_count: number; owner_name?: string | null; created_at: string }
const { t, formatDate } = useI18n()
const qc = useQueryClient()
const { data, isLoading } = useQuery({ queryKey: ['admin', 'workspaces'], queryFn: () => api<AdminWs[]>('/api/admin/workspaces') })
const q = ref('')
const list = computed(() => (data.value ?? []).filter((w) => `${w.name} ${w.owner_name ?? ''}`.toLowerCase().includes(q.value.toLowerCase())))

async function remove(w: AdminWs) {
  if (!(await confirmDialog({ title: t('settings.ws.deleteTitle', { name: w.name }), message: t('settings.admin.deleteWsMessage', { boards: w.boards_count, pages: w.pages_count }), danger: true, confirmLabel: t('common.delete'), typeToConfirm: w.name }))) return
  await api(`/api/admin/workspaces/${w.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['admin', 'workspaces'] })
  qc.invalidateQueries({ queryKey: ['workspaces'] })
}
</script>

<template>
  <SettingsSection :description="t('settings.admin.workspacesHint', { count: data?.length ?? 0 })">
    <template #actions><UiInput v-model="q" :icon="Search" size="sm" :placeholder="t('common.search')" style="width: 200px" /></template>
    <div v-if="isLoading" class="skeleton" style="height: 100px; margin: 12px" />
    <table v-else class="tbl">
      <thead>
        <tr><th>{{ t('workspace.name') }}</th><th>{{ t('roles.owner') }}</th><th class="num">{{ t('settings.members.title') }}</th><th class="num">{{ t('nav.boards') }}</th><th class="num">{{ t('nav.docs') }}</th><th>{{ t('settings.admin.created') }}</th><th /></tr>
      </thead>
      <tbody>
        <tr v-for="w in list" :key="w.id">
          <td class="name">{{ w.name }}</td>
          <td class="muted">{{ w.owner_name ?? '—' }}</td>
          <td class="num tabular">{{ w.members_count }}</td>
          <td class="num tabular">{{ w.boards_count }}</td>
          <td class="num tabular">{{ w.pages_count }}</td>
          <td class="muted">{{ formatDate(w.created_at) }}</td>
          <td class="act"><UiButton size="sm" variant="ghost" :icon="Trash2" @click="remove(w)" /></td>
        </tr>
      </tbody>
    </table>
  </SettingsSection>
</template>

<style scoped>
.tbl { width: 100%; border-collapse: collapse; font-size: var(--fs-md); }
.tbl th { text-align: left; font-weight: 500; font-size: var(--fs-sm); color: var(--text-3); padding: 10px 16px; border-bottom: 1px solid var(--border); }
.tbl td { padding: 10px 16px; border-bottom: 1px solid var(--border); }
.tbl tr:last-child td { border-bottom: none; }
.tbl .num { text-align: right; }
.tbl .name { font-weight: 500; }
.tbl .act { width: 40px; padding: 4px 8px; }
@media (max-width: 767px) { .tbl th:nth-child(n + 4):not(:last-child), .tbl td:nth-child(n + 4):not(:last-child) { display: none; } }
</style>
