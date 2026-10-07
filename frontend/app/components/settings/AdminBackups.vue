<script setup lang="ts">
import { Archive, Download, RotateCcw, Trash2, Upload, Plus, ShieldCheck, Clock } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

/**
 * Sauvegardes de toute l'instance (base de données + fichiers envoyés) : création,
 * téléchargement, import, restauration (précédée d'une sauvegarde de sécurité) et
 * sauvegardes programmées avec rétention.
 */
const { t, relativeTime, formatDateTime } = useI18n()
const qc = useQueryClient()
interface Backup { name: string; size: number; created_at: string | null; kind: string }
interface Info { backups: Backup[]; directory: string; schedule: 'off' | 'daily' | 'weekly'; hour: number; keep: number; last_at: string | null; timezone: string }
const { data } = useQuery({ queryKey: ['admin-backups'], queryFn: () => api<Info>('/api/admin/backups') })
const refresh = () => qc.invalidateQueries({ queryKey: ['admin-backups'] })

const size = (b: number) => (b > 1 << 30 ? `${(b / (1 << 30)).toFixed(2)} Go` : b > 1 << 20 ? `${(b / (1 << 20)).toFixed(1)} Mo` : `${Math.max(1, Math.round(b / 1024))} ko`)
const busy = ref<string | null>(null)
async function createNow() {
  busy.value = 'create'
  try { await api('/api/admin/backups', { method: 'POST' }); toast.success(t('backups.created')); refresh() }
  catch (e) { toast.error(errorMessage(e)) } finally { busy.value = null }
}
function download(b: Backup) { window.location.href = `/api/admin/backups/${encodeURIComponent(b.name)}/download` }
async function remove(b: Backup) {
  if (!(await confirmDialog({ title: t('backups.deleteTitle'), message: b.name, danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/admin/backups/${encodeURIComponent(b.name)}`, { method: 'DELETE' })
  refresh()
}
async function restore(b: Backup) {
  const ok = await confirmDialog({
    title: t('backups.restoreTitle'),
    message: t('backups.restoreHint', { date: b.created_at ? formatDateTime(b.created_at) : b.name }),
    danger: true, confirmLabel: t('backups.restore'), typeToConfirm: 'RESTORE',
  })
  if (!ok) return
  busy.value = b.name
  try {
    const r = await api<{ safety_backup: string; report: { rows: number; uploads: number; restart_required: boolean } }>(`/api/admin/backups/${encodeURIComponent(b.name)}/restore`, { method: 'POST' })
    toast.success(t('backups.restored', { rows: r.report.rows, files: r.report.uploads }))
    if (r.report.restart_required) toast(t('backups.restartRequired'))
    refresh()
  } catch (e) { toast.error(errorMessage(e)) } finally { busy.value = null }
}
const fileInput = ref<HTMLInputElement | null>(null)
async function upload(e: Event) {
  const f = (e.target as HTMLInputElement).files?.[0]
  if (!f) return
  busy.value = 'upload'
  try {
    const fd = new FormData()
    fd.append('file', f)
    const r = await fetch('/api/admin/backups/upload', { method: 'POST', body: fd, credentials: 'include' })
    if (!r.ok) throw new Error((await r.json().catch(() => ({}))).error ?? r.statusText)
    toast.success(t('backups.uploaded'))
    refresh()
  } catch (err) { toast.error(t(`backups.errors.${(err as Error).message}`) || String(err)) } finally {
    busy.value = null
    if (fileInput.value) fileInput.value.value = ''
  }
}

// Programmation
const schedule = ref<'off' | 'daily' | 'weekly'>('off')
const hour = ref('3')
const keep = ref('7')
watch(data, (d) => { if (d) { schedule.value = d.schedule; hour.value = String(d.hour); keep.value = String(d.keep) } }, { immediate: true })
async function saveSchedule() {
  await api('/api/admin/backups/settings', { method: 'PUT', body: { schedule: schedule.value, hour: Number(hour.value), keep: Number(keep.value) } })
  toast.success(t('common.saved'))
  refresh()
}
const hours = Array.from({ length: 24 }, (_, h) => ({ value: String(h), label: `${String(h).padStart(2, '0')}:00` }))
</script>

<template>
  <SettingsSection :title="t('backups.title')" :description="t('backups.hint')">
    <template #actions>
      <input ref="fileInput" type="file" accept=".gz,application/gzip" hidden @change="upload" />
      <UiButton size="sm" :icon="Upload" :loading="busy === 'upload'" @click="fileInput?.click()">{{ t('backups.import') }}</UiButton>
      <UiButton size="sm" variant="primary" :icon="Plus" :loading="busy === 'create'" @click="createNow">{{ t('backups.createNow') }}</UiButton>
    </template>
    <div v-for="b in data?.backups ?? []" :key="b.name" class="bk">
      <span class="bk-ic" :class="b.kind"><component :is="b.kind === 'prerestore' ? ShieldCheck : b.kind === 'auto' ? Clock : Archive" :size="15" /></span>
      <div class="bk-body">
        <div class="bk-title">{{ b.created_at ? formatDateTime(b.created_at) : b.name }} <span class="bk-kind">{{ t(`backups.kinds.${b.kind}`) }}</span></div>
        <div class="bk-meta">{{ size(b.size) }} · {{ relativeTime(b.created_at) }}</div>
      </div>
      <UiButton size="sm" variant="ghost" :icon="Download" :aria-label="t('backups.download')" :title="t('backups.download')" @click="download(b)" />
      <UiButton size="sm" variant="ghost" :icon="RotateCcw" :loading="busy === b.name" :disabled="!!busy" @click="restore(b)">{{ t('backups.restore') }}</UiButton>
      <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove(b)" />
    </div>
    <p v-if="data && !data.backups.length" class="bk-empty"><Archive :size="16" /> {{ t('backups.none') }}</p>
    <p v-if="data" class="bk-dir">{{ t('backups.directory', { dir: data.directory }) }}</p>
  </SettingsSection>

  <SettingsSection :title="t('backups.scheduleTitle')" :description="t('backups.scheduleHint', { tz: data?.timezone ?? 'UTC' })">
    <SettingsRow :label="t('backups.frequency')" :hint="data?.last_at ? t('backups.lastAt', { time: relativeTime(data.last_at) }) : ''">
      <UiSegmented v-model="schedule" size="md" :options="[{ value: 'off', label: t('backups.off') }, { value: 'daily', label: t('backups.daily') }, { value: 'weekly', label: t('backups.weekly') }]" />
    </SettingsRow>
    <template v-if="schedule !== 'off'">
      <SettingsRow :label="t('backups.hour')">
        <UiCombobox v-model="hour" :items="hours" :width="140"><button class="sel">{{ hours[Number(hour)]?.label }}</button></UiCombobox>
      </SettingsRow>
      <SettingsRow :label="t('backups.keep')" :hint="t('backups.keepHint')">
        <UiInput v-model="keep" type="number" min="1" max="100" style="width: 90px" />
      </SettingsRow>
    </template>
    <div class="bk-save"><UiButton size="sm" variant="primary" @click="saveSchedule">{{ t('common.save') }}</UiButton></div>
  </SettingsSection>
</template>

<style scoped>
.bk { display: flex; align-items: center; gap: 10px; padding: 10px 16px; }
.bk + .bk { border-top: 1px solid var(--border); }
.bk-ic { width: 30px; height: 30px; border-radius: var(--r-sm); display: grid; place-items: center; background: var(--bg-hover); color: var(--text-2); flex-shrink: 0; }
.bk-ic.prerestore { color: var(--green); }
.bk-body { flex: 1; min-width: 0; }
.bk-title { font-weight: 500; display: flex; align-items: center; gap: 8px; }
.bk-kind { font-size: 10.5px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-3); }
.bk-meta { font-size: var(--fs-sm); color: var(--text-3); }
.bk-empty { display: flex; align-items: center; gap: 8px; padding: 16px; margin: 0; color: var(--text-3); }
.bk-dir { margin: 0; padding: 8px 16px 12px; font-size: var(--fs-xs); color: var(--text-3); font-family: var(--font-mono); }
.bk-save { display: flex; justify-content: flex-end; padding: 10px 16px; }
.sel { height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-sm); min-width: 100px; text-align: left; }
</style>
