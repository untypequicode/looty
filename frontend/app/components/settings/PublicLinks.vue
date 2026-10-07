<script setup lang="ts">
import { Globe, Link2, Copy, Ban, ExternalLink } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

/** Liens publics (documents publiés) et liens d'invitation actifs — instance ou espace. */
const props = defineProps<{ wsId?: string }>()
const { t, relativeTime, formatDate } = useI18n()
const qc = useQueryClient()
interface Links {
  pages: { id: string; title: string; icon: string | null; url: string; workspace_id: string; workspace_name: string; updated_at: string; updated_by: string | null }[]
  invites: { id: string; workspace_id: string; workspace_name: string; role: string; role_name: string | null; expires_at: string | null; max_uses: number | null; uses: number; created_at: string; created_by: string | null }[]
}
const endpoint = computed(() => (props.wsId ? `/api/workspaces/${props.wsId}/admin/public-links` : '/api/admin/insights/public-links'))
const { data } = useQuery({ queryKey: computed(() => ['public-links', endpoint.value]), queryFn: () => api<Links>(endpoint.value) })
const refresh = () => qc.invalidateQueries({ queryKey: ['public-links', endpoint.value] })
function openUrl(u: string) { window.open(u, '_blank', 'noopener') }
function copy(u: string) { navigator.clipboard.writeText(u); toast.success(t('common.linkCopied')) }
async function unpublish(p: Links['pages'][number]) {
  if (!(await confirmDialog({ title: t('links.unpublishTitle', { title: p.title || t('docs.untitled') }), message: t('links.unpublishHint'), danger: true, confirmLabel: t('links.unpublish') }))) return
  if (props.wsId) await api(`/api/workspaces/${props.wsId}/pages/${p.id}/share`, { method: 'DELETE' })
  else await api(`/api/admin/insights/public-links/${p.id}`, { method: 'DELETE' })
  refresh()
}
async function revoke(i: Links['invites'][number]) {
  if (props.wsId) await api(`/api/workspaces/${props.wsId}/invites/${i.id}`, { method: 'DELETE' })
  else await api(`/api/admin/insights/invites/${i.id}`, { method: 'DELETE' })
  refresh()
}
</script>

<template>
  <SettingsSection :title="t('links.pages')" :description="t('links.pagesHint')">
    <div v-for="p in data?.pages ?? []" :key="p.id" class="lk">
      <span class="lk-ic"><Globe :size="15" /></span>
      <div class="lk-body">
        <NuxtLink :to="`/w/${p.workspace_id}/doc/${p.id}`" class="lk-title truncate">{{ p.icon ? p.icon + ' ' : '' }}{{ p.title || t('docs.untitled') }}</NuxtLink>
        <div class="lk-meta truncate">{{ wsId ? '' : p.workspace_name + ' · ' }}{{ t('links.updated', { time: relativeTime(p.updated_at), name: p.updated_by ?? '?' }) }}</div>
      </div>
      <UiButton size="sm" variant="ghost" :icon="ExternalLink" :aria-label="t('common.open')" @click="openUrl(p.url)" />
      <UiButton size="sm" variant="ghost" :icon="Copy" :aria-label="t('common.copy')" @click="copy(p.url)" />
      <UiButton size="sm" variant="ghost" :icon="Ban" @click="unpublish(p)">{{ t('links.unpublish') }}</UiButton>
    </div>
    <p v-if="data && !data.pages.length" class="lk-empty"><Globe :size="16" /> {{ t('links.noPages') }}</p>
  </SettingsSection>
  <SettingsSection :title="t('links.invites')" :description="t('links.invitesHint')">
    <div v-for="i in data?.invites ?? []" :key="i.id" class="lk">
      <span class="lk-ic"><Link2 :size="15" /></span>
      <div class="lk-body">
        <div class="lk-title truncate">{{ wsId ? '' : i.workspace_name + ' · ' }}{{ i.role_name ?? t(`roles.${i.role}`) }}</div>
        <div class="lk-meta truncate">
          {{ t('settings.members.createdBy', { name: i.created_by ?? '?', time: relativeTime(i.created_at) }) }} ·
          {{ i.expires_at ? t('settings.members.expires', { date: formatDate(i.expires_at) }) : t('settings.members.noExpiry') }} ·
          {{ i.max_uses ? t('settings.members.uses', { uses: i.uses, max: i.max_uses }) : t('settings.members.usesUnlimited', { uses: i.uses }) }}
        </div>
      </div>
      <UiButton size="sm" variant="ghost" :icon="Ban" @click="revoke(i)">{{ t('common.revoke') }}</UiButton>
    </div>
    <p v-if="data && !data.invites.length" class="lk-empty"><Link2 :size="16" /> {{ t('settings.members.noInvites') }}</p>
  </SettingsSection>
</template>

<style scoped>
.lk { display: flex; align-items: center; gap: 10px; padding: 10px 16px; }
.lk + .lk { border-top: 1px solid var(--border); }
.lk-ic { width: 28px; height: 28px; border-radius: var(--r-sm); display: grid; place-items: center; background: var(--bg-hover); color: var(--text-2); flex-shrink: 0; }
.lk-body { flex: 1; min-width: 0; }
.lk-title { font-weight: 500; display: block; color: var(--text); }
.lk-meta { font-size: var(--fs-sm); color: var(--text-3); }
.lk-empty { display: flex; align-items: center; gap: 8px; padding: 16px; margin: 0; color: var(--text-3); }
</style>
