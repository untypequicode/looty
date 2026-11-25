<script setup lang="ts">
import { Trash2, Laptop } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { formatLatency, machineLabel } from '~/utils/tunnel'

/** Tunnels PC de la personne (ADR 0020) : état en direct, expiration, révocation (coupe la connexion tout de suite). */
interface Tunnel { id: string; name: string; project_id: string | null; expires_at: string; last_connected_at: string | null; status: { connected: boolean; latency_ms?: number | null; agent?: string } }

const { t, relativeTime } = useI18n()
const qc = useQueryClient()
const { data } = useQuery({
  queryKey: ['tunnels'],
  queryFn: () => api<{ enabled: boolean; tunnels: Tunnel[] }>('/api/tunnels', { silent: true }),
  refetchInterval: 10_000,
})

const hint = (x: Tunnel) => [
  x.status.connected ? `${t('tunnel.connected')} · ${machineLabel(x.status.agent)}${x.status.latency_ms != null ? ` · ${formatLatency(x.status.latency_ms)}` : ''}` : t('tunnel.disconnected'),
  t('tunnel.expires', { when: relativeTime(x.expires_at) }),
].join(' · ')

async function revoke(x: Tunnel) {
  if (!(await confirmDialog({ title: t('tunnel.revokeTitle'), message: t('tunnel.revokeMessage'), confirmLabel: t('tunnel.revoke'), danger: true }))) return
  await api(`/api/tunnels/${x.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: ['tunnels'] })
  toast.success(t('tunnel.revoked'))
}
</script>

<template>
  <SettingsSection :description="t('tunnel.settingsDescription')">
    <p v-if="data && !data.enabled" class="empty" data-testid="tunnels-off">{{ t('tunnel.disabled') }}</p>
    <p v-else-if="!data?.tunnels.length" class="empty" data-testid="tunnels-empty"><Laptop :size="16" aria-hidden="true" /> {{ t('tunnel.none') }}</p>
    <SettingsRow v-for="x in data?.tunnels ?? []" :key="x.id" :label="x.name" :hint="hint(x)" data-testid="tunnel-row">
      <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="`${t('tunnel.revoke')} · ${x.name}`" :title="t('tunnel.revoke')" data-testid="tunnel-revoke" @click="revoke(x)" />
    </SettingsRow>
  </SettingsSection>
</template>

<style scoped>
.empty { display: flex; align-items: center; gap: 8px; margin: 0; padding: 18px 16px; color: var(--text-3); }
</style>
