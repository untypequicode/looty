<script setup lang="ts">
import { Users, Building2, Kanban, CircleDot, FileText, Paperclip, Activity, UserPlus } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { motion } from '~/composables/useMotion'

interface Stats {
  total_users: number; active_users_7d: number; new_users_30d: number; suspended_users: number
  total_workspaces: number; total_boards: number; total_tasks: number; open_tasks: number
  total_pages: number; total_attachments: number; total_attachments_size: number
}
const { t } = useI18n()
const { data } = useQuery({ queryKey: ['admin', 'stats'], queryFn: () => api<{ stats: Stats; activity: { day: string; tasks: number; pages: number }[] }>('/api/admin/stats') })

function bytes(n: number) {
  if (!n) return '0 B'
  const u = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(u.length - 1, Math.floor(Math.log(n) / Math.log(1024)))
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${u[i]}`
}
const tiles = computed(() => {
  const s = data.value?.stats
  if (!s) return []
  return [
    { key: 'users', icon: Users, value: s.total_users, label: t('settings.admin.users'), sub: t('settings.admin.suspended', { count: s.suspended_users }) },
    { key: 'active', icon: Activity, value: s.active_users_7d, label: t('settings.admin.active7d'), sub: t('settings.admin.ofUsers', { count: s.total_users }) },
    { key: 'new', icon: UserPlus, value: s.new_users_30d, label: t('settings.admin.new30d') },
    { key: 'ws', icon: Building2, value: s.total_workspaces, label: t('settings.admin.workspaces') },
    { key: 'boards', icon: Kanban, value: s.total_boards, label: t('settings.admin.boards') },
    { key: 'tasks', icon: CircleDot, value: s.total_tasks, label: t('settings.admin.tasks'), sub: t('settings.admin.open', { count: s.open_tasks }) },
    { key: 'pages', icon: FileText, value: s.total_pages, label: t('settings.admin.pages') },
    { key: 'files', icon: Paperclip, value: s.total_attachments, label: t('settings.admin.attachments'), sub: bytes(s.total_attachments_size) },
  ]
})
const els = ref<HTMLElement[]>([])
const grid = ref<HTMLElement | null>(null)
watch(tiles, async (v) => {
  if (!v.length) return
  await nextTick()
  if (grid.value) motion.stagger(grid.value.children, { y: 8, each: 0.03 })
  v.forEach((tile, i) => motion.countUp(els.value[i], tile.value))
}, { immediate: true })
</script>

<template>
  <SettingsSection :description="t('settings.admin.overviewHint')">
    <div ref="grid" class="tiles">
      <div v-for="(tl, i) in tiles" :key="tl.key" class="tile">
        <div class="tile-top"><component :is="tl.icon" :size="14" /> {{ tl.label }}</div>
        <div :ref="(el) => { if (el) els[i] = el as HTMLElement }" class="tile-value tabular">{{ tl.value }}</div>
        <div v-if="tl.sub" class="tile-sub">{{ tl.sub }}</div>
      </div>
    </div>
  </SettingsSection>
  <SettingsSection :title="t('settings.admin.activity')" :description="t('settings.admin.activityHint')">
    <SettingsActivityChart v-if="data" :data="data.activity" />
    <div v-else class="skeleton" style="height: 220px; margin: 16px" />
  </SettingsSection>
</template>

<style scoped>
.tiles { display: grid; grid-template-columns: repeat(4, 1fr); }
.tile { padding: 14px 16px; border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); }
.tile:nth-child(4n) { border-right: none; }
.tile:nth-last-child(-n + 4) { border-bottom: none; }
.tile-top { display: flex; align-items: center; gap: 6px; color: var(--text-3); font-size: var(--fs-sm); }
.tile-value { font-size: 24px; font-weight: 600; letter-spacing: -0.02em; margin-top: 8px; }
.tile-sub { font-size: var(--fs-xs); color: var(--text-3); margin-top: 2px; }
@media (max-width: 767px) {
  .tiles { grid-template-columns: repeat(2, 1fr); }
  .tile { border-right: 1px solid var(--border) !important; border-bottom: 1px solid var(--border) !important; }
  .tile:nth-child(2n) { border-right: none !important; }
}
</style>
