<script setup lang="ts">
import { HardDrive, Database, FolderOpen, Trash2, ExternalLink, FileWarning } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

interface Usage { kind: string; label: string; workspace_id?: string | null; target_id?: string | null; board_id?: string | null; in_trash: boolean }
interface FileRow { name: string; size: number; modified?: string | null; file_type: string; usages: Usage[] }
interface Bucket { key: string; bytes: number; count: number }
interface Storage {
  upload_bytes: number; file_count: number; attachments_recorded_bytes: number; db_bytes: number
  disk?: { total: number; available: number } | null
  by_usage: Bucket[]; by_type: Bucket[]; largest: FileRow[]; orphans: FileRow[]; orphan_count: number; orphan_bytes: number
}

const { t, formatDate } = useI18n()
const { data, isLoading, refetch } = useQuery({ queryKey: ['admin', 'storage'], queryFn: () => api<Storage>('/api/admin/storage') })
const view = ref<'largest' | 'orphans'>('largest')

function bytes(n: number) {
  if (!n) return '0 B'
  const u = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(u.length - 1, Math.floor(Math.log(n) / Math.log(1024)))
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${u[i]}`
}
const diskUsed = computed(() => (data.value?.disk ? data.value.disk.total - data.value.disk.available : 0))
const pct = (part: number, whole: number) => (whole ? Math.max(0.5, (part / whole) * 100) : 0)
const maxOf = (b?: Bucket[]) => Math.max(1, ...(b ?? []).map((x) => x.bytes))

function usageLink(u: Usage): string | null {
  if (!u.workspace_id || !u.target_id) return null
  if (u.in_trash) return `/w/${u.workspace_id}/trash`
  if (u.kind === 'page') return `/w/${u.workspace_id}/doc/${u.target_id}`
  return u.board_id ? `/w/${u.workspace_id}/board/${u.board_id}?task=${u.target_id}` : null
}

async function removeFile(f: FileRow) {
  if (!(await confirmDialog({ title: t('storage.deleteTitle', { name: f.name }), message: t('storage.deleteMessage'), danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/admin/storage/files/${encodeURIComponent(f.name)}`, { method: 'DELETE' })
  refetch()
}
async function cleanOrphans() {
  if (!data.value || !(await confirmDialog({ title: t('storage.cleanTitle', { count: data.value.orphan_count }), message: t('storage.cleanMessage', { size: bytes(data.value.orphan_bytes) }), danger: true, confirmLabel: t('storage.clean') }))) return
  const r = await api<{ count: number; bytes: number }>('/api/admin/storage/orphans', { method: 'DELETE' })
  toast.success(t('storage.cleaned', { count: r.count, size: bytes(r.bytes) }))
  refetch()
}
</script>

<template>
  <div v-if="isLoading" class="skeleton" style="height: 240px" />
  <template v-else-if="data">
    <SettingsSection :title="t('storage.title')" :description="t('storage.description')">
      <div class="sg-tiles">
        <div class="sg-tile">
          <FolderOpen :size="16" class="sg-ic" />
          <div class="sg-val">{{ bytes(data.upload_bytes) }}</div>
          <div class="sg-lab">{{ t('storage.uploads') }}</div>
          <div class="sg-sub">{{ t('storage.files', { count: data.file_count }) }}</div>
        </div>
        <div v-if="data.disk" class="sg-tile">
          <HardDrive :size="16" class="sg-ic" />
          <div class="sg-val">{{ bytes(data.disk.available) }}</div>
          <div class="sg-lab">{{ t('storage.free') }}</div>
          <div class="sg-meter" role="meter" :aria-valuenow="diskUsed" :aria-valuemax="data.disk.total" :title="t('storage.diskTip', { used: bytes(diskUsed), total: bytes(data.disk.total), uploads: bytes(data.upload_bytes) })">
            <span class="sg-meter-other" :style="{ width: pct(diskUsed - data.upload_bytes, data.disk.total) + '%' }" />
            <span class="sg-meter-up" :style="{ width: pct(data.upload_bytes, data.disk.total) + '%' }" />
          </div>
          <div class="sg-sub">{{ t('storage.diskOf', { used: bytes(diskUsed), total: bytes(data.disk.total) }) }}</div>
        </div>
        <div class="sg-tile">
          <Database :size="16" class="sg-ic" />
          <div class="sg-val">{{ bytes(data.db_bytes) }}</div>
          <div class="sg-lab">{{ t('storage.database') }}</div>
        </div>
        <div class="sg-tile">
          <FileWarning :size="16" class="sg-ic" />
          <div class="sg-val">{{ bytes(data.orphan_bytes) }}</div>
          <div class="sg-lab">{{ t('storage.orphans') }}</div>
          <div class="sg-sub">
            {{ t('storage.files', { count: data.orphan_count }) }}
            <button v-if="data.orphan_count" class="sg-link" @click="cleanOrphans">{{ t('storage.clean') }}</button>
          </div>
        </div>
      </div>
      <p class="sg-note">{{ t('storage.recorded', { recorded: bytes(data.attachments_recorded_bytes), real: bytes(data.upload_bytes) }) }}</p>
    </SettingsSection>

    <SettingsSection :title="t('storage.breakdown')">
      <div class="sg-cols">
        <div>
          <h4 class="sg-h">{{ t('storage.byUsage') }}</h4>
          <div v-for="b in data.by_usage" :key="b.key" class="sg-bar-row" :title="`${t(`storage.usage.${b.key}`)} · ${bytes(b.bytes)} · ${t('storage.files', { count: b.count })}`">
            <span class="sg-bar-lab">{{ t(`storage.usage.${b.key}`) }}</span>
            <span class="sg-bar"><span :style="{ width: pct(b.bytes, maxOf(data.by_usage)) + '%' }" /></span>
            <span class="sg-bar-val tabular">{{ bytes(b.bytes) }}</span>
          </div>
        </div>
        <div>
          <h4 class="sg-h">{{ t('storage.byType') }}</h4>
          <div v-for="b in data.by_type" :key="b.key" class="sg-bar-row" :title="`${t(`storage.type.${b.key}`)} · ${bytes(b.bytes)} · ${t('storage.files', { count: b.count })}`">
            <span class="sg-bar-lab">{{ t(`storage.type.${b.key}`) }}</span>
            <span class="sg-bar"><span :style="{ width: pct(b.bytes, maxOf(data.by_type)) + '%' }" /></span>
            <span class="sg-bar-val tabular">{{ bytes(b.bytes) }}</span>
          </div>
        </div>
      </div>
    </SettingsSection>

    <SettingsSection :title="t('storage.filesTitle')">
      <template #actions>
        <UiSegmented v-model="view" :options="[{ value: 'largest', label: t('storage.largest') }, { value: 'orphans', label: `${t('storage.orphans')} · ${data.orphan_count}` }]" />
      </template>
      <div class="sg-table">
        <div v-for="f in (view === 'largest' ? data.largest : data.orphans)" :key="f.name" class="sg-row">
          <div class="sg-file">
            <a :href="`/uploads/${f.name}`" target="_blank" rel="noopener" class="sg-name truncate">{{ f.name }} <ExternalLink :size="11" /></a>
            <div class="sg-uses">
              <template v-if="f.usages.length">
                <component :is="usageLink(u) ? 'NuxtLink' : 'span'" v-for="(u, i) in f.usages.slice(0, 3)" :key="i" :to="usageLink(u) ?? undefined" class="sg-use" :class="{ trash: u.in_trash }">
                  {{ t(`storage.usage.${u.in_trash ? 'trash' : u.kind}`) }} · {{ u.label || t('docs.untitled') }}
                </component>
                <span v-if="f.usages.length > 3" class="sg-more">+{{ f.usages.length - 3 }}</span>
              </template>
              <span v-else class="sg-use orphan">{{ t('storage.unused') }}</span>
            </div>
          </div>
          <span class="sg-type">{{ t(`storage.type.${f.file_type}`) }}</span>
          <span class="sg-date tabular">{{ f.modified ? formatDate(f.modified) : '' }}</span>
          <span class="sg-size tabular">{{ bytes(f.size) }}</span>
          <UiButton v-if="!f.usages.length" size="xs" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="removeFile(f)" />
          <span v-else class="sg-spacer" />
        </div>
        <p v-if="!(view === 'largest' ? data.largest : data.orphans).length" class="sg-note">{{ t('storage.none') }}</p>
      </div>
    </SettingsSection>
  </template>
</template>

<style scoped>
.sg-tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 10px; padding: 12px 16px; }
.sg-tile { padding: 12px; border-radius: var(--r-md); box-shadow: inset 0 0 0 1px var(--border); display: flex; flex-direction: column; gap: 2px; }
.sg-ic { color: var(--text-3); margin-bottom: 4px; }
.sg-val { font-size: 20px; font-weight: 600; font-variant-numeric: tabular-nums; }
.sg-lab { font-size: var(--fs-sm); color: var(--text-2); }
.sg-sub { font-size: var(--fs-sm); color: var(--text-3); display: flex; gap: 8px; align-items: center; }
.sg-link { color: var(--accent); font-size: var(--fs-sm); }
.sg-meter { display: flex; gap: 2px; height: 6px; border-radius: 3px; background: var(--bg-hover); overflow: hidden; margin: 6px 0 2px; }
.sg-meter-other { background: var(--text-3); opacity: 0.45; border-radius: 3px; }
.sg-meter-up { background: var(--accent); border-radius: 3px; }
.sg-note { margin: 0; padding: 0 16px 12px; color: var(--text-3); font-size: var(--fs-sm); }
.sg-cols { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; padding: 12px 16px; }
.sg-h { margin: 0 0 8px; font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; }
.sg-bar-row { display: grid; grid-template-columns: 110px 1fr 70px; align-items: center; gap: 10px; height: 26px; }
.sg-bar-lab { font-size: var(--fs-sm); color: var(--text-2); }
.sg-bar { height: 8px; border-radius: 4px; background: var(--bg-hover); overflow: hidden; }
.sg-bar span { display: block; height: 100%; border-radius: 4px; background: var(--accent); }
.sg-bar-val { font-size: var(--fs-sm); color: var(--text-2); text-align: right; }
.sg-table { padding: 4px 8px 8px; }
.sg-row { display: flex; align-items: center; gap: 12px; padding: 8px; border-radius: var(--r-sm); }
.sg-row:hover { background: var(--bg-hover); }
.sg-file { flex: 1; min-width: 0; }
.sg-name { display: inline-flex; align-items: center; gap: 4px; max-width: 100%; font-size: var(--fs-md); color: var(--text); }
.sg-uses { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 3px; }
.sg-use { font-size: 11px; padding: 1px 7px; border-radius: var(--r-full); background: var(--bg-hover); color: var(--text-2); max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
a.sg-use:hover { color: var(--text); }
.sg-use.trash { color: var(--orange); }
.sg-use.orphan { color: var(--text-3); }
.sg-more { font-size: 11px; color: var(--text-3); }
.sg-type, .sg-date { font-size: var(--fs-sm); color: var(--text-3); flex: none; }
.sg-type { width: 70px; }
.sg-date { width: 90px; }
.sg-size { font-size: var(--fs-sm); color: var(--text); width: 70px; text-align: right; flex: none; }
.sg-spacer { width: 24px; flex: none; }
@media (max-width: 720px) { .sg-cols { grid-template-columns: 1fr; } .sg-type, .sg-date { display: none; } }
</style>
