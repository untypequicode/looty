<script setup lang="ts">
import { CircleCheck, TriangleAlert, CircleX, CircleMinus, RefreshCw, Send, BookOpen } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useI18n, currentLocale } from '~/composables/useI18n'
import { useWorkspaces } from '~/composables/useWorkspace'
import { socketBase } from '~/composables/useRealtime'
import { toast } from '~/composables/useToast'

/** Diagnostic de l'instance : contrôles du serveur (GET /api/admin/doctor) et du navigateur (WebSocket, adresse). */
type Status = 'ok' | 'warn' | 'error' | 'skip'
interface Check { id: string; status: Status; code: string; detail: string }
const { t } = useI18n()
const config = useRuntimeConfig()
const { data, isFetching, refetch, error } = useQuery({ queryKey: ['admin', 'doctor'], queryFn: () => api<{ version: string; base_url: string; checks: Check[] }>('/api/admin/doctor'), refetchOnWindowFocus: false, staleTime: Infinity })
const { data: workspaces } = useWorkspaces()

const browser = ref<Check[]>([])
function probe(url: string, ms = 6000): Promise<{ ok: boolean; detail: string }> {
  return new Promise((resolve) => {
    let done = false
    const finish = (ok: boolean, detail = '') => { if (!done) { done = true; clearTimeout(timer); try { ws.close() } catch { /* déjà fermé */ } resolve({ ok, detail }) } }
    const ws = new WebSocket(url)
    const timer = setTimeout(() => finish(false, 'timeout'), ms)
    ws.onopen = () => finish(true)
    ws.onerror = () => finish(false, 'refused')
  })
}
async function runBrowser(baseUrl: string) {
  const out: Check[] = []
  const origin = location.origin
  out.push(origin === baseUrl
    ? { id: 'origin', status: 'ok', code: '', detail: origin }
    : { id: 'origin', status: 'error', code: 'origin_mismatch', detail: `${origin} ≠ ${baseUrl}` })
  out.push(location.protocol === 'https:' || ['localhost', '127.0.0.1'].includes(location.hostname)
    ? { id: 'https', status: 'ok', code: '', detail: location.protocol }
    : { id: 'https', status: 'warn', code: 'no_https', detail: location.protocol })
  const ws = workspaces.value?.[0]?.id
  if (ws) {
    const r = await probe(`${socketBase(config.public.wsBase as string)}/api/ws?workspace_id=${ws}`)
    out.push({ id: 'ws', status: r.ok ? 'ok' : 'error', code: r.ok ? '' : 'ws_failed', detail: r.detail })
  } else out.push({ id: 'ws', status: 'skip', code: 'ws_no_space', detail: '' })
  const c = await probe(`${(config.public.collabUrl as string) || `${socketBase(config.public.wsBase as string)}/collab`}`)
  out.push({ id: 'collab', status: c.ok ? 'ok' : 'error', code: c.ok ? '' : 'collab_failed', detail: c.detail })
  browser.value = out
}
watch(() => data.value?.base_url, (b) => { if (b) runBrowser(b) }, { immediate: true })
async function again() { browser.value = []; await refetch() ; if (data.value) runBrowser(data.value.base_url) }

const ORDER = ['origin', 'https', 'reachable', 'base_url', 'proxy', 'ws', 'collab', 'caldav', 'oidc', 'smtp', 'database', 'files', 'secret_key', 'version']
const checks = computed(() => [...(data.value?.checks ?? []), ...browser.value].sort((a, b) => ORDER.indexOf(a.id) - ORDER.indexOf(b.id)))
const counts = computed(() => ({
  error: checks.value.filter((c) => c.status === 'error').length,
  warn: checks.value.filter((c) => c.status === 'warn').length,
}))
const ICON = { ok: CircleCheck, warn: TriangleAlert, error: CircleX, skip: CircleMinus }
const docUrl = computed(() => `https://looty.untypequicode.fr${currentLocale() === 'fr' ? '/fr' : ''}/docs/host/operate/troubleshooting/`)
function message(c: Check) {
  if (!c.code) return c.detail
  const key = `doctor.msg.${c.code}`
  const text = t(key)
  return text === key ? c.detail : text
}

const testing = ref(false)
async function sendTest() {
  testing.value = true
  try {
    const r = await api<{ sent_to: string }>('/api/admin/mail/test', { method: 'POST', silent: true })
    toast.success(t('doctor.testSent', { email: r.sent_to }))
  } catch (e) { toast.error(errorMessage(e)) } finally { testing.value = false }
}
</script>

<template>
  <SettingsSection :title="t('doctor.title')" :description="t('doctor.description')">
    <template #actions>
      <UiButton size="sm" :icon="RefreshCw" :loading="isFetching" @click="again">{{ t('doctor.run') }}</UiButton>
    </template>
    <div v-if="error" class="d-error">{{ errorMessage(error) }}</div>
    <div v-else-if="isFetching && !data" class="skeleton" style="height: 200px" />
    <template v-else>
      <div class="d-summary" :class="counts.error ? 'bad' : counts.warn ? 'meh' : 'good'" role="status">
        {{ counts.error ? t('doctor.summaryError', { count: counts.error }) : counts.warn ? t('doctor.summaryWarn', { count: counts.warn }) : t('doctor.summaryOk') }}
      </div>
      <div v-for="c in checks" :key="`${c.id}-${c.detail}`" class="d-row" :class="c.status" :data-check="c.id" :data-status="c.status">
        <component :is="ICON[c.status]" :size="16" class="d-ic" />
        <div class="d-body">
          <div class="d-title">{{ t(`doctor.checks.${c.id}`) }}</div>
          <div v-if="message(c)" class="d-msg">{{ message(c) }}</div>
          <div v-if="c.code && c.detail && message(c) !== c.detail" class="d-detail">{{ c.detail }}</div>
        </div>
        <UiButton v-if="c.id === 'smtp' && c.status !== 'skip'" size="sm" :icon="Send" :loading="testing" @click="sendTest">{{ t('doctor.sendTest') }}</UiButton>
        <a v-if="c.status === 'warn' || c.status === 'error'" :href="docUrl" target="_blank" rel="noopener" class="d-help"><BookOpen :size="13" /> {{ t('doctor.help') }}</a>
      </div>
      <p class="d-cli faint">{{ t('doctor.cli') }} <code>docker compose exec backend looty doctor</code></p>
    </template>
  </SettingsSection>
</template>

<style scoped>
.d-summary { margin: 14px 14px 6px; padding: 10px 12px; border-radius: var(--r-md); font-weight: 500; }
.d-summary.good { background: var(--green-soft, var(--bg-hover)); color: var(--green, inherit); }
.d-summary.meh { background: var(--amber-soft, var(--bg-hover)); color: var(--amber, inherit); }
.d-summary.bad { background: var(--red-soft); color: var(--red); }
.d-row { display: flex; align-items: flex-start; gap: 10px; padding: 12px 14px; border-top: 1px solid var(--border); }
.d-ic { margin-top: 2px; flex: none; }
.d-row.ok .d-ic { color: var(--green, currentColor); }
.d-row.warn .d-ic { color: var(--amber, currentColor); }
.d-row.error .d-ic { color: var(--red); }
.d-row.skip .d-ic { opacity: .5; }
.d-body { flex: 1; min-width: 0; }
.d-title { font-weight: 500; }
.d-msg, .d-detail { font-size: var(--fs-sm); color: var(--text-muted, inherit); overflow-wrap: anywhere; }
.d-detail { font-family: var(--font-mono, monospace); opacity: .8; }
.d-help { display: inline-flex; gap: 4px; align-items: center; font-size: var(--fs-sm); color: var(--accent); white-space: nowrap; }
.d-cli { margin: 0; padding: 12px 14px; border-top: 1px solid var(--border); font-size: var(--fs-sm); }
.d-error { padding: 14px; color: var(--red); }
</style>
