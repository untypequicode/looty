<script setup lang="ts">
import {
  GraduationCap, ExternalLink, MoreHorizontal, RefreshCw, LogOut, Bell, BellOff, Settings, ListPlus, AlertTriangle, ArrowLeft, Send,
  Clock, BookOpen, MessageSquare, Star, KeyRound, LogIn,
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
import {
  useMoodle, useMoodleData, useMoodleInvalidate, moodleKeys,
  type MoodleOverview, type MoodleSite, type MoodleCourse, type MoodleDeadline, type MoodleConversation, type MoodleMessage, type MoodleNotification,
} from '~/composables/useMoodle'

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

// --- Connexion ---
const site = ref<MoodleSite | null>(null)
const siteError = ref('')
watch(() => [overview.value?.url, connected.value] as const, async ([url]) => {
  site.value = null
  siteError.value = ''
  if (!url || connected.value) return
  try { site.value = await api<MoodleSite>(`/api/workspaces/${wsId.value}/moodle/site`, { silent: true }) } catch (e) { siteError.value = errorMessage(e) }
}, { immediate: true })
const method = ref<'sso' | 'password' | 'token'>('sso')
watch(site, (s) => { if (s) method.value = s.sso ? 'sso' : 'password' })
const form = reactive({ token: '', username: '', password: '' })
const connecting = ref(false)
const connectError = ref('')
async function connect() {
  connecting.value = true
  connectError.value = ''
  try {
    const body = method.value === 'password' ? { username: form.username, password: form.password } : { token: form.token }
    const r = await api<{ full_name: string; calendar: boolean }>(`/api/workspaces/${wsId.value}/moodle/connect`, { method: 'POST', body, silent: true })
    toast.success(t('moodle.connected', { name: r.full_name }))
    if (!r.calendar) toast(t('moodle.calendarUnavailable'))
    Object.assign(form, { token: '', username: '', password: '' })
    tab.value = 'overview'
    invalidate()
    qc.invalidateQueries({ queryKey: ['calendars'] })
  } catch (e) { connectError.value = errorMessage(e) } finally { connecting.value = false }
}
function openLaunch() {
  if (site.value) window.open(site.value.launch_url, '_blank', 'noopener')
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
const tab = ref<'overview' | 'messages' | 'notifications'>('overview')
const courses = useMoodleData<MoodleCourse[]>(wsId, 'courses', connected)
const deadlines = useMoodleData<MoodleDeadline[]>(wsId, 'deadlines', connected)
const convs = useMoodleData<MoodleConversation[]>(wsId, 'conversations', computed(() => connected.value && tab.value === 'messages'), 60_000)
const notifs = useMoodleData<{ notifications: MoodleNotification[]; unread: number }>(wsId, 'notifications', connected, 120_000)
const loadError = computed(() => courses.error.value ?? deadlines.error.value)
watch(loadError, (e) => { if (e) qc.invalidateQueries({ queryKey: moodleKeys.overview(wsId.value) }) })

const dayFmt = (d: string) => new Date(d).toLocaleDateString(locale.value, { weekday: 'short', day: 'numeric', month: 'short' })
const timeFmt = (d: string) => new Date(d).toLocaleTimeString(locale.value, { hour: '2-digit', minute: '2-digit' })
const soon = (d: string) => new Date(d).getTime() - Date.now() < 2 * 86400_000
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
  for (const k of ['courses', 'deadlines', 'conversations', 'notifications']) qc.removeQueries({ queryKey: moodleKeys.data(wsId.value, k) })
  convId.value = null
  qc.invalidateQueries({ queryKey: ['calendars'] })
  invalidate()
}
const moreOpen = ref(false)
const moreItems = computed<MenuItem[]>(() => [
  { label: t('moodle.syncCalendar'), icon: RefreshCw, onSelect: syncCalendar },
  { label: conn.value?.notify ? t('moodle.notifyOff') : t('moodle.notifyOn'), icon: conn.value?.notify ? BellOff : Bell, onSelect: toggleNotify },
  ...(overview.value?.can_configure ? [{ label: t('moodle.platformSettings'), icon: Settings, onSelect: () => navigateTo(`/w/${wsId.value}/settings/workspace`) }] : []),
  { divider: true },
  { label: t('moodle.disconnect'), icon: LogOut, danger: true, onSelect: disconnect },
])
const refreshAll = () => invalidate()
function openExternal(url: string) { window.open(url, '_blank', 'noopener') }

useHead({ title: () => t('moodle.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: conn?.site_name || t('moodle.title'), icon: GraduationCap }]">
      <template v-if="connected">
        <UiTooltip :text="t('moodle.refresh')"><UiButton size="sm" variant="ghost" :icon="RefreshCw" :aria-label="t('moodle.refresh')" @click="refreshAll" /></UiTooltip>
        <UiButton size="sm" :icon="ExternalLink" @click="openExternal(conn!.base_url)">{{ t('moodle.open') }}</UiButton>
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
    <div v-else-if="!connected" class="md-center">
      <div class="md-card wide">
        <img v-if="site?.logo" :src="site.logo" alt="" class="md-logo" referrerpolicy="no-referrer" />
        <GraduationCap v-else :size="28" class="md-hero" />
        <h2>{{ t('moodle.connectTitle', { site: site?.site_name || overview.url }) }}</h2>
        <p class="muted">{{ t('moodle.connectHint') }}</p>
        <p v-if="siteError" class="md-err">{{ siteError }}</p>
        <UiSegmented v-model="method" size="md" :options="[{ value: 'sso', label: t('moodle.methods.sso') }, { value: 'password', label: t('moodle.methods.password') }, { value: 'token', label: t('moodle.methods.token') }]" />
        <template v-if="method === 'sso'">
          <ol class="md-steps">
            <li>{{ t('moodle.sso1') }} <UiButton size="xs" :icon="LogIn" :disabled="!site" @click="openLaunch">{{ t('moodle.ssoOpen') }}</UiButton></li>
            <li>{{ t('moodle.sso2') }}</li>
            <li>{{ t('moodle.sso3') }}</li>
          </ol>
          <UiInput v-model="form.token" placeholder="moodlemobile://token=…" autocomplete="off" @keydown.enter="connect" />
        </template>
        <template v-else-if="method === 'password'">
          <UiInput v-model="form.username" :placeholder="t('moodle.username')" autocomplete="username" />
          <UiInput v-model="form.password" type="password" :placeholder="t('moodle.password')" autocomplete="current-password" @keydown.enter="connect" />
          <p class="md-small">{{ t('moodle.passwordHint') }}</p>
        </template>
        <template v-else>
          <p class="md-small">{{ t('moodle.tokenHint') }} <a v-if="site" :href="site.tokens_url" target="_blank" rel="noopener">{{ t('moodle.tokensPage') }}</a></p>
          <UiInput v-model="form.token" :placeholder="t('moodle.tokenPlaceholder')" autocomplete="off" @keydown.enter="connect" />
        </template>
        <p v-if="connectError" class="md-err">{{ connectError }}</p>
        <UiButton variant="primary" :icon="KeyRound" :loading="connecting" :disabled="method === 'password' ? !form.username || !form.password : !form.token.trim()" @click="connect">{{ t('moodle.connect') }}</UiButton>
        <p class="md-small">{{ t('moodle.privacy') }}</p>
      </div>
    </div>

    <!-- Connecté -->
    <div v-else class="md-body">
      <div v-if="tokenBroken" class="md-banner">
        <AlertTriangle :size="14" /><span>{{ t('moodle.tokenExpired') }}</span>
        <UiButton size="xs" variant="ghost" @click="disconnect">{{ t('moodle.reconnect') }}</UiButton>
      </div>
      <div class="md-tabs">
        <UiSegmented v-model="tab" size="md" :options="[
          { value: 'overview', label: t('moodle.tabs.overview') },
          { value: 'messages', label: unreadConvs ? `${t('moodle.tabs.messages')} · ${unreadConvs}` : t('moodle.tabs.messages') },
          { value: 'notifications', label: notifs.data.value?.unread ? `${t('moodle.tabs.notifications')} · ${notifs.data.value.unread}` : t('moodle.tabs.notifications') },
        ]" />
        <span class="md-who muted">{{ conn!.full_name }}</span>
      </div>

      <div v-if="tab === 'overview'" class="md-grid">
        <section class="md-sec">
          <h3><Clock :size="15" /> {{ t('moodle.deadlines') }}</h3>
          <div v-if="deadlines.isLoading.value" class="md-skel"><div v-for="i in 3" :key="i" class="skeleton" style="height: 52px; margin: 4px 0" /></div>
          <p v-else-if="deadlines.error.value" class="md-err">{{ errorMessage(deadlines.error.value) }}</p>
          <p v-else-if="!deadlines.data.value?.length" class="muted md-empty">{{ t('moodle.noDeadlines') }}</p>
          <div v-for="d in deadlines.data.value" :key="d.id" class="dl" :class="{ overdue: d.overdue, soon: !d.overdue && soon(d.due) }">
            <div class="dl-date"><b>{{ dayFmt(d.due) }}</b><span>{{ timeFmt(d.due) }}</span></div>
            <div class="dl-body">
              <a :href="d.url" target="_blank" rel="noopener" class="dl-name">{{ d.name }}</a>
              <div class="dl-meta">{{ d.course }} · <span :class="{ 'dl-late': d.overdue }">{{ d.overdue ? t('moodle.overdue') : relativeTime(d.due) }}</span></div>
            </div>
            <UiTooltip :text="t('moodle.addTask')"><UiButton size="xs" variant="ghost" :icon="ListPlus" :aria-label="t('moodle.addTask')" @click="addTask(d)" /></UiTooltip>
          </div>
          <p class="md-small">{{ t('moodle.calendarNote') }}</p>
        </section>
        <section class="md-sec">
          <h3><BookOpen :size="15" /> {{ t('moodle.courses') }}</h3>
          <div v-if="courses.isLoading.value" class="md-skel"><div v-for="i in 4" :key="i" class="skeleton" style="height: 44px; margin: 4px 0" /></div>
          <p v-else-if="courses.error.value" class="md-err">{{ errorMessage(courses.error.value) }}</p>
          <p v-else-if="!courses.data.value?.length" class="muted md-empty">{{ t('moodle.noCourses') }}</p>
          <a v-for="c in courses.data.value" :key="c.id" :href="c.url" target="_blank" rel="noopener" class="crs">
            <span class="crs-badge" :style="{ background: `hsl(${(c.id * 67) % 360} 55% 92%)`, color: `hsl(${(c.id * 67) % 360} 45% 32%)` }">{{ (c.shortname || c.fullname).slice(0, 3).toUpperCase() }}</span>
            <span class="crs-body">
              <span class="crs-name truncate">{{ c.fullname }} <Star v-if="c.favourite" :size="11" class="crs-fav" /></span>
              <span v-if="c.category" class="crs-cat">{{ c.category }}</span>
            </span>
            <span v-if="c.progress != null" class="crs-prog" :title="t('moodle.progress', { n: Math.round(c.progress) })">
              <span class="crs-track"><span class="crs-fill" :style="{ width: `${c.progress}%` }" /></span>
              <small>{{ Math.round(c.progress) }} %</small>
            </span>
          </a>
        </section>
      </div>

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

      <div v-else class="md-notifs">
        <p v-if="!notifs.data.value?.notifications.length" class="muted md-empty">{{ t('moodle.noNotifications') }}</p>
        <component :is="n.url ? 'a' : 'div'" v-for="n in notifs.data.value?.notifications" :key="n.id" class="nt" :class="{ unread: !n.read }" :href="n.url || undefined" target="_blank" rel="noopener">
          <Bell :size="14" class="nt-ic" />
          <span class="nt-body"><b>{{ n.subject }}</b><span v-if="n.text && n.text !== n.subject" class="nt-text">{{ n.text }}</span></span>
          <time>{{ relativeTime(n.time) }}</time>
        </component>
      </div>
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
.md-logo { max-height: 36px; max-width: 180px; object-fit: contain; }
.muted { color: var(--text-3); }
.md-small { font-size: var(--fs-sm); color: var(--text-3); margin: 0; }
.md-err { color: var(--red); font-size: var(--fs-sm); margin: 0; }
.md-steps { margin: 0; padding-left: 20px; display: flex; flex-direction: column; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); }
.md-body { flex: 1; min-height: 0; display: flex; flex-direction: column; padding: 12px 20px 20px; gap: 12px; overflow-y: auto; }
.md-banner { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-radius: var(--r-md); background: color-mix(in srgb, var(--red) 12%, transparent); color: var(--red); font-size: var(--fs-sm); }
.md-banner span { flex: 1; }
.md-tabs { display: flex; align-items: center; gap: 12px; }
.md-who { margin-left: auto; font-size: var(--fs-sm); }
.md-grid { display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr); gap: 16px; align-items: start; }
.md-sec { padding: 12px 14px; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); display: flex; flex-direction: column; gap: 2px; }
.md-sec h3 { margin: 0 0 6px; display: flex; align-items: center; gap: 6px; font-size: var(--fs-md); font-weight: 600; }
.md-empty { padding: 12px 0; margin: 0; font-size: var(--fs-sm); }
.dl { display: flex; align-items: center; gap: 12px; padding: 8px 6px; border-radius: var(--r-md); }
.dl:hover { background: var(--bg-hover); }
.dl-date { display: flex; flex-direction: column; align-items: center; min-width: 64px; font-size: var(--fs-sm); color: var(--text-2); line-height: 1.25; }
.dl-date span { color: var(--text-3); font-size: var(--fs-xs); }
.dl.soon .dl-date b { color: #c26a00; }
.dl.overdue .dl-date b, .dl-late { color: var(--red); }
.dl-body { flex: 1; min-width: 0; }
.dl-name { color: var(--text); font-weight: 500; text-decoration: none; }
.dl-name:hover { text-decoration: underline; }
.dl-meta { font-size: var(--fs-sm); color: var(--text-3); }
.crs { display: flex; align-items: center; gap: 10px; padding: 7px 6px; border-radius: var(--r-md); text-decoration: none; color: var(--text); }
.crs:hover { background: var(--bg-hover); }
.crs-badge { width: 34px; height: 34px; border-radius: var(--r-md); display: grid; place-items: center; font-size: 10.5px; font-weight: 700; flex: none; }
.crs-body { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.crs-name { font-weight: 500; }
.crs-fav { color: #e0a100; fill: currentColor; vertical-align: -1px; }
.crs-cat { font-size: var(--fs-xs); color: var(--text-3); }
.crs-prog { display: flex; align-items: center; gap: 6px; flex: none; }
.crs-prog small { font-size: var(--fs-xs); color: var(--text-3); min-width: 34px; text-align: right; }
.crs-track { width: 64px; height: 5px; border-radius: 3px; background: var(--bg-active); overflow: hidden; }
.crs-fill { display: block; height: 100%; background: #2a78d6; border-radius: 3px; }
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
@media (max-width: 900px) { .md-grid { grid-template-columns: 1fr; } }
@media (max-width: 767px) {
  .md-body { padding: 10px 12px 16px; }
  .md-msgs { grid-template-columns: 1fr; }
  .md-msgs.reading .cv-list, .md-msgs:not(.reading) .cv-thread { display: none; }
  .md-who { display: none; }
}
</style>
