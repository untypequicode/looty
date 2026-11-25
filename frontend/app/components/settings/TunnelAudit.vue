<script setup lang="ts">
import { useQuery } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'

interface Ev { id: number; at: string; event: string; tunnel_name?: string | null; owner_name?: string | null; owner_email?: string | null; actor_name?: string | null; details: Record<string, unknown> }
const { t } = useI18n()
const { data } = useQuery({ queryKey: ['admin', 'tunnel-audit'], queryFn: () => api<{ retention_days: number; events: Ev[] }>('/api/tunnels/audit?limit=100') })
const when = (s: string) => new Date(s).toLocaleString()
const who = (e: Ev) => e.actor_name || e.owner_name || e.owner_email || ''
const label = (e: string) => { const k = `settings.instance.tunnelEvents.${e}`; const v = t(k); return v === k ? e : v }
const detail = (e: Ev) => Object.entries(e.details ?? {}).filter(([, v]) => v !== null && typeof v !== 'object').map(([k, v]) => `${k}: ${v}`).join(', ')
</script>

<template>
  <SettingsSection :title="t('settings.instance.tunnelAudit')">
    <p v-if="!data?.events.length" class="empty">{{ t('settings.instance.tunnelAuditEmpty') }}</p>
    <div v-else class="wrap">
      <table>
        <thead><tr><th>{{ t('settings.instance.tunnelAuditWhen') }}</th><th>{{ t('settings.instance.tunnelAuditEvent') }}</th><th>{{ t('settings.instance.tunnelAuditWho') }}</th><th>{{ t('settings.instance.tunnelAuditTunnel') }}</th><th>{{ t('settings.instance.tunnelAuditDetails') }}</th></tr></thead>
        <tbody>
          <tr v-for="e in data.events" :key="e.id">
            <td>{{ when(e.at) }}</td><td>{{ label(e.event) }}</td><td>{{ who(e) }}</td><td>{{ e.tunnel_name }}</td><td class="det">{{ detail(e) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </SettingsSection>
</template>

<style scoped>
.wrap { max-height: 320px; overflow: auto; }
table { width: 100%; border-collapse: collapse; font-size: 12.5px; }
th, td { text-align: left; padding: 5px 8px; border-bottom: 1px solid var(--border); white-space: nowrap; }
.det { white-space: normal; color: var(--text-muted); }
.empty { color: var(--text-muted); font-size: 13px; }
</style>
