<script setup lang="ts">
import { Users, Building2, Activity, Database, FolderOpen, HardDrive, Archive, CircleAlert, CircleCheck, ExternalLink, Gauge } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'

/** État de l'instance : espace utilisé, comptes, tâches de fond, sauvegarde, version (GET /api/admin/status). */
interface Job { name: string; runs: number; errors: number; last_run: string | null; last_ok: string | null; last_error: string | null; last_error_at: string | null }
interface Status {
  version: string
  update: { checked: boolean; latest?: string; available?: boolean; url?: string; error?: boolean }
  counts: { accounts: number; accounts_suspended: number; accounts_active_30d: number; accounts_pending_deletion: number; workspaces: number; personal_spaces: number; sessions: number }
  storage: { db_bytes: number; upload_bytes: number; upload_files: number; disk: { total: number; available: number } | null; spaces: { id: string; name: string; bytes: number }[]; personal_bytes: number }
  jobs: Job[]
  backup: { last_at: string | null; schedule: 'off' | 'daily' | 'weekly' }
  metrics_enabled: boolean
}
const { t, relativeTime, formatDateTime } = useI18n()
const { data, isLoading } = useQuery({ queryKey: ['admin', 'status'], queryFn: () => api<Status>('/api/admin/status'), refetchInterval: 30_000 })

function bytes(n: number) {
  if (!n) return '0 B'
  const u = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(u.length - 1, Math.floor(Math.log(n) / Math.log(1024)))
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${u[i]}`
}
const pct = (part: number, whole: number) => (whole ? Math.max(0.5, (part / whole) * 100) : 0)
const diskUsed = computed(() => (data.value?.storage.disk ? data.value.storage.disk.total - data.value.storage.disk.available : 0))
const maxSpace = computed(() => Math.max(1, data.value?.storage.personal_bytes ?? 0, ...(data.value?.storage.spaces ?? []).map((s) => s.bytes)))
const jobName = (n: string) => {
  const key = `adminStatus.jobNames.${n}`
  const label = t(key)
  return label === key ? n : label
}
/** Une sauvegarde de plus de huit jours (ou jamais faite) alors qu'elles sont programmées. */
const backupStale = computed(() => {
  const b = data.value?.backup
  if (!b || b.schedule === 'off') return false
  return !b.last_at || Date.now() - new Date(b.last_at).getTime() > 8 * 86400_000
})
</script>

<template>
  <div v-if="isLoading" class="skeleton" style="height: 240px" />
  <template v-else-if="data">
    <SettingsSection :title="t('adminStatus.title')" :description="t('adminStatus.description')">
      <div class="st-tiles">
        <div class="st-tile">
          <Gauge :size="16" class="st-ic" />
          <div class="st-val">{{ data.version }}</div>
          <div class="st-lab">{{ t('adminStatus.version') }}</div>
          <div v-if="data.update.checked && data.update.available" class="st-sub warn">
            {{ t('adminStatus.updateAvailable', { version: data.update.latest }) }}
            <a v-if="data.update.url" :href="data.update.url" target="_blank" rel="noopener"><ExternalLink :size="12" /></a>
          </div>
          <div v-else-if="data.update.checked && data.update.error" class="st-sub">{{ t('adminStatus.updateError') }}</div>
          <div v-else-if="data.update.checked" class="st-sub ok">{{ t('adminStatus.upToDate') }}</div>
          <div v-else class="st-sub" :title="t('adminStatus.updateOff')">{{ t('adminStatus.updateOff') }}</div>
        </div>
        <div class="st-tile">
          <Users :size="16" class="st-ic" />
          <div class="st-val">{{ data.counts.accounts }}</div>
          <div class="st-lab">{{ t('adminStatus.accounts') }}</div>
          <div class="st-sub">{{ t('adminStatus.active30', { count: data.counts.accounts_active_30d }) }}</div>
          <div v-if="data.counts.accounts_suspended" class="st-sub">{{ t('adminStatus.accountsOf', { count: data.counts.accounts_suspended }) }}</div>
          <div v-if="data.counts.accounts_pending_deletion" class="st-sub warn">{{ t('adminStatus.pendingDeletion', { count: data.counts.accounts_pending_deletion }) }}</div>
        </div>
        <div class="st-tile">
          <Building2 :size="16" class="st-ic" />
          <div class="st-val">{{ data.counts.workspaces }}</div>
          <div class="st-lab">{{ t('adminStatus.workspaces') }}</div>
          <div class="st-sub">{{ t('adminStatus.personalSpaces', { count: data.counts.personal_spaces }) }}</div>
        </div>
        <div class="st-tile">
          <Activity :size="16" class="st-ic" />
          <div class="st-val">{{ data.counts.sessions }}</div>
          <div class="st-lab">{{ t('adminStatus.sessions') }}</div>
        </div>
        <div class="st-tile">
          <Archive :size="16" class="st-ic" />
          <div class="st-val st-val-sm">{{ data.backup.last_at ? relativeTime(data.backup.last_at) : '—' }}</div>
          <div class="st-lab">{{ t('adminStatus.backup') }}</div>
          <div v-if="data.backup.schedule === 'off'" class="st-sub">{{ t('adminStatus.backupOff') }}</div>
          <div v-else-if="!data.backup.last_at" class="st-sub warn">{{ t('adminStatus.backupNever') }}</div>
          <div v-else-if="backupStale" class="st-sub warn">{{ t('adminStatus.backupWarn', { days: 8 }) }}</div>
          <div v-else class="st-sub">{{ formatDateTime(data.backup.last_at) }}</div>
        </div>
      </div>
    </SettingsSection>

    <SettingsSection :title="t('adminStatus.storage')">
      <div class="st-tiles">
        <div class="st-tile">
          <Database :size="16" class="st-ic" />
          <div class="st-val">{{ bytes(data.storage.db_bytes) }}</div>
          <div class="st-lab">{{ t('adminStatus.database') }}</div>
        </div>
        <div class="st-tile">
          <FolderOpen :size="16" class="st-ic" />
          <div class="st-val">{{ bytes(data.storage.upload_bytes) }}</div>
          <div class="st-lab">{{ t('adminStatus.files') }}</div>
          <div class="st-sub">{{ t('adminStatus.filesCount', { count: data.storage.upload_files }) }}</div>
        </div>
        <div v-if="data.storage.disk" class="st-tile">
          <HardDrive :size="16" class="st-ic" />
          <div class="st-val">{{ bytes(data.storage.disk.available) }}</div>
          <div class="st-lab">{{ t('adminStatus.diskFree') }}</div>
          <div class="st-meter" role="meter" :aria-valuenow="diskUsed" :aria-valuemax="data.storage.disk.total"><span :style="{ width: pct(diskUsed, data.storage.disk.total) + '%' }" /></div>
          <div class="st-sub">{{ t('adminStatus.diskOf', { used: bytes(diskUsed), total: bytes(data.storage.disk.total) }) }}</div>
        </div>
      </div>
      <h4 class="st-h">{{ t('adminStatus.bySpace') }}</h4>
      <div class="st-bars">
        <div v-for="s in data.storage.spaces" :key="s.id" class="st-bar-row">
          <span class="st-bar-lab truncate" :title="s.name">{{ s.name }}</span>
          <span class="st-bar"><span :style="{ width: pct(s.bytes, maxSpace) + '%' }" /></span>
          <span class="st-bar-val">{{ bytes(s.bytes) }}</span>
        </div>
        <div class="st-bar-row">
          <span class="st-bar-lab truncate faint">{{ t('adminStatus.personalFiles') }}</span>
          <span class="st-bar"><span class="alt" :style="{ width: pct(data.storage.personal_bytes, maxSpace) + '%' }" /></span>
          <span class="st-bar-val">{{ bytes(data.storage.personal_bytes) }}</span>
        </div>
      </div>
    </SettingsSection>

    <SettingsSection :title="t('adminStatus.jobs')" :description="t('adminStatus.jobsHint')">
      <p v-if="!data.jobs.length" class="st-empty">{{ t('adminStatus.noJobs') }}</p>
      <div v-for="j in data.jobs" :key="j.name" class="st-job">
        <component :is="j.last_error && (!j.last_ok || (j.last_error_at ?? '') > j.last_ok) ? CircleAlert : CircleCheck" :size="15" :class="j.last_error && (!j.last_ok || (j.last_error_at ?? '') > j.last_ok) ? 'bad' : 'ok'" />
        <div class="st-job-body">
          <div class="st-job-name">{{ jobName(j.name) }}</div>
          <div class="st-sub">{{ t('adminStatus.lastRun') }} : {{ j.last_run ? relativeTime(j.last_run) : t('adminStatus.noRunYet') }} · {{ t('adminStatus.processed') }} : {{ j.runs }} · {{ t('adminStatus.failed') }} : {{ j.errors }}</div>
          <div v-if="j.last_error" class="st-sub bad">{{ t('adminStatus.lastError') }} ({{ j.last_error_at ? relativeTime(j.last_error_at) : '' }}) : {{ j.last_error }}</div>
        </div>
      </div>
    </SettingsSection>

    <SettingsSection :title="t('adminStatus.metrics')">
      <p class="st-empty">{{ data.metrics_enabled ? t('adminStatus.metricsOn') : t('adminStatus.metricsOff') }}</p>
    </SettingsSection>
  </template>
</template>

<style scoped>
.st-tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 10px; padding: 12px 16px; }
.st-tile { padding: 12px; border-radius: var(--r-md); box-shadow: inset 0 0 0 1px var(--border); display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.st-ic { color: var(--text-3); margin-bottom: 4px; }
.st-val { font-size: 20px; font-weight: 600; font-variant-numeric: tabular-nums; }
.st-val-sm { font-size: 16px; padding: 2px 0; }
.st-lab { font-size: var(--fs-sm); color: var(--text-2); }
.st-sub { font-size: var(--fs-sm); color: var(--text-3); display: flex; gap: 6px; align-items: center; overflow-wrap: anywhere; }
.st-sub.warn, .warn { color: var(--orange); }
.st-sub.ok, .ok { color: var(--green, var(--accent)); }
.st-sub.bad, .bad { color: var(--red, #d33); }
.st-sub a { color: inherit; display: inline-flex; }
.st-meter { height: 6px; border-radius: 3px; background: var(--bg-hover); overflow: hidden; margin: 6px 0 2px; }
.st-meter span { display: block; height: 100%; background: var(--accent); }
.st-h { margin: 8px 16px 6px; font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; }
.st-bars { padding: 0 16px 12px; }
.st-bar-row { display: grid; grid-template-columns: 180px 1fr 80px; align-items: center; gap: 10px; height: 26px; }
.st-bar-lab { font-size: var(--fs-sm); color: var(--text-2); }
.st-bar { height: 8px; border-radius: 4px; background: var(--bg-hover); overflow: hidden; }
.st-bar span { display: block; height: 100%; border-radius: 4px; background: var(--accent); }
.st-bar span.alt { background: var(--text-3); opacity: 0.6; }
.st-bar-val { font-size: var(--fs-sm); color: var(--text-2); text-align: right; font-variant-numeric: tabular-nums; }
.st-empty { margin: 0; padding: 12px 16px; font-size: var(--fs-sm); color: var(--text-3); }
.st-job { display: flex; gap: 10px; padding: 10px 16px; align-items: flex-start; }
.st-job > svg { margin-top: 2px; flex: none; }
.st-job-body { min-width: 0; flex: 1; }
.st-job-name { font-size: var(--fs-md); }
@media (max-width: 639px) { .st-bar-row { grid-template-columns: 110px 1fr 64px; } }
</style>
