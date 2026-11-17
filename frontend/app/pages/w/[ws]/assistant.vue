<script setup lang="ts">
import { Bot, PanelLeft, PenLine, FilePlus2, Wrench, GraduationCap, Undo2, Paperclip } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { refDebounced } from '@vueuse/core'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { toast } from '~/composables/useToast'
import { confirmDialog, promptDialog } from '~/composables/useConfirm'
import { useStudyActions } from '~/composables/useStudy'
import { aiOpenItem } from '~/composables/useAiOpenItem'
import {
  aiApi, aiInbox, streamMessage, aiErrorMessage,
  type AiAttachment, type AiConversation, type AiMessage, type AiGrant, type AiStudyGrant, type AiSuggestion, type AiGrantNeed, type AiStreamEvent,
} from '~/composables/useAi'
import { needKey, type EndedGrant } from '~/utils/grantState'
import { draggingPage } from '~/composables/useMovePage'
import { dropIntent, type DropIntent } from '~/utils/chatFiles'
import { AI_PROVIDER_CATALOG } from '~/utils/aiProviders'

/**
 * Assistant : page de l'espace à deux colonnes (conversations à gauche, discussion à droite). Lecture seule
 * tant qu'une autorisation n'est pas accordée dans la discussion ; le serveur vérifie tout (ADR 0010).
 */
definePageMeta({ layout: 'workspace' })
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const qc = useQueryClient()
const { wsId, pages } = useWorkspace()
useHead({ title: () => t('ai.page.title') })


// --- Connexions et liste ----------------------------------------------------------------------
const { data: connections } = useQuery({ queryKey: ['ai', 'connections'], queryFn: aiApi.connections })
const connectionId = ref('')
watch(connections, (c) => { if (c?.length && !c.some((x) => x.id === connectionId.value)) connectionId.value = c[0]!.id }, { immediate: true })

const search = ref('')
const debouncedSearch = refDebounced(search, 250)
const { data: history, isLoading: historyLoading } = useQuery({
  queryKey: computed(() => ['ai', 'conversations', debouncedSearch.value.trim()]),
  queryFn: () => aiApi.conversations(debouncedSearch.value.trim()),
})
const listOpen = ref(false)

// --- Conversation courante ------------------------------------------------------------------------
const conv = ref<AiConversation | null>(null)
const messages = ref<AiMessage[]>([])
const grants = ref<AiGrant[]>([])
const studyGrants = ref<AiStudyGrant[]>([])
const endedGrants = ref<EndedGrant[]>([])
const suggestions = ref<AiSuggestion[]>([])
const draft = ref('')
const attachments = ref<AiAttachment[]>([])
const busy = ref(false)
const streaming = ref('')
const toolLog = ref<string[]>([])
let abort: AbortController | null = null
let retryAfter = false
const scroller = ref<HTMLElement | null>(null)
function scrollDown() { nextTick(() => { if (scroller.value) scroller.value.scrollTop = scroller.value.scrollHeight }) }

function defaultAttachments(): AiAttachment[] {
  if (aiInbox.value.length) { const a = aiInbox.value; aiInbox.value = []; return a }
  return aiOpenItem.value ? [aiOpenItem.value] : []
}

async function loadConversation(id: string) {
  const r = await aiApi.conversation(id)
  conv.value = r.conversation
  messages.value = r.messages
  grants.value = r.grants
  studyGrants.value = r.study_grants ?? []
  endedGrants.value = r.ended_grants ?? []
  suggestions.value = r.suggestions
  if (r.conversation.connection_id) connectionId.value = r.conversation.connection_id
  scrollDown()
}
function reset() {
  conv.value = null; messages.value = []; grants.value = []; studyGrants.value = []; endedGrants.value = []; suggestions.value = []
  draft.value = ''; streaming.value = ''
  attachments.value = defaultAttachments()
}
const routeConv = computed(() => (typeof route.query.c === 'string' ? route.query.c : ''))
watch(routeConv, async (id) => {
  if (id === conv.value?.id) return
  if (!id) return reset()
  attachments.value = []
  try { await loadConversation(id) } catch { router.replace({ query: {} }); reset() }
}, { immediate: true })
// « Demander à l'IA » depuis un menu alors que la page est déjà ouverte : nouvelle conversation avec l'élément.
watch(aiInbox, (a) => { if (a.length) { if (routeConv.value) router.push({ query: {} }); else reset() } })

function openNew() { listOpen.value = false; if (routeConv.value) router.push({ query: {} }); else reset() }
function select(id: string) { listOpen.value = false; router.push({ query: { c: id } }) }

async function ensureConversation(): Promise<AiConversation> {
  if (conv.value) {
    if (conv.value.connection_id !== connectionId.value) {
      await api(`/api/ai/conversations/${conv.value.id}`, { method: 'PATCH', body: { connection_id: connectionId.value } })
      conv.value = { ...conv.value, connection_id: connectionId.value }
    }
    return conv.value
  }
  const c = await api<AiConversation>('/api/ai/conversations', { method: 'POST', body: { connection_id: connectionId.value, context_kind: 'none' } })
  conv.value = c
  router.replace({ query: { c: c.id } })
  qc.invalidateQueries({ queryKey: ['ai', 'conversations'] })
  return c
}

function pageChanged() { qc.invalidateQueries({ predicate: (q) => q.queryKey[0] === 'ws' && q.queryKey[2] === 'pages' }) }
function onEvent(e: AiStreamEvent) {
  if (e.type === 'text') { streaming.value += e.delta; scrollDown() }
  else if (e.type === 'tool') toolLog.value.push(`${e.name}${e.ok ? '' : ' ✕'}`)
  else if (e.type === 'suggestion') { suggestions.value.push(e.suggestion); scrollDown() }
  else if (e.type === 'page_changed') pageChanged()
  else if (e.type === 'grant_needed') { messages.value.push({ id: `live-${Date.now()}`, role: 'tool', content: '', data: { grant_needed: e.grant } }); scrollDown() }
  else if (e.type === 'title') { if (conv.value) conv.value = { ...conv.value, title: e.title }; qc.invalidateQueries({ queryKey: ['ai', 'conversations'] }) }
  else if (e.type === 'error') toast.error(aiErrorMessage(e.message, t))
}

async function send(override?: string) {
  const text = (override ?? draft.value).trim()
  if (!text || busy.value || !connectionId.value) return
  busy.value = true
  try {
    const c = await ensureConversation()
    const atts = override ? [] : attachments.value
    messages.value.push({ id: `tmp-${Date.now()}`, role: 'user', content: text, data: atts.length ? { attachments: atts.map(({ kind, id, title }) => ({ kind, id, title })) } : undefined })
    if (!override) { draft.value = ''; attachments.value = [] }
    streaming.value = ''
    toolLog.value = []
    scrollDown()
    abort = new AbortController()
    await streamMessage(c.id, text, atts, onEvent, abort.signal, wsId.value)
  } catch (e: any) {
    if (e?.name !== 'AbortError') toast.error(errorMessage(e))
  } finally {
    busy.value = false
    abort = null
    if (conv.value) await loadConversation(conv.value.id).catch(() => {})
    streaming.value = ''
    qc.invalidateQueries({ queryKey: ['ai', 'conversations'] })
    if (retryAfter) { retryAfter = false; void send(t('ai.page.retry')) }
  }
}
function stop() { abort?.abort() }

// --- Dépôt sur la discussion : fichiers de l'ordinateur ou page / entraînement glissé depuis l'arbre ---
const composer = ref<{ addFiles: (f: File[]) => Promise<void>; addPage: (d: NonNullable<typeof draggingPage.value>) => void } | null>(null)
const dropping = ref<DropIntent>('none')
let dragDepth = 0
const canAttach = () => !!connections.value?.length
function onDragEnter(e: DragEvent) {
  const intent = dropIntent(e.dataTransfer?.types)
  if (intent === 'none' || !canAttach()) return
  dragDepth++
  dropping.value = intent
}
function onDragOver(e: DragEvent) {
  const intent = dropIntent(e.dataTransfer?.types)
  if (intent === 'none' || !canAttach()) return
  e.preventDefault()
  // La page glissée de l'arbre n'autorise que « move » : on ne la déplace pas, on la joint.
  if (e.dataTransfer) e.dataTransfer.dropEffect = intent === 'files' ? 'copy' : 'move'
}
function onDragLeave() {
  dragDepth = Math.max(0, dragDepth - 1)
  if (!dragDepth) dropping.value = 'none'
}
function onDrop(e: DragEvent) {
  const intent = dropIntent(e.dataTransfer?.types)
  dragDepth = 0
  dropping.value = 'none'
  if (intent === 'none' || !canAttach()) return
  e.preventDefault()
  if (intent === 'files') void composer.value?.addFiles(Array.from(e.dataTransfer?.files ?? []))
  else if (draggingPage.value && e.dataTransfer?.getData('application/x-looty-page') === draggingPage.value.id) composer.value?.addPage(draggingPage.value)
}
const providerName = computed(() => {
  const c = connections.value?.find((x) => x.id === (conv.value?.connection_id ?? connectionId.value))
  return c ? AI_PROVIDER_CATALOG.find((p) => p.value === c.provider)?.name ?? c.provider : ''
})

// --- Liste : renommer, supprimer ----------------------------------------------------------------
async function rename(c: AiConversation) {
  const title = await promptDialog({ title: t('ai.page.renameTitle'), initial: c.title, confirmLabel: t('common.rename') })
  if (!title) return
  try {
    await api(`/api/ai/conversations/${c.id}`, { method: 'PATCH', body: { title } })
    if (conv.value?.id === c.id) conv.value = { ...conv.value, title }
    qc.invalidateQueries({ queryKey: ['ai', 'conversations'] })
  } catch (e) { toast.error(errorMessage(e)) }
}
async function remove(c: AiConversation) {
  if (!(await confirmDialog({ title: t('ai.page.deleteTitle', { title: c.title || t('ai.chat.untitled') }), message: t('ai.page.deleteMessage'), confirmLabel: t('common.delete'), danger: true }))) return
  try {
    await api(`/api/ai/conversations/${c.id}`, { method: 'DELETE' })
    qc.invalidateQueries({ queryKey: ['ai', 'conversations'] })
    if (conv.value?.id === c.id) openNew()
  } catch (e) { toast.error(errorMessage(e)) }
}

// --- Autorisations, suggestions, annulations dans la discussion ---------------------------------
async function refreshGrants() {
  if (!conv.value) return
  const r = await aiApi.conversation(conv.value.id)
  grants.value = r.grants
  studyGrants.value = r.study_grants ?? []
  endedGrants.value = r.ended_grants ?? []
}
function onGranted() { if (busy.value) retryAfter = true; else void send(t('ai.page.retry')) }

async function undoPage(revId: string, undo = false) {
  try {
    const r = await api<{ page_id: string; undo_revision_id: string | null }>(`/api/ai/page-revisions/${revId}/restore`, { method: 'POST' })
    if (!undo) restored.value = new Set(restored.value).add(revId)
    pageChanged()
    toast.success(t(undo ? 'ai.page.undoUndone' : 'ai.page.pageChangedUndone'), undo || !r.undo_revision_id ? {} : { duration: 10000, action: { label: t('ai.page.undo'), run: () => { void undoPage(r.undo_revision_id!, true) } } })
  } catch (e) { toast.error(errorMessage(e)) }
}
const deciding = ref(new Set<string>())
async function decide(s: AiSuggestion, accept: boolean) {
  if (deciding.value.has(s.id)) return
  deciding.value = new Set(deciding.value).add(s.id)
  try {
    const r = await api<{ page_id: string; revision_id: string | null } | undefined>(`/api/ai/suggestions/${s.id}/${accept ? 'accept' : 'reject'}`, { method: 'POST' })
    suggestions.value = suggestions.value.filter((x) => x.id !== s.id)
    if (accept) {
      pageChanged()
      toast.success(t('ai.chat.applied'), r?.revision_id ? { duration: 10000, action: { label: t('ai.chat.studyRestore'), run: () => { void undoPage(r.revision_id!) } } } : {})
    }
  } catch (e) { toast.error(errorMessage(e)) } finally {
    const next = new Set(deciding.value)
    next.delete(s.id)
    deciding.value = next
  }
}
const studyActions = useStudyActions()
const restored = ref(new Set<string>())

// « Créer une page avec cette réponse » : page personnelle de l'espace courant, puis ouverture.
const toPageBusy = ref('')
async function toPage(messageId: string) {
  if (toPageBusy.value) return
  toPageBusy.value = messageId
  try {
    const r = await api<{ page_id: string; workspace_id: string }>(`/api/ai/messages/${messageId}/to-page`, { method: 'POST', body: { workspace_id: wsId.value, personal: true } })
    pageChanged()
    await router.push(links.doc(r.workspace_id, r.page_id))
  } catch (e) { toast.error(errorMessage(e)) } finally { toPageBusy.value = '' }
}
async function restoreStudy(setId: string, revId: string, undo = false) {
  try {
    const undoId = await studyActions.restore(setId, revId)
    if (!undo) restored.value = new Set(restored.value).add(revId)
    toast.success(t(undo ? 'study.restoreUndone' : 'study.restored'), undo ? {} : { duration: 10000, action: { label: t('study.undo'), run: () => { void restoreStudy(setId, undoId, true) } } })
  } catch (e: any) { toast.error(errorMessage(e)) }
}

// --- Affichage -------------------------------------------------------------------------------------
const visible = computed(() => messages.value.filter((m) => (m.role !== 'tool' && (m.content || m.role === 'user')) || (m.role === 'tool' && (m.data?.study_set_id || m.data?.page_revision_id || m.data?.grant_needed))))
/** Une seule carte par cible : la plus récente. */
const lastNeed = computed(() => {
  const last = new Map<string, string>()
  for (const m of messages.value) if (m.role === 'tool' && m.data?.grant_needed) last.set(needKey(m.data.grant_needed), m.id)
  return last
})
const showNeed = (m: AiMessage) => lastNeed.value.get(needKey(m.data.grant_needed)) === m.id
const pageTitle = (id: string) => pages.data.value?.find((p) => p.id === id)?.title
const pageLink = (id: string) => (pages.data.value?.some((p) => p.id === id) ? links.doc(wsId.value, id) : null)
const empty = computed(() => !visible.value.length && !streaming.value)
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('ai.page.title'), icon: Bot }]">
      <UiButton class="as-toggle" size="sm" variant="ghost" :icon="PanelLeft" :aria-label="t('ai.page.conversations')" @click="listOpen = !listOpen" />
    </AppPageHeader>
    <div v-if="connections && !connections.length" class="as-solo">
      <UiEmpty :icon="Bot" :title="t('ai.chat.noConnection')">
        <UiButton variant="primary" :to="`/w/${wsId}/settings/ai`">{{ t('ai.chat.setUp') }}</UiButton>
      </UiEmpty>
    </div>
    <div v-else class="as" :class="{ 'list-open': listOpen }">
      <aside class="as-side">
        <AiConversationList v-model:query="search" :conversations="history ?? []" :active-id="conv?.id ?? null" :loading="historyLoading" @select="select" @new="openNew" @rename="rename" @remove="remove" />
      </aside>
      <section class="as-main" :class="{ 'as-dropping': dropping !== 'none' }" :aria-label="conv?.title || t('ai.page.title')" @dragenter="onDragEnter" @dragover="onDragOver" @dragleave="onDragLeave" @drop="onDrop">
        <div v-if="dropping !== 'none'" class="as-drop" aria-hidden="true">
          <Paperclip :size="22" />
          <span>{{ t(dropping === 'files' ? 'ai.files.dropFiles' : 'ai.files.dropPage') }}</span>
        </div>
        <div ref="scroller" class="as-thread">
          <div class="as-col">
            <div v-if="empty" class="as-intro">
              <Bot :size="28" :stroke-width="1.5" />
              <h2>{{ t('ai.page.title') }}</h2>
              <p>{{ t('ai.page.intro') }}</p>
              <p class="as-sub">{{ t('ai.page.subtitle') }}</p>
            </div>
            <template v-for="m in visible" :key="m.id">
              <template v-if="m.role === 'tool'">
                <AiGrantCard v-if="m.data?.grant_needed && showNeed(m) && conv" :need="m.data.grant_needed" :conversation-id="conv.id" :grants="grants" :study-grants="studyGrants" :ended="endedGrants" :card-at="m.created_at" @changed="refreshGrants" @granted="onGranted" />
                <div v-if="m.data?.study_set_id" class="as-row">
                  <NuxtLink :to="`/study/${m.data.study_set_id}?mode=edit`" class="msg as-link"><GraduationCap :size="13" /> {{ t('ai.chat.studyLink') }}</NuxtLink>
                  <UiButton v-if="m.data.study_revision_id" size="sm" variant="ghost" :icon="Undo2" :disabled="restored.has(m.data.study_revision_id)" @click="restoreStudy(m.data.study_set_id, m.data.study_revision_id)">{{ t('ai.chat.studyRestore') }}</UiButton>
                </div>
                <div v-if="m.data?.page_revision_id" class="as-row">
                  <component :is="pageLink(m.data.page_id) ? 'NuxtLink' : 'span'" :to="pageLink(m.data.page_id) ?? undefined" class="msg as-link"><PenLine :size="13" /> {{ t('ai.page.pageChanged') }}</component>
                  <UiButton size="sm" variant="ghost" :icon="Undo2" :disabled="restored.has(m.data.page_revision_id)" @click="undoPage(m.data.page_revision_id)">{{ t('ai.chat.studyRestore') }}</UiButton>
                </div>
              </template>
              <div v-else-if="m.role === 'assistant'" class="as-answer">
                <RichMarkdown class="msg assistant md" :src="m.content" />
                <UiButton v-if="!String(m.id).startsWith('live-')" class="as-topage" size="xs" variant="ghost" :icon="FilePlus2" :loading="toPageBusy === m.id" @click="toPage(m.id)">{{ t('ai.chat.toPage') }}</UiButton>
              </div>
              <div v-else class="msg user">
                <div v-if="m.data?.attachments?.length" class="as-att">
                  <AiChip v-for="a in m.data.attachments" :key="a.id" class="as-chip" :kind="a.kind" :title="a.title" :expires-at="a.expires_at" :provider="providerName" />
                </div>
                {{ m.content }}
              </div>
            </template>
            <RichMarkdown v-if="streaming" class="msg assistant md" :src="streaming" />
            <div v-if="busy && toolLog.length" class="as-tools"><Wrench :size="11" /> {{ toolLog.join(' · ') }}</div>
            <AiSuggestionCard
              v-for="s in suggestions" :key="s.id" :suggestion="s" :page-title="pageTitle(s.page_id)" :page-href="pageLink(s.page_id)" :busy="deciding.has(s.id)"
              @accept="decide(s, true)" @reject="decide(s, false)"
            />
          </div>
        </div>
        <div class="as-compose">
          <div class="as-col">
            <AiComposer ref="composer" v-model:draft="draft" :dropping="dropping" v-model:attachments="attachments" v-model:connection-id="connectionId" :busy="busy" :connections="connections ?? []" @send="send()" @stop="stop" />
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.as-solo { flex: 1; display: grid; place-items: center; }
.as { flex: 1; min-height: 0; display: grid; grid-template-columns: 280px minmax(0, 1fr); }
.as-side { min-height: 0; background: var(--bg-sidebar, transparent); }
.as-main { min-height: 0; display: flex; flex-direction: column; }
.as-thread { flex: 1; min-height: 0; overflow-y: auto; padding: 16px 20px; }
.as-col { max-width: 760px; margin: 0 auto; width: 100%; display: flex; flex-direction: column; gap: 10px; }
.as-compose { padding: 0 20px 16px; }
.as-intro { display: flex; flex-direction: column; align-items: center; text-align: center; gap: 6px; color: var(--text-3); padding: 48px 16px 16px; }
.as-intro h2 { margin: 4px 0 0; color: var(--text); font-size: var(--fs-lg); }
.as-intro p { margin: 0; max-width: 460px; line-height: 1.5; }
.as-sub { font-size: var(--fs-sm); color: var(--text-4); }
.msg { white-space: pre-wrap; word-break: break-word; padding: 8px 12px; border-radius: var(--r-md); line-height: 1.55; max-width: 92%; }
.msg.md { white-space: normal; }
.as-answer { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; }
.as-topage { opacity: 0.7; }
.as-answer:hover .as-topage, .as-topage:focus-visible { opacity: 1; }
.msg.user { align-self: flex-end; background: var(--accent-soft); }
.msg.assistant { align-self: flex-start; background: var(--bg-hover); }
.as-att { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 4px; }
.as-chip { --chip-bg: var(--bg-elevated); --chip-color: var(--text-2); }
.as-main { position: relative; }
.as-drop { position: absolute; inset: 8px; z-index: 5; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; border: 2px dashed var(--border-focus); border-radius: var(--r-lg); background: color-mix(in srgb, var(--bg) 82%, transparent); color: var(--text-2); pointer-events: none; }
.as-row { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.as-link { align-self: flex-start; display: inline-flex; align-items: center; gap: 6px; background: var(--accent-soft); color: var(--text); text-decoration: none; }
.as-tools { display: flex; align-items: center; gap: 6px; color: var(--text-4); font-size: var(--fs-xs); }
.as-toggle { display: none; }
@media (max-width: 800px) {
  .as { grid-template-columns: minmax(0, 1fr); }
  .as-side { display: none; }
  .as.list-open .as-side { display: block; }
  .as.list-open .as-main { display: none; }
  .as-toggle { display: inline-flex; }
}
</style>
