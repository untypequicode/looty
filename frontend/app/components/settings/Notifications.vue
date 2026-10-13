<script setup lang="ts">
import { Mail, BellRing, Trash2, Send, Smartphone, MonitorSmartphone, AlertTriangle } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { usePush, pushSupported } from '~/composables/usePush'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

interface EventPref { event: string; email: boolean; push: boolean }
interface Prefs { email_enabled: boolean; events: EventPref[]; email_available: boolean; email: string }
interface Device { id: string; device?: string | null; endpoint: string; endpoint_host: string; created_at: string; last_used_at?: string | null }

const { t, relativeTime } = useI18n()
const qc = useQueryClient()
const KEY = ['me', 'notification-prefs']
const DKEY = ['me', 'push-devices']
const { data: prefs } = useQuery({ queryKey: KEY, queryFn: () => api<Prefs>('/api/user/notifications') })
const { data: devices } = useQuery({ queryKey: DKEY, queryFn: () => api<Device[]>('/api/user/push/subscriptions') })
const push = usePush()
const here = ref<string | null>(null)

async function refreshHere() {
  await push.refresh()
  const reg = pushSupported() ? await navigator.serviceWorker.getRegistration('/') : null
  here.value = (await reg?.pushManager.getSubscription())?.endpoint ?? null
}
onMounted(refreshHere)

async function save(body: Partial<Prefs>) {
  const updated = await api<Prefs>('/api/user/notifications', { method: 'PUT', body })
  qc.setQueryData(KEY, updated)
}
function toggle(e: EventPref, channel: 'email' | 'push', value: boolean) {
  const next = { ...e, [channel]: value }
  qc.setQueryData<Prefs>(KEY, (old) => (old ? { ...old, events: old.events.map((x) => (x.event === e.event ? next : x)) } : old))
  save({ events: [next] })
}

const busy = ref(false)
async function enablePush() {
  busy.value = true
  try {
    if (await push.enable()) toast.success(t('settings.notify.pushEnabled'))
    else if (push.state.value === 'denied') toast.error(t('settings.notify.pushDenied'))
    await refreshHere()
    qc.invalidateQueries({ queryKey: DKEY })
  } catch (e) {
    toast.error(errorMessage(e))
  } finally { busy.value = false }
}
async function removeDevice(d: Device) {
  if (d.endpoint === here.value) await push.disable(d.id)
  else await api(`/api/user/push/subscriptions/${d.id}`, { method: 'DELETE' })
  await refreshHere()
  qc.invalidateQueries({ queryKey: DKEY })
}
async function testPush() {
  const r = await api<{ sent: number }>('/api/user/push/test', { method: 'POST' })
  if (r.sent) toast.success(t('settings.notify.testSent', { n: r.sent }))
  else toast.error(t('settings.notify.testNone'))
}
// Sans push (http, navigateur ancien) : notifications système tant que Looty est ouvert.
const desktop = ref(typeof window !== 'undefined' && 'Notification' in window ? window.Notification.permission : 'denied')
async function enableDesktop() { desktop.value = await window.Notification.requestPermission() }
// iPhone / iPad : le push n'existe que pour l'application ajoutée à l'écran d'accueil (iOS 16.4+).
const iosBrowser = typeof navigator !== 'undefined' && /iPhone|iPad|iPod/.test(navigator.userAgent) && !(window.matchMedia?.('(display-mode: standalone)').matches || (navigator as any).standalone)
const anyPush = computed(() => (devices.value?.length ?? 0) > 0)
</script>

<template>
  <SettingsSection :title="t('settings.notify.channels')" :description="t('settings.notify.channelsHint')">
    <SettingsRow :label="t('settings.notify.emailLabel')" :hint="prefs?.email_available ? t('settings.notify.emailHint', { email: prefs?.email ?? '' }) : t('settings.notify.emailUnavailable')">
      <UiSwitch :model-value="!!prefs?.email_enabled && !!prefs?.email_available" :disabled="!prefs?.email_available" @update:model-value="(v: boolean) => save({ email_enabled: v })" />
    </SettingsRow>
    <SettingsRow :label="t('settings.notify.pushLabel')" :hint="push.state.value === 'unsupported' ? (iosBrowser ? t('settings.notify.pushIos') : t('settings.notify.pushUnsupported')) : push.state.value === 'denied' ? t('settings.notify.pushDenied') : t('settings.notify.pushHint')">
      <span v-if="push.state.value === 'on'" class="ok">{{ t('settings.notify.pushOnHere') }}</span>
      <UiButton v-else-if="push.state.value === 'unsupported' && desktop === 'default'" size="sm" @click="enableDesktop">{{ t('settings.notify.enableDesktop') }}</UiButton>
      <span v-else-if="push.state.value === 'unsupported' && desktop === 'granted'" class="ok">{{ t('settings.notify.desktopOn') }}</span>
      <UiButton v-else-if="push.state.value === 'off'" size="sm" :icon="BellRing" :loading="busy" @click="enablePush">{{ t('settings.notify.enableHere') }}</UiButton>
    </SettingsRow>
  </SettingsSection>

  <SettingsSection :title="t('settings.notify.events')" :description="t('settings.notify.eventsHint')">
    <div class="matrix">
      <div class="mx-head">
        <span />
        <span class="mx-col"><Mail :size="14" /> {{ t('settings.notify.email') }}</span>
        <span class="mx-col"><BellRing :size="14" /> {{ t('settings.notify.push') }}</span>
      </div>
      <div v-for="e in prefs?.events ?? []" :key="e.event" class="mx-row">
        <div class="mx-label">
          <div class="mx-title">{{ t(`settings.notify.ev.${e.event}`) }}</div>
          <div class="mx-hint">{{ t(`settings.notify.evHint.${e.event}`) }}</div>
        </div>
        <span class="mx-cell"><UiSwitch :model-value="e.email" :disabled="!prefs?.email_available || !prefs?.email_enabled" :aria-label="t('settings.notify.email')" @update:model-value="(v: boolean) => toggle(e, 'email', v)" /></span>
        <span class="mx-cell"><UiSwitch :model-value="e.push" :aria-label="t('settings.notify.push')" @update:model-value="(v: boolean) => toggle(e, 'push', v)" /></span>
      </div>
    </div>
    <p v-if="!anyPush" class="note"><AlertTriangle :size="13" /> {{ t('settings.notify.noDeviceNote') }}</p>
    <p class="note">{{ t('settings.notify.emailDelayNote') }}</p>
  </SettingsSection>

  <SettingsSection :title="t('settings.notify.devices')" :description="t('settings.notify.devicesHint')">
    <template v-if="anyPush" #actions><UiButton size="sm" variant="ghost" :icon="Send" @click="testPush">{{ t('settings.notify.test') }}</UiButton></template>
    <SettingsRow v-for="d in devices ?? []" :key="d.id" :label="d.device || d.endpoint_host" :hint="`${d.endpoint === here ? t('settings.notify.thisDevice') + ' · ' : ''}${d.last_used_at ? t('settings.security.lastUsed', { time: relativeTime(d.last_used_at) }) : t('settings.security.neverUsed')}`">
      <template #text><span class="dev-ico"><MonitorSmartphone v-if="d.endpoint === here" :size="14" /></span></template>
      <UiButton size="sm" variant="ghost" :icon="Trash2" @click="removeDevice(d)">{{ t('common.remove') }}</UiButton>
    </SettingsRow>
    <div v-if="!anyPush" class="empty"><Smartphone :size="16" /> {{ t('settings.notify.noDevices') }}</div>
  </SettingsSection>
</template>

<style scoped>
.ok { color: var(--success, #4cb782); font-weight: 500; font-size: var(--fs-sm); }
.matrix { display: flex; flex-direction: column; }
.mx-head, .mx-row { display: grid; grid-template-columns: minmax(0, 1fr) 88px 88px; align-items: center; gap: 8px; padding: 10px 16px; }
.mx-head { color: var(--text-3); font-size: var(--fs-sm); font-weight: 500; border-bottom: 1px solid var(--border); }
.mx-col { display: inline-flex; align-items: center; justify-content: center; gap: 6px; }
.mx-row + .mx-row { border-top: 1px solid var(--border); }
.mx-title { font-weight: 500; }
.mx-hint { color: var(--text-3); font-size: var(--fs-sm); margin-top: 2px; }
.mx-cell { display: flex; justify-content: center; }
.note { display: flex; align-items: center; gap: 6px; margin: 0; padding: 10px 16px; border-top: 1px solid var(--border); color: var(--text-3); font-size: var(--fs-sm); }
.empty { display: flex; align-items: center; gap: 8px; padding: 14px 16px; color: var(--text-3); }
.dev-ico { color: var(--accent); }
@media (max-width: 639px) {
  .mx-head, .mx-row { grid-template-columns: minmax(0, 1fr) 56px 56px; padding: 10px 12px; }
  .mx-col { flex-direction: column; gap: 2px; font-size: 11px; }
}
</style>
