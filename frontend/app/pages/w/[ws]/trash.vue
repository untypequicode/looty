<script setup lang="ts">
import { Trash2, RotateCcw, FileText, Kanban, X } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { api } from '~/composables/api'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { useAuth } from '~/composables/useAuth'
import { useTrash, useTrashInvalidate, restoreFromTrash, type TrashedTask, type TrashedPage } from '~/composables/useTrash'

definePageMeta({ layout: 'workspace' })

const { wsId, canWrite, isAdmin } = useWorkspace()
const { t, relativeTime, formatDate } = useI18n()
const { data, isLoading } = useTrash(wsId)
const invalidate = useTrashInvalidate()
const tab = ref<'tasks' | 'pages'>('tasks')
watch(data, (d) => { if (d && !d.tasks.length && d.pages.length) tab.value = 'pages' }, { once: true })


const daysLeft = (iso: string) => Math.max(0, Math.ceil((+new Date(iso) - Date.now()) / 86400000))
const { user } = useAuth()
const canPurge = (deletedBy?: string | null) => isAdmin.value || (!!deletedBy && deletedBy === user.value?.id)

async function restore(kind: 'tasks' | 'pages', item: TrashedTask | TrashedPage) {
  await restoreFromTrash(kind, item.id)
  toast.success(t('trash.restored'))
  invalidate(wsId.value)
}
async function purge(kind: 'tasks' | 'pages', item: TrashedTask | TrashedPage) {
  if (!(await confirmDialog({ title: t('trash.deleteTitle', { title: item.title || t('docs.untitled') }), message: t('trash.deleteMessage'), danger: true, confirmLabel: t('trash.deleteForever') }))) return
  await api(`/api/trash/${kind}/${item.id}`, { method: 'DELETE' })
  invalidate(wsId.value)
}
async function emptyTrash() {
  if (!(await confirmDialog({ title: t('trash.emptyTitle'), message: t('trash.deleteMessage'), danger: true, confirmLabel: t('trash.empty') }))) return
  await api(`/api/workspaces/${wsId.value}/trash`, { method: 'DELETE' })
  invalidate(wsId.value)
}
const count = computed(() => (data.value?.tasks.length ?? 0) + (data.value?.pages.length ?? 0))
useHead({ title: () => t('trash.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('trash.title'), icon: Trash2 }]">
      <UiSegmented v-model="tab" :options="[{ value: 'tasks', label: `${t('trash.tasks')}${data?.tasks.length ? ` · ${data.tasks.length}` : ''}` }, { value: 'pages', label: `${t('trash.pages')}${data?.pages.length ? ` · ${data.pages.length}` : ''}` }]" />
      <UiButton v-if="isAdmin && count" size="sm" variant="ghost" :icon="X" @click="emptyTrash">{{ t('trash.empty') }}</UiButton>
    </AppPageHeader>
    <div class="tr">
      <div class="tr-inner">
        <p v-if="data" class="tr-hint">
          {{ data.retention_days ? t('trash.hint', { days: data.retention_days }) : t('trash.disabled') }}
          <NuxtLink v-if="isAdmin" :to="`/w/${wsId}/settings/workspace`">{{ t('trash.change') }}</NuxtLink>
        </p>
        <div v-if="isLoading"><div v-for="i in 5" :key="i" class="skeleton" style="height: 48px; margin: 6px 0" /></div>
        <template v-else-if="tab === 'tasks'">
          <UiEmpty v-if="!data?.tasks.length" :icon="Trash2" :title="t('trash.emptyTasks')" />
          <div v-for="tk in data?.tasks" :key="tk.id" class="tr-row">
            <Kanban :size="15" class="tr-icon" />
            <div class="tr-main">
              <div class="tr-title truncate">{{ tk.title }}<span v-if="tk.subtasks" class="tr-sub">+{{ t('trash.subtasks', { count: tk.subtasks }) }}</span></div>
              <div class="tr-meta truncate">{{ tk.board_name }} · {{ t('trash.deletedBy', { name: tk.deleted_by_name || t('trash.someone'), when: relativeTime(tk.deleted_at) }) }}</div>
            </div>
            <UiTooltip :text="formatDate(tk.expires_at)"><span class="tr-left">{{ t('trash.daysLeft', { count: daysLeft(tk.expires_at) }) }}</span></UiTooltip>
            <UiButton v-if="canWrite" size="sm" :icon="RotateCcw" @click="restore('tasks', tk)">{{ t('trash.restore') }}</UiButton>
            <UiTooltip v-if="canPurge(tk.deleted_by)" :text="t('trash.deleteForever')"><UiButton size="sm" variant="ghost" :icon="Trash2" @click="purge('tasks', tk)" /></UiTooltip>
          </div>
        </template>
        <template v-else>
          <UiEmpty v-if="!data?.pages.length" :icon="Trash2" :title="t('trash.emptyPages')" :description="isAdmin ? '' : t('trash.pagesOwnOnly')" />
          <div v-for="p in data?.pages" :key="p.id" class="tr-row">
            <span v-if="p.icon" class="tr-icon tr-emoji">{{ p.icon }}</span><FileText v-else :size="15" class="tr-icon" />
            <div class="tr-main">
              <div class="tr-title truncate">{{ p.title || t('docs.untitled') }}<span v-if="p.subpages" class="tr-sub">+{{ t('trash.subpages', { count: p.subpages }) }}</span></div>
              <div class="tr-meta truncate"><template v-if="p.parent_title">{{ p.parent_title }} · </template>{{ t('trash.deletedBy', { name: p.deleted_by_name || t('trash.someone'), when: relativeTime(p.deleted_at) }) }}</div>
            </div>
            <UiTooltip :text="formatDate(p.expires_at)"><span class="tr-left">{{ t('trash.daysLeft', { count: daysLeft(p.expires_at) }) }}</span></UiTooltip>
            <UiButton v-if="canWrite" size="sm" :icon="RotateCcw" @click="restore('pages', p)">{{ t('trash.restore') }}</UiButton>
            <UiTooltip :text="t('trash.deleteForever')"><UiButton size="sm" variant="ghost" :icon="Trash2" @click="purge('pages', p)" /></UiTooltip>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tr { flex: 1; overflow-y: auto; padding: 8px 16px 48px; }
.tr-inner { max-width: 860px; margin: 0 auto; }
.tr-hint { color: var(--text-3); font-size: var(--fs-sm); margin: 8px 10px 12px; }
.tr-hint a { color: var(--accent); margin-left: 4px; }
.tr-row { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: var(--r-md); }
.tr-row:hover { background: var(--bg-hover); }
.tr-icon { color: var(--text-3); flex: none; width: 18px; text-align: center; }
.tr-emoji { font-size: 15px; }
.tr-main { flex: 1; min-width: 0; }
.tr-title { font-weight: 500; }
.tr-sub { margin-left: 8px; font-size: var(--fs-sm); color: var(--text-3); font-weight: 400; }
.tr-meta { font-size: var(--fs-sm); color: var(--text-3); }
.tr-left { font-size: var(--fs-sm); color: var(--text-3); flex: none; }
@media (max-width: 639px) { .tr-left { display: none; } }
</style>
