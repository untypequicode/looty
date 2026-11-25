<script setup lang="ts">
import { Laptop, Copy, Unplug, CircleCheck, CircleAlert, LoaderCircle, TriangleAlert, ExternalLink } from 'lucide-vue-next'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace } from '~/composables/useWorkspace'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import {
  bridgeCommands, instanceUrl, tunnelPhase, formatLatency, machineLabel, validLocalUrl,
  DEFAULT_JUPYTER_URL, COMPOSE_URL, RELEASES_URL, type TunnelLive,
} from '~/utils/tunnel'

/**
 * « Utiliser mon PC » (ADR 0020) : crée le jeton de tunnel du projet, affiche une seule fois la ligne de lancement de
 * l'agent `looty-bridge`, puis le statut en direct. Le tunnel attaché au projet vient du parent (`GET …/jupyter`, sondé).
 */
export interface ProjectTunnel extends TunnelLive { id: string; name?: string | null; mine?: boolean }

const props = defineProps<{ projectId: string; projectName?: string; tunnel: ProjectTunnel | null; canEdit: boolean; kernel?: string }>()
const emit = defineEmits<{
  /** Le projet cible (ou ne cible plus) un tunnel : le parent recharge la configuration. */
  (e: 'changed'): void
  /** Repli demandé quand le tunnel est tombé (moteur du navigateur). */
  (e: 'fallback'): void
}>()
const { t } = useI18n()
const { wsId } = useWorkspace()

type Step = 'idle' | 'setup' | 'launch'
const step = ref<Step>(props.tunnel ? 'idle' : 'idle')
const jupyterOn = ref(true)
const localUrl = ref(DEFAULT_JUPYTER_URL)
const days = ref(30)
const confirmed = ref(false)
const busy = ref(false)
const error = ref('')
const token = ref('')
const created = ref<string | null>(null)
const fresh = ref(false)
const tab = ref<'nix' | 'compose' | 'binary'>('nix')

const phase = computed(() => tunnelPhase(props.tunnel, fresh.value && props.tunnel?.id === created.value))
const owner = computed(() => !!props.tunnel?.mine)
const commands = computed(() => bridgeCommands(instanceUrl(typeof location === 'undefined' ? '' : location.origin), token.value, jupyterOn.value ? [{ name: 'jupyter', url: localUrl.value.trim() }] : []))
const line = computed(() => commands.value[tab.value])
const urlOk = computed(() => !jupyterOn.value || validLocalUrl(localUrl.value))
const canCreate = computed(() => confirmed.value && jupyterOn.value && urlOk.value && !busy.value)

watch(() => props.tunnel?.connected, (c) => { if (c) fresh.value = false })

function startSetup() {
  error.value = ''
  confirmed.value = false
  step.value = 'setup'
}

async function create() {
  busy.value = true
  error.value = ''
  let id: string | null = null
  try {
    const name = (props.projectName ? `${props.projectName} · ` : '') + t('tunnel.defaultName')
    const r = await api<{ id: string; token: string }>('/api/tunnels', {
      method: 'POST', silent: true,
      body: { name: name.slice(0, 80), workspace_id: wsId.value, project_id: props.projectId, expires_in_days: days.value, ack_remote_code: confirmed.value },
    })
    id = r.id
    token.value = r.token
    await api(`/api/projects/${props.projectId}/jupyter`, { method: 'PUT', silent: true, body: { tunnel_id: r.id, tunnel_service: 'jupyter', kernel: props.kernel || 'python3' } })
    created.value = r.id
    fresh.value = true
    step.value = 'launch'
    emit('changed')
  } catch (e) {
    // Jeton créé mais projet non attaché : on le révoque pour ne rien laisser traîner.
    if (id) await api(`/api/tunnels/${id}`, { method: 'DELETE', silent: true }).catch(() => {})
    token.value = ''
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}

async function copy() {
  try {
    await navigator.clipboard.writeText(line.value)
    toast.success(t('common.copied'))
  } catch {
    toast.error(t('tunnel.copyFailed'))
  }
}

function hideToken() {
  token.value = ''
  step.value = 'idle'
}

async function stop() {
  const tn = props.tunnel
  if (!tn) return
  const ok = await confirmDialog({ title: t('tunnel.stopTitle'), message: owner.value ? t('tunnel.stopMessage') : t('tunnel.detachMessage'), confirmLabel: owner.value ? t('tunnel.stopConfirm') : t('tunnel.detach'), danger: true })
  if (!ok) return
  busy.value = true
  try {
    await api(`/api/projects/${props.projectId}/jupyter`, { method: 'DELETE', silent: true })
    if (owner.value) await api(`/api/tunnels/${tn.id}`, { method: 'DELETE', silent: true })
    token.value = ''
    step.value = 'idle'
    toast.success(t('tunnel.stopped'))
    emit('changed')
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <section class="tb" data-testid="tunnel-box" :aria-label="t('tunnel.title')">
    <header class="tb-head">
      <Laptop :size="15" aria-hidden="true" />
      <strong>{{ t('tunnel.title') }}</strong>
    </header>

    <!-- Tunnel du projet : statut en direct -->
    <template v-if="tunnel && step !== 'launch'">
      <p class="tb-status" :class="phase" role="status" aria-live="polite" data-testid="tunnel-status" :data-phase="phase">
        <component :is="phase === 'connected' ? CircleCheck : phase === 'waiting' ? LoaderCircle : CircleAlert" :size="14" :class="{ spin: phase === 'waiting' }" aria-hidden="true" />
        <span v-if="phase === 'connected'">{{ t('tunnel.connected') }} · {{ machineLabel(tunnel.agent) }}<template v-if="tunnel.latency_ms != null"> · {{ formatLatency(tunnel.latency_ms) }}</template></span>
        <span v-else-if="phase === 'waiting'">{{ t('tunnel.waiting') }}</span>
        <span v-else-if="phase === 'unusable'">{{ t('tunnel.unusable') }}</span>
        <span v-else>{{ t('tunnel.disconnected') }}</span>
      </p>
      <p class="tb-meta">{{ tunnel.name }}<template v-if="tunnel.service"> · <code>{{ tunnel.service }}</code></template></p>
      <div v-if="phase === 'disconnected' || phase === 'unusable'" class="tb-drop" role="alert" data-testid="tunnel-dropped">
        <TriangleAlert :size="14" aria-hidden="true" />
        <div>
          <p>{{ phase === 'unusable' ? t('tunnel.unusableHint') : t('tunnel.droppedHint') }}</p>
          <UiButton size="xs" variant="ghost" @click="emit('fallback')">{{ t('tunnel.useBrowser') }}</UiButton>
        </div>
      </div>
      <p v-if="!owner" class="tb-meta">{{ t('tunnel.notYours') }}</p>
      <p v-if="error" class="tb-err" role="alert"><CircleAlert :size="14" aria-hidden="true" /> {{ error }}</p>
      <div v-if="canEdit" class="tb-actions">
        <UiButton size="sm" variant="ghost" :icon="Unplug" :loading="busy" data-testid="tunnel-stop" @click="stop">{{ owner ? t('tunnel.stop') : t('tunnel.detach') }}</UiButton>
      </div>
    </template>

    <!-- Pas de tunnel : accueil -->
    <template v-else-if="step === 'idle'">
      <p class="tb-meta">{{ t('tunnel.intro') }}</p>
      <p v-if="!canEdit" class="tb-meta">{{ t('tunnel.needEdit') }}</p>
      <div v-else class="tb-actions">
        <UiButton size="sm" variant="primary" :icon="Laptop" data-testid="tunnel-use-pc" @click="startSetup">{{ t('tunnel.usePc') }}</UiButton>
      </div>
    </template>

    <!-- Choix des services, avertissement, création -->
    <form v-else-if="step === 'setup'" class="tb-form" @submit.prevent="create">
      <fieldset class="tb-fs">
        <legend>{{ t('tunnel.services') }}</legend>
        <label class="tb-check"><input v-model="jupyterOn" type="checkbox" disabled data-testid="tunnel-svc-jupyter"> {{ t('tunnel.svcJupyter') }}</label>
        <label class="tb-check off"><input type="checkbox" disabled> {{ t('tunnel.svcLatex') }} <span class="tb-soon">{{ t('tunnel.soon') }}</span></label>
      </fieldset>
      <label class="tb-field">
        <span>{{ t('tunnel.localUrl') }}</span>
        <UiInput v-model="localUrl" type="url" spellcheck="false" autocomplete="off" data-testid="tunnel-local-url" :aria-invalid="!urlOk" />
        <small>{{ t('tunnel.localUrlHint') }}</small>
      </label>
      <label class="tb-field">
        <span>{{ t('tunnel.duration') }}</span>
        <select v-model.number="days" class="tb-select">
          <option :value="7">{{ t('tunnel.days', { n: 7 }) }}</option>
          <option :value="30">{{ t('tunnel.days', { n: 30 }) }}</option>
          <option :value="90">{{ t('tunnel.days', { n: 90 }) }}</option>
        </select>
      </label>
      <div class="tb-warn" role="note">
        <TriangleAlert :size="14" aria-hidden="true" />
        <div>
          <p><strong>{{ t('tunnel.warnTitle') }}</strong></p>
          <p>{{ t('tunnel.warnBody') }}</p>
          <p>{{ t('tunnel.warnRestrict') }}</p>
        </div>
      </div>
      <label class="tb-check"><input v-model="confirmed" type="checkbox" data-testid="tunnel-confirm"> {{ t('tunnel.confirm') }}</label>
      <p v-if="error" class="tb-err" role="alert"><CircleAlert :size="14" aria-hidden="true" /> {{ error }}</p>
      <div class="tb-actions">
        <UiButton size="sm" variant="ghost" @click="step = 'idle'">{{ t('common.cancel') }}</UiButton>
        <UiButton size="sm" variant="primary" type="submit" :loading="busy" :disabled="!canCreate" data-testid="tunnel-create">{{ t('tunnel.create') }}</UiButton>
      </div>
    </form>

    <!-- Jeton (une seule fois) et lignes à copier -->
    <template v-else>
      <p class="tb-meta">{{ t('tunnel.launchIntro') }}</p>
      <p class="tb-once" role="note"><TriangleAlert :size="14" aria-hidden="true" /> {{ t('tunnel.tokenOnce') }}</p>
      <div class="tb-token">
        <code data-testid="tunnel-token">{{ token }}</code>
      </div>
      <UiSegmented v-model="tab" size="sm" :aria-label="t('tunnel.launchWith')" :options="[{ value: 'nix', label: 'nix run' }, { value: 'compose', label: 'Docker Compose' }, { value: 'binary', label: t('tunnel.binary') }]" />
      <div class="tb-line">
        <code data-testid="tunnel-line" tabindex="0" :aria-label="t('tunnel.lineLabel')">{{ line }}</code>
        <UiButton size="sm" :icon="Copy" data-testid="tunnel-copy" @click="copy">{{ t('common.copy') }}</UiButton>
      </div>
      <p v-if="tab === 'compose'" class="tb-meta"><a :href="COMPOSE_URL" target="_blank" rel="noopener">{{ t('tunnel.composeFile') }} <ExternalLink :size="12" aria-hidden="true" /></a> · {{ t('tunnel.composeHint') }}</p>
      <p v-else-if="tab === 'binary'" class="tb-meta"><a :href="RELEASES_URL" target="_blank" rel="noopener">{{ t('tunnel.download') }} <ExternalLink :size="12" aria-hidden="true" /></a></p>
      <p v-else class="tb-meta">{{ t('tunnel.nixHint') }}</p>
      <p v-if="tunnel" class="tb-status" :class="phase" role="status" aria-live="polite" data-testid="tunnel-status" :data-phase="phase">
        <component :is="phase === 'connected' ? CircleCheck : phase === 'waiting' ? LoaderCircle : CircleAlert" :size="14" :class="{ spin: phase === 'waiting' }" aria-hidden="true" />
        <span v-if="phase === 'connected'">{{ t('tunnel.connected') }} · {{ machineLabel(tunnel.agent) }}<template v-if="tunnel.latency_ms != null"> · {{ formatLatency(tunnel.latency_ms) }}</template></span>
        <span v-else-if="phase === 'waiting'">{{ t('tunnel.waiting') }}</span>
        <span v-else>{{ t('tunnel.disconnected') }}</span>
      </p>
      <div class="tb-actions">
        <UiButton size="sm" variant="ghost" :icon="Unplug" @click="stop">{{ t('tunnel.stop') }}</UiButton>
        <span class="tb-spacer" />
        <UiButton size="sm" variant="primary" data-testid="tunnel-done" @click="hideToken">{{ t('tunnel.done') }}</UiButton>
      </div>
    </template>
  </section>
</template>

<style scoped>
.tb { display: flex; flex-direction: column; gap: 10px; padding: 12px; border: 1px solid var(--border); border-radius: var(--r-md); }
.tb-head { display: flex; align-items: center; gap: 8px; }
.tb-meta { margin: 0; color: var(--text-2); font-size: var(--fs-sm, 13px); line-height: 1.5; }
.tb-meta a { color: inherit; text-decoration: underline; }
.tb-status { display: flex; align-items: center; gap: 6px; margin: 0; font-size: var(--fs-sm, 13px); font-weight: 500; }
.tb-status.connected { color: var(--green); }
.tb-status.waiting { color: var(--text-2); }
.tb-status.disconnected, .tb-status.unusable { color: var(--red); }
.spin { animation: tb-spin 1s linear infinite; }
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
@keyframes tb-spin { to { transform: rotate(360deg); } }
.tb-drop, .tb-warn, .tb-once { display: flex; gap: 8px; align-items: flex-start; padding: 8px 10px; border-radius: var(--r-sm); font-size: var(--fs-sm, 13px); color: var(--orange, #e2a03f); background: color-mix(in srgb, var(--orange, #e2a03f) 10%, transparent); }
.tb-drop p, .tb-warn p { margin: 0 0 4px; color: var(--text); line-height: 1.5; }
.tb-once { margin: 0; color: var(--text); }
.tb-drop :deep(svg), .tb-warn :deep(svg), .tb-once :deep(svg) { flex: none; margin-top: 2px; color: var(--orange, #e2a03f); }
.tb-err { display: flex; gap: 6px; margin: 0; padding: 8px 10px; border-radius: var(--r-sm); font-size: var(--fs-sm, 13px); color: var(--red); background: color-mix(in srgb, var(--red) 8%, transparent); }
.tb-form { display: flex; flex-direction: column; gap: 10px; }
.tb-fs { border: 0; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 6px; }
.tb-fs legend { font-size: var(--fs-sm, 13px); font-weight: 500; margin-bottom: 4px; }
.tb-field { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm, 13px); font-weight: 500; }
.tb-field small { font-weight: 400; color: var(--text-3); }
.tb-check { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm, 13px); }
.tb-check.off { color: var(--text-3); }
.tb-soon { font-size: 11px; padding: 1px 6px; border-radius: 99px; background: var(--bg-code); }
.tb-select { height: 32px; padding: 0 8px; border-radius: var(--r-md); border: 1px solid var(--border); background: var(--bg-panel, var(--bg)); color: var(--text); font: inherit; font-weight: 400; }
.tb-token, .tb-line { display: flex; align-items: center; gap: 8px; padding: 8px 8px 8px 12px; border-radius: var(--r-md); background: var(--bg-code); }
.tb-token code, .tb-line code { flex: 1; min-width: 0; font-family: var(--font-mono, monospace); font-size: 12px; word-break: break-all; user-select: all; }
.tb-actions { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.tb-spacer { flex: 1; }
</style>
