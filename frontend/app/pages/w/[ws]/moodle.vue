<script setup lang="ts">
import {
  GraduationCap, ExternalLink, MoreHorizontal, RefreshCw, LogOut, Bell, BellOff, Settings, AlertTriangle, ArrowLeft, Send,
  MessageSquare, KeyRound, LogIn, Plus, LayoutDashboard,
} from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace } from '~/composables/useWorkspace'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { useUi } from '~/composables/useUi'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { isDesktop } from '~/composables/useDesktop'
import {
  useMoodle, useMoodleData, useMoodleInvalidate, moodleKeys,
  type MoodleOverview, type MoodleSite, type MoodleDeadline, type MoodleConversation, type MoodleMessage, type MoodleNotification,
} from '~/composables/useMoodle'
import { useMoodleCourses } from '~/composables/useMoodleCourses'
import { useMoodleBoard, PANEL_KINDS, PANEL_ICONS } from '~/composables/useMoodleBoard'

definePageMeta({ layout: 'workspace' })

/**
 * Moodle de l'espace : connexion du compte (SSO, jeton ou identifiants), échéances
 * (à transformer en tâches), cours, messages et notifications. Le calendrier Moodle est
 * importé dans le calendrier Looty.
 */
const { t, locale, relativeTime } = useI18n()
const { wsId } = useWorkspace()
const { isMobile } = useBreakpoint()
const { openCreateTask } = useUi()
const qc = useQueryClient()
const invalidate = useMoodleInvalidate(wsId)

const { data: overview, isLoading } = useMoodle(wsId)
const conn = computed(() => overview.value?.connection ?? null)
const connected = computed(() => !!conn.value)
const tokenBroken = computed(() => !!conn.value?.last_error?.startsWith('moodle_invalid_token'))
/** Reconnexion volontaire (sans déconnecter) : pour obtenir la connexion automatique au site. */
const reconnecting = ref(false)
const showConnect = computed(() => !connected.value || reconnecting.value)
// Cours ouvert dans Looty (?course=…) : le bouton Retour du navigateur revient à la liste.
const route = useRoute()
const courseId = computed(() => Number(route.query.course) || null)
const openCourse = (id: number | null) => navigateTo({ query: { ...route.query, course: id ?? undefined } })

// --- Connexion ---
const site = ref<MoodleSite | null>(null)
const siteError = ref('')
watch(() => [overview.value?.url, connected.value] as const, async ([url]) => {
  site.value = null
  siteError.value = ''
  if (!url) return
  try { site.value = await api<MoodleSite>(`/api/workspaces/${wsId.value}/moodle/site`, { silent: true }) } catch (e) { if (!connected.value) siteError.value = errorMessage(e) }
}, { immediate: true })
const method = ref<'password' | 'token'>('token')
const otherWays = ref(false)
const form = reactive({ token: '', username: '', password: '' })
const connecting = ref(false)
const connectError = ref('')
async function connect(body?: Record<string, string>) {
  connecting.value = true
  connectError.value = ''
  try {
    const payload = body ?? (method.value === 'password' ? { username: form.username, password: form.password } : { token: form.token })
    const r = await api<{ full_name: string; calendar: boolean }>(`/api/workspaces/${wsId.value}/moodle/connect`, { method: 'POST', body: payload, silent: true })
    toast.success(t('moodle.connected', { name: r.full_name }))
    if (!r.calendar) toast(t('moodle.calendarUnavailable'))
    Object.assign(form, { token: '', username: '', password: '' })
    waiting.value = false
    reconnecting.value = false
    tab.value = 'overview'
    invalidate()
    qc.invalidateQueries({ queryKey: ['calendars'] })
  } catch (e) { connectError.value = errorMessage(e) } finally { connecting.value = false }
}

// Portail officiel : la page de connexion habituelle de la plateforme (CAS…). Moodle
// renvoie ensuite une clé à « son application » : l'application de bureau l'intercepte
// dans sa fenêtre Moodle ; dans le navigateur, le gestionnaire de protocole web+looty
// la reçoit si la plateforme le permet.
const waiting = ref(false)
/** Application Android : la connexion s’ouvre dans un écran natif qui intercepte moodlemobile://. */
const inAndroidApp = typeof window !== 'undefined' && !!(window as any).LootyApp
/** Connexion par le portail officiel dans une fenêtre de l’application (bureau, Android). */
const portal = isDesktop || inAndroidApp
async function portalLogin() {
  if (!site.value) return
  connectError.value = ''
  waiting.value = true
  if (isDesktop) {
    await fetch('/__looty/moodle/open', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ url: site.value.launch_url, login: true }) })
    return
  }
  if (inAndroidApp) {
    ;(window as any).LootyApp.postMessage(JSON.stringify({ type: 'moodleLogin', url: site.value.launch_url }))
    return
  }
  localStorage.setItem('looty.moodle.pending', wsId.value)
  try { navigator.registerProtocolHandler('web+looty', `${location.origin}/moodle-callback?link=%s`) } catch { /* navigateur sans gestionnaire */ }
  window.open(site.value.launch_url_web, 'looty-moodle', 'popup,width=960,height=780')
}
/** « Autres façons » : identifiant (comptes locaux), ou clé dans l'application de bureau. */
const usesPassword = computed(() => !portal || method.value === 'password')
function connectOther() {
  connect(usesPassword.value ? { username: form.username, password: form.password } : { token: form.token })
}
function openTokens() {
  if (site.value) window.open(site.value.tokens_url, 'looty-moodle', 'popup,width=960,height=780')
}
function onDesktopToken(e: Event) { const link = (e as CustomEvent<string>).detail; if (link) connect({ token: link }); else waiting.value = false }
function onPopupMessage(e: MessageEvent) {
  if (e.origin === location.origin && e.data?.type === 'looty-moodle-token' && typeof e.data.link === 'string') connect({ token: e.data.link })
}
// Connexion terminée dans la fenêtre Moodle (sans lien avec cette page) : simple mise à jour.
let channel: BroadcastChannel | null = null
function onChannel(e: MessageEvent) {
  if (e.data?.type !== 'connected' || e.data.ws !== wsId.value) return
  waiting.value = false
  tab.value = 'overview'
  invalidate()
  qc.invalidateQueries({ queryKey: ['calendars'] })
}
onMounted(() => {
  window.addEventListener('looty:moodle-token', onDesktopToken)
  window.addEventListener('message', onPopupMessage)
  try { channel = new BroadcastChannel('looty-moodle'); channel.onmessage = onChannel } catch { /* ancien navigateur */ }
})
onBeforeUnmount(() => { window.removeEventListener('looty:moodle-token', onDesktopToken); window.removeEventListener('message', onPopupMessage); channel?.close() })

/** Site officiel (connecté automatiquement si possible) : fenêtre Moodle de l'application de bureau, sinon nouvel onglet. */
async function openSite(url?: string) {
  const r = await api<{ url: string }>(`/api/workspaces/${wsId.value}/moodle/open${url ? `?url=${encodeURIComponent(url)}` : ''}`, { silent: true }).catch(() => ({ url: url ?? conn.value?.base_url ?? '' }))
  if (!r.url) return
  if (isDesktop) await fetch('/__looty/moodle/open', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ url: r.url, login: false }) })
  else window.open(r.url, '_blank', 'noopener')
}
// Onglet « Site Moodle » (si la plateforme accepte d'être affichée dans Looty).
const frameSrc = ref('')
async function loadFrame() {
  const r = await api<{ url: string }>(`/api/workspaces/${wsId.value}/moodle/open`, { silent: true }).catch(() => ({ url: conn.value?.base_url ?? '' }))
  frameSrc.value = r.url
}

// --- Réglage de l'adresse (administrateurs) ---
const urlDraft = ref('')
watch(overview, (o) => { if (o && !urlDraft.value) urlDraft.value = o.url ?? o.default_url }, { immediate: true })
const savingUrl = ref(false)
async function saveUrl() {
  savingUrl.value = true
  try {
    await api(`/api/workspaces/${wsId.value}/moodle/settings`, { method: 'PUT', body: { url: urlDraft.value.trim() || null } })
    await qc.invalidateQueries({ queryKey: moodleKeys.overview(wsId.value) })
  } catch { /* toast par api() */ } finally { savingUrl.value = false }
}

// --- Données ---
const tab = ref<'overview' | 'messages' | 'notifications' | 'site'>('overview')
const courses = useMoodleCourses(wsId, connected)
const deadlines = useMoodleData<MoodleDeadline[]>(wsId, 'deadlines', connected)
const convs = useMoodleData<MoodleConversation[]>(wsId, 'conversations', computed(() => connected.value && tab.value === 'messages'), 60_000)
const notifs = useMoodleData<{ notifications: MoodleNotification[]; unread: number }>(wsId, 'notifications', connected, 120_000)
const loadError = computed(() => courses.query.error.value ?? deadlines.error.value)
watch(loadError, (e) => { if (e) qc.invalidateQueries({ queryKey: moodleKeys.overview(wsId.value) }) })

const dayFmt = (d: string) => new Date(d).toLocaleDateString(locale.value, { weekday: 'short', day: 'numeric', month: 'short' })
const timeFmt = (d: string) => new Date(d).toLocaleTimeString(locale.value, { hour: '2-digit', minute: '2-digit' })
function addTask(d: MoodleDeadline) {
  openCreateTask({ title: d.name, description: `${d.course}\n${d.url}`, dueDate: d.due })
}

// Messages
const convId = ref<number | null>(null)
const thread = ref<MoodleMessage[]>([])
const threadLoading = ref(false)
const reply = ref('')
const sending = ref(false)
const threadEl = ref<HTMLElement | null>(null)
async function openConv(c: MoodleConversation) {
  convId.value = c.id
  threadLoading.value = true
  try {
    thread.value = (await api<{ messages: MoodleMessage[] }>(`/api/workspaces/${wsId.value}/moodle/conversations/${c.id}`)).messages
    if (c.unread) qc.setQueryData<MoodleConversation[]>(moodleKeys.data(wsId.value, 'conversations'), (l) => l?.map((x) => (x.id === c.id ? { ...x, unread: 0 } : x)))
  } finally {
    threadLoading.value = false
    nextTick(() => threadEl.value?.scrollTo({ top: threadEl.value.scrollHeight }))
  }
}
async function send() {
  const c = convs.data.value?.find((x) => x.id === convId.value)
  if (!c || !reply.value.trim() || sending.value) return
  sending.value = true
  try {
    await api(`/api/workspaces/${wsId.value}/moodle/conversations/${c.id}`, { method: 'POST', body: { text: reply.value.trim() } })
    reply.value = ''
    await openConv(c)
    convs.refetch()
  } finally { sending.value = false }
}
const current = computed(() => convs.data.value?.find((x) => x.id === convId.value) ?? null)
const unreadConvs = computed(() => (convs.data.value ?? []).reduce((n, c) => n + c.unread, 0))
/** Conversation choisie dans le panneau « Messages récents » : ouverte dans l'onglet Messages. */
function openConvId(id: number) {
  tab.value = 'messages'
  const c = convs.data.value?.find((x) => x.id === id)
  if (c) openConv(c)
}

// --- Tableau de bord ---
const board = useMoodleBoard(wsId)
const addOpen = ref(false)
const addItems = computed<MenuItem[]>(() => PANEL_KINDS.map(({ kind }) => ({
  label: t(`moodle.board.kinds.${kind}`), icon: PANEL_ICONS[kind], onSelect: () => board.add(kind),
})))
async function resetBoard() {
  if (!(await confirmDialog({ title: t('moodle.board.resetTitle'), message: t('moodle.board.resetHint'), confirmLabel: t('moodle.board.reset') }))) return
  await board.reset()
}

// --- Actions ---
const syncing = ref(false)
async function syncCalendar() {
  syncing.value = true
  try {
    const r = await api<{ events: number }>(`/api/workspaces/${wsId.value}/moodle/calendar/sync`, { method: 'POST' })
    toast.success(t('moodle.calendarSynced', { count: r.events }))
    invalidate()
  } finally { syncing.value = false }
}
async function toggleNotify() {
  await api(`/api/workspaces/${wsId.value}/moodle/preferences`, { method: 'PUT', body: { notify: !conn.value?.notify } })
  invalidate()
}
async function disconnect() {
  if (!(await confirmDialog({ title: t('moodle.disconnectTitle'), message: t('moodle.disconnectHint'), danger: true, confirmLabel: t('moodle.disconnect') }))) return
  await api(`/api/workspaces/${wsId.value}/moodle/connect`, { method: 'DELETE' })
  qc.setQueryData<MoodleOverview>(moodleKeys.overview(wsId.value), (o) => o && { ...o, connection: null })
  for (const k of ['courses?all=true', 'deadlines', 'conversations', 'notifications']) qc.removeQueries({ queryKey: moodleKeys.data(wsId.value, k) })
  convId.value = null
  qc.invalidateQueries({ queryKey: ['calendars'] })
  invalidate()
}
const moreOpen = ref(false)
const moreItems = computed<MenuItem[]>(() => [
  { label: t('moodle.syncCalendar'), icon: RefreshCw, onSelect: syncCalendar },
  { label: conn.value?.notify ? t('moodle.notifyOff') : t('moodle.notifyOn'), icon: conn.value?.notify ? BellOff : Bell, onSelect: toggleNotify },
  ...(board.customized.value ? [{ label: t('moodle.board.reset'), icon: LayoutDashboard, onSelect: resetBoard }] : []),
  ...(overview.value?.can_configure ? [{ label: t('moodle.platformSettings'), icon: Settings, onSelect: () => navigateTo(`/w/${wsId.value}/settings/workspace`) }] : []),
  { divider: true },
  { label: t('moodle.disconnect'), icon: LogOut, danger: true, onSelect: disconnect },
])
const refreshAll = () => invalidate()
watch(tab, (v) => { if (v === 'site' && !frameSrc.value) loadFrame() })

useHead({ title: () => t('moodle.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: conn?.site_name || t('moodle.title'), icon: GraduationCap }]">
      <template v-if="connected">
        <UiTooltip :text="t('moodle.refresh')"><UiButton size="sm" variant="ghost" :icon="RefreshCw" :aria-label="t('moodle.refresh')" @click="refreshAll" /></UiTooltip>
        <UiButton size="sm" :icon="ExternalLink" @click="openSite()">{{ t('moodle.open') }}</UiButton>
        <UiMenu v-model:open="moreOpen" :items="moreItems" placement="bottom-end">
          <UiButton size="sm" variant="ghost" :icon="MoreHorizontal" :aria-label="t('common.more')" />
        </UiMenu>
      </template>
    </AppPageHeader>

    <div v-if="isLoading" class="md-center"><UiSpinner /></div>

    <!-- Pas encore configuré pour l'espace -->
    <div v-else-if="!overview?.url" class="md-center">
      <div class="md-card">
        <GraduationCap :size="28" class="md-hero" />
        <h2>{{ t('moodle.setupTitle') }}</h2>
        <p class="muted">{{ overview?.can_configure ? t('moodle.setupHint') : t('moodle.askAdmin') }}</p>
        <template v-if="overview?.can_configure">
          <UiInput v-model="urlDraft" placeholder="https://moodle.u-bordeaux.fr" @keydown.enter="saveUrl" />
          <UiButton variant="primary" :loading="savingUrl" @click="saveUrl">{{ t('moodle.enable') }}</UiButton>
        </template>
      </div>
    </div>

    <!-- Connexion du compte -->
    <div v-else-if="showConnect" class="md-center">
      <div class="md-card wide">
        <UiButton v-if="reconnecting" class="md-cancel" size="sm" variant="ghost" :icon="ArrowLeft" @click="reconnecting = false">{{ t('common.cancel') }}</UiButton>
        <img v-if="site?.logo" :src="site.logo" alt="" class="md-logo" referrerpolicy="no-referrer" />
        <GraduationCap v-else :size="28" class="md-hero" />
        <h2>{{ t('moodle.connectTitle', { site: site?.site_name || overview.url }) }}</h2>
        <p class="muted">{{ t('moodle.connectHint') }}</p>
        <p v-if="siteError" class="md-err">{{ siteError }}</p>
        <!-- Application de bureau : portail officiel dans une fenêtre, connexion automatique. -->
        <template v-if="portal">
          <UiButton class="md-portal" variant="primary" :icon="LogIn" :disabled="!site" :loading="connecting" @click="portalLogin">{{ t('moodle.portalLogin', { site: site?.site_name || 'Moodle' }) }}</UiButton>
          <p class="md-small">{{ t('moodle.portalHintDesktop') }}</p>
          <div v-if="waiting" class="md-wait"><UiSpinner :size="14" /> <span>{{ t('moodle.waitingDesktop') }}</span></div>
        </template>
        <!-- Navigateur : Moodle renvoie sa clé à « moodlemobile:// », qu'un navigateur ne
             transmet pas ; on la récupère sur la page « Clés de sécurité » de Moodle. -->
        <ol v-else class="md-steps">
          <li>
            <div class="md-step-text"><b>{{ t('moodle.webStep1') }}</b><span>{{ t('moodle.webStep1Hint') }}</span></div>
            <UiButton size="sm" variant="primary" :icon="LogIn" :disabled="!site" @click="portalLogin">{{ t('moodle.portalLogin', { site: site?.site_name || 'Moodle' }) }}</UiButton>
          </li>
          <li>
            <div class="md-step-text"><b>{{ t('moodle.webStep2') }}</b><span>{{ t('moodle.webStep2Hint') }}</span></div>
            <UiButton size="sm" :icon="KeyRound" :disabled="!site" @click="openTokens">{{ t('moodle.tokensPage') }}</UiButton>
          </li>
          <li>
            <div class="md-step-text"><b>{{ t('moodle.webStep3') }}</b><span>{{ t('moodle.webStep3Hint') }}</span></div>
            <div class="md-paste">
              <UiInput v-model="form.token" :placeholder="t('moodle.tokenPlaceholder')" autocomplete="off" @keydown.enter="connect({ token: form.token })" />
              <UiButton variant="primary" :loading="connecting" :disabled="!form.token.trim()" @click="connect({ token: form.token })">{{ t('moodle.connect') }}</UiButton>
            </div>
          </li>
        </ol>
        <p v-if="connectError" class="md-err">{{ connectError }}</p>

        <button class="md-other" :aria-expanded="otherWays" @click="otherWays = !otherWays">{{ t('moodle.otherWays') }}</button>
        <template v-if="otherWays">
          <UiSegmented v-if="portal" v-model="method" size="md" :options="[{ value: 'token', label: t('moodle.methods.token') }, { value: 'password', label: t('moodle.methods.password') }]" />
          <template v-if="method === 'password' || !portal">
            <UiInput v-model="form.username" :placeholder="t('moodle.username')" autocomplete="username" />
            <UiInput v-model="form.password" type="password" :placeholder="t('moodle.password')" autocomplete="current-password" @keydown.enter="connectOther" />
            <p class="md-small">{{ t('moodle.passwordHint') }}</p>
          </template>
          <template v-else>
            <p class="md-small">{{ t('moodle.tokenHint') }} <a v-if="site" :href="site.tokens_url" target="_blank" rel="noopener">{{ t('moodle.tokensPage') }}</a></p>
            <UiInput v-model="form.token" :placeholder="t('moodle.tokenPlaceholder')" autocomplete="off" @keydown.enter="connectOther" />
          </template>
          <UiButton :icon="KeyRound" :loading="connecting" :disabled="usesPassword ? !form.username || !form.password : !form.token.trim()" @click="connectOther">{{ t('moodle.connect') }}</UiButton>
        </template>
        <p class="md-small">{{ t('moodle.privacy') }}</p>
      </div>
    </div>

    <!-- Connecté -->
    <div v-else class="md-body">
      <div v-if="tokenBroken" class="md-banner">
        <AlertTriangle :size="14" /><span>{{ t('moodle.tokenExpired') }}</span>
        <UiButton size="xs" variant="ghost" @click="disconnect">{{ t('moodle.reconnect') }}</UiButton>
      </div>
      <div v-else-if="!conn!.autologin" class="md-banner info">
        <LogIn :size="14" /><span>{{ t('moodle.autologinHint') }}</span>
        <UiButton size="xs" variant="ghost" @click="reconnecting = true">{{ t('moodle.autologinAction') }}</UiButton>
      </div>
      <MoodleCourse v-if="courseId" :ws="wsId" :course-id="courseId" :course="courses.byId(courseId)" @back="openCourse(null)" @open="openSite" />
      <template v-else>
      <div class="md-tabs">
        <UiSegmented v-model="tab" size="md" :options="[
          { value: 'overview', label: t('moodle.tabs.overview') },
          { value: 'messages', label: unreadConvs ? `${t('moodle.tabs.messages')} · ${unreadConvs}` : t('moodle.tabs.messages') },
          { value: 'notifications', label: notifs.data.value?.unread ? `${t('moodle.tabs.notifications')} · ${notifs.data.value.unread}` : t('moodle.tabs.notifications') },
          ...(site?.frameable && !isDesktop ? [{ value: 'site' as const, label: t('moodle.tabs.site') }] : []),
        ]" />
        <span class="md-who muted">{{ conn!.full_name }}</span>
        <UiMenu v-if="tab === 'overview'" v-model:open="addOpen" :items="addItems" placement="bottom-end">
          <UiButton size="sm" :icon="Plus">{{ t('moodle.board.add') }}</UiButton>
        </UiMenu>
      </div>

      <MoodleBoard v-if="tab === 'overview'" :ws="wsId" @open="openSite" @course="openCourse" @conversation="openConvId" @task="addTask" />

      <div v-else-if="tab === 'messages'" class="md-msgs" :class="{ reading: convId != null }">
        <div class="cv-list">
          <div v-if="convs.isLoading.value" class="md-skel"><div v-for="i in 4" :key="i" class="skeleton" style="height: 52px; margin: 4px 0" /></div>
          <p v-else-if="!convs.data.value?.length" class="muted md-empty">{{ t('moodle.noMessages') }}</p>
          <button v-for="c in convs.data.value" :key="c.id" class="cv" :class="{ on: c.id === convId, unread: c.unread > 0 }" @click="openConv(c)">
            <UiAvatar :name="c.name" :size="28" />
            <span class="cv-body">
              <span class="cv-top"><b class="truncate">{{ c.name }}</b><time v-if="c.last_time">{{ relativeTime(c.last_time) }}</time></span>
              <span class="cv-last truncate">{{ c.last_from_me ? `${t('moodle.you')} : ` : '' }}{{ c.last_text }}</span>
            </span>
            <span v-if="c.unread" class="cv-badge">{{ c.unread }}</span>
          </button>
        </div>
        <div class="cv-thread">
          <div v-if="convId == null" class="md-center muted"><MessageSquare :size="24" /><span>{{ t('moodle.pickConversation') }}</span></div>
          <template v-else>
            <header class="th-head">
              <UiButton v-if="isMobile" size="sm" variant="ghost" :icon="ArrowLeft" :aria-label="t('common.close')" @click="convId = null" />
              <b>{{ current?.name }}</b>
            </header>
            <div ref="threadEl" class="th-msgs">
              <div v-if="threadLoading && !thread.length" class="md-center"><UiSpinner /></div>
              <div v-for="m in thread" :key="m.id" class="bubble" :class="{ mine: m.mine }">
                <div v-if="!m.mine" class="b-from">{{ m.from }}</div>
                <!-- eslint-disable-next-line vue/no-v-html : nettoyé côté serveur -->
                <div class="b-text" v-html="m.html" />
                <time class="b-time">{{ dayFmt(m.time) }} {{ timeFmt(m.time) }}</time>
              </div>
            </div>
            <div class="th-reply">
              <textarea v-model="reply" rows="2" :placeholder="t('moodle.replyPlaceholder')" @keydown.enter.exact.prevent="send" />
              <UiButton variant="primary" size="sm" :icon="Send" :loading="sending" :disabled="!reply.trim()" :aria-label="t('mail.send')" @click="send" />
            </div>
          </template>
        </div>
      </div>

      <div v-else-if="tab === 'site'" class="md-site">
        <iframe v-if="frameSrc" :src="frameSrc" :title="conn!.site_name || 'Moodle'" referrerpolicy="no-referrer" />
        <div v-else class="md-center"><UiSpinner /></div>
      </div>

      <div v-else class="md-notifs">
        <p v-if="!notifs.data.value?.notifications.length" class="muted md-empty">{{ t('moodle.noNotifications') }}</p>
        <component :is="n.url ? 'a' : 'div'" v-for="n in notifs.data.value?.notifications" :key="n.id" class="nt" :class="{ unread: !n.read }" :href="n.url || undefined" target="_blank" rel="noopener" @click="n.url && ($event.preventDefault(), openSite(n.url))">
          <Bell :size="14" class="nt-ic" />
          <span class="nt-body"><b>{{ n.subject }}</b><span v-if="n.text && n.text !== n.subject" class="nt-text">{{ n.text }}</span></span>
          <time>{{ relativeTime(n.time) }}</time>
        </component>
      </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.page { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.md-center { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; padding: 24px; }
.md-card { width: 100%; max-width: 380px; display: flex; flex-direction: column; gap: 10px; padding: 24px; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); text-align: center; }
.md-card.wide { max-width: 480px; text-align: left; }
.md-card h2 { margin: 0; font-size: 17px; }
.md-card p { margin: 0; }
.md-hero { color: #f98012; align-self: center; }
.md-card.wide .md-hero, .md-logo { align-self: flex-start; }
.md-portal { align-self: stretch; justify-content: center; height: 38px; }
.md-wait { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-radius: var(--r-md); background: var(--bg-hover); font-size: var(--fs-sm); color: var(--text-2); }
.md-other { align-self: flex-start; font-size: var(--fs-sm); color: var(--text-3); text-decoration: underline; }
.md-other:hover { color: var(--text); }
.md-site { flex: 1; min-height: 520px; display: flex; border-radius: var(--r-lg); overflow: hidden; box-shadow: var(--shadow-card); background: #fff; }
.md-site iframe { flex: 1; border: 0; width: 100%; }
.md-logo { max-height: 36px; max-width: 180px; object-fit: contain; }
.muted { color: var(--text-3); }
.md-small { font-size: var(--fs-sm); color: var(--text-3); margin: 0; }
.md-err { color: var(--red); font-size: var(--fs-sm); margin: 0; }
.md-steps { margin: 4px 0 0; padding-left: 20px; display: flex; flex-direction: column; gap: 12px; font-size: var(--fs-sm); color: var(--text-2); }
.md-steps li { padding-left: 4px; }
.md-steps li::marker { color: var(--accent-text); font-weight: 600; }
.md-step-text { display: flex; flex-direction: column; gap: 2px; margin-bottom: 6px; }
.md-step-text b { color: var(--text); font-weight: 600; }
.md-paste { display: flex; gap: 6px; }
.md-paste > :first-child { flex: 1; }
.md-body { flex: 1; min-height: 0; display: flex; flex-direction: column; padding: 12px 20px 20px; gap: 12px; overflow-y: auto; }
.md-banner { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-radius: var(--r-md); background: color-mix(in srgb, var(--red) 12%, transparent); color: var(--red); font-size: var(--fs-sm); }
.md-banner span { flex: 1; }
.md-banner.info { background: color-mix(in srgb, var(--accent) 10%, transparent); color: var(--text-2); }
.md-cancel { align-self: flex-start; }
.md-tabs { display: flex; align-items: center; gap: 12px; }
.md-who { margin-left: auto; font-size: var(--fs-sm); }
.md-empty { padding: 12px 0; margin: 0; font-size: var(--fs-sm); }
.md-msgs { flex: 1; min-height: 420px; display: grid; grid-template-columns: 300px 1fr; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); overflow: hidden; }
.cv-list { border-right: 1px solid var(--border); overflow-y: auto; padding: 6px; }
.cv { width: 100%; display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: var(--r-md); text-align: left; }
.cv:hover { background: var(--bg-hover); }
.cv.on { background: var(--bg-active); }
.cv-body { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.cv-top { display: flex; align-items: center; gap: 6px; }
.cv-top b { flex: 1; min-width: 0; font-weight: 500; }
.cv.unread .cv-top b { font-weight: 700; }
.cv-top time { font-size: var(--fs-xs); color: var(--text-3); flex: none; }
.cv-last { font-size: var(--fs-sm); color: var(--text-3); }
.cv-badge { min-width: 18px; height: 18px; padding: 0 5px; border-radius: 9px; background: var(--accent); color: #fff; font-size: 11px; font-weight: 600; display: grid; place-items: center; }
.cv-thread { min-width: 0; display: flex; flex-direction: column; min-height: 0; }
.th-head { display: flex; align-items: center; gap: 6px; padding: 10px 14px; border-bottom: 1px solid var(--border); }
.th-msgs { flex: 1; min-height: 0; overflow-y: auto; padding: 14px; display: flex; flex-direction: column; gap: 8px; }
.bubble { max-width: 75%; align-self: flex-start; padding: 7px 11px; border-radius: 12px; background: var(--bg-hover); }
.bubble.mine { align-self: flex-end; background: var(--accent-soft); }
.b-from { font-size: var(--fs-xs); font-weight: 600; color: var(--text-2); margin-bottom: 2px; }
.b-text :deep(p) { margin: 0; }
.b-time { display: block; font-size: 10.5px; color: var(--text-3); margin-top: 2px; text-align: right; }
.th-reply { display: flex; align-items: flex-end; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--border); }
.th-reply textarea { flex: 1; resize: none; padding: 7px 10px; border-radius: var(--r-md); border: 1px solid var(--border-strong); background: var(--bg-input); color: var(--text); font: inherit; }
.md-notifs { display: flex; flex-direction: column; gap: 2px; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); padding: 6px; }
.nt { display: flex; align-items: flex-start; gap: 10px; padding: 9px 10px; border-radius: var(--r-md); color: var(--text); text-decoration: none; }
.nt:hover { background: var(--bg-hover); }
.nt-ic { color: var(--text-3); margin-top: 2px; flex: none; }
.nt.unread .nt-ic { color: var(--accent); }
.nt-body { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.nt-body b { font-weight: 500; }
.nt.unread .nt-body b { font-weight: 600; }
.nt-text { font-size: var(--fs-sm); color: var(--text-3); }
.nt time { font-size: var(--fs-xs); color: var(--text-3); flex: none; }
@media (max-width: 767px) {
  .md-body { padding: 10px 12px 16px; }
  .md-msgs { grid-template-columns: 1fr; }
  .md-msgs.reading .cv-list, .md-msgs:not(.reading) .cv-thread { display: none; }
  .md-who { display: none; }
}
</style>
