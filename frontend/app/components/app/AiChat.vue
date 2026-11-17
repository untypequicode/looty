<script setup lang="ts">
import { X, Bot, Send, Square, Plus, PenLine, Check, Ban, Wrench, GraduationCap, Undo2 } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace } from '~/composables/useWorkspace'
import { toast } from '~/composables/useToast'
import { useStudyActions } from '~/composables/useStudy'
import { renderChatMarkdown } from '~/editor/chatMarkdown'
import {
  aiOpen, aiApi, streamMessage, aiErrorMessage,
  type AiConversation, type AiMessage, type AiGrant, type AiStudyGrant, type AiSuggestion, type AiMode, type AiStreamEvent,
} from '~/composables/useAi'

/**
 * Agent IA : discussion liée à la page, au projet ou à l'espace ouvert (ou globale). Lecture seule
 * tant qu'une page n'est pas désignée comme cible d'écriture ; le serveur vérifie tout.
 */
const { t, relativeTime } = useI18n()
const route = useRoute()
const qc = useQueryClient()
const { wsId } = useWorkspace()

interface Ctx { kind: AiConversation['context_kind']; id: string | null }
const here = computed<Ctx>(() => {
  if (route.path.includes('/doc/') && route.params.page) return { kind: 'page', id: route.params.page as string }
  if (route.path.includes('/projects/') && route.params.id) return { kind: 'project', id: route.params.id as string }
  if (wsId.value) return { kind: 'workspace', id: wsId.value }
  return { kind: 'none', id: null }
})
const scope = ref<'here' | 'none'>('here')
const ctx = computed<Ctx>(() => (scope.value === 'none' ? { kind: 'none', id: null } : here.value))

const { data: connections } = useQuery({ queryKey: ['ai', 'connections'], queryFn: aiApi.connections, enabled: aiOpen })
const connectionId = ref('')
watch(connections, (c) => { if (c?.length && !c.some((x) => x.id === connectionId.value)) connectionId.value = c[0]!.id }, { immediate: true })

const { data: history } = useQuery({
  queryKey: computed(() => ['ai', 'conversations', ctx.value.kind, ctx.value.id]),
  queryFn: () => aiApi.conversations(ctx.value.kind, ctx.value.id ?? undefined),
  enabled: aiOpen,
})

const conv = ref<AiConversation | null>(null)
const messages = ref<AiMessage[]>([])
const grants = ref<AiGrant[]>([])
const studyGrants = ref<AiStudyGrant[]>([])
const suggestions = ref<AiSuggestion[]>([])
const draft = ref('')
const busy = ref(false)
const streaming = ref('')
const toolLog = ref<string[]>([])
let abort: AbortController | null = null
const list = ref<HTMLElement | null>(null)

async function openConversation(id: string) {
  if (!id) { conv.value = null; messages.value = []; grants.value = []; studyGrants.value = []; suggestions.value = []; return }
  const r = await aiApi.conversation(id)
  conv.value = r.conversation
  messages.value = r.messages
  grants.value = r.grants
  studyGrants.value = r.study_grants ?? []
  suggestions.value = r.suggestions
  if (r.conversation.connection_id) connectionId.value = r.conversation.connection_id
  scrollDown()
}
// Un autre contexte : on repart d'une conversation vide (l'historique de ce contexte reste listé).
watch(() => [ctx.value.kind, ctx.value.id], () => { if (conv.value && (conv.value.context_kind !== ctx.value.kind || conv.value.context_id !== ctx.value.id)) openConversation('') })

function scrollDown() { nextTick(() => { if (list.value) list.value.scrollTop = list.value.scrollHeight }) }

async function ensureConversation(): Promise<AiConversation> {
  if (conv.value) {
    if (conv.value.connection_id !== connectionId.value) {
      await api(`/api/ai/conversations/${conv.value.id}`, { method: 'PATCH', body: { connection_id: connectionId.value } })
      conv.value = { ...conv.value, connection_id: connectionId.value }
    }
    return conv.value
  }
  const c = await api<AiConversation>('/api/ai/conversations', { method: 'POST', body: { connection_id: connectionId.value, context_kind: ctx.value.kind, context_id: ctx.value.id } })
  conv.value = c
  qc.invalidateQueries({ queryKey: ['ai', 'conversations'] })
  return c
}

function onEvent(e: AiStreamEvent) {
  if (e.type === 'text') { streaming.value += e.delta; scrollDown() }
  else if (e.type === 'tool') toolLog.value.push(`${e.name}${e.ok ? '' : ' ✕'}`)
  else if (e.type === 'suggestion') suggestions.value.push(e.suggestion)
  else if (e.type === 'page_changed') pageChanged()
  else if (e.type === 'error') toast.error(aiErrorMessage(e.message, t))
}
function pageChanged() {
  qc.invalidateQueries({ predicate: (q) => q.queryKey[0] === 'ws' && q.queryKey[2] === 'pages' })
}

async function send() {
  const text = draft.value.trim()
  if (!text || busy.value || !connectionId.value) return
  busy.value = true
  try {
    const c = await ensureConversation()
    messages.value.push({ id: `tmp-${Date.now()}`, role: 'user', content: text })
    draft.value = ''
    streaming.value = ''
    toolLog.value = []
    scrollDown()
    abort = new AbortController()
    await streamMessage(c.id, text, onEvent, abort.signal)
  } catch (e: any) {
    if (e?.name !== 'AbortError') toast.error(errorMessage(e))
  } finally {
    busy.value = false
    abort = null
    if (conv.value) await openConversation(conv.value.id).catch(() => {})
    streaming.value = ''
    qc.invalidateQueries({ queryKey: ['ai', 'conversations'] })
  }
}
function stop() { abort?.abort() }
function onKey(e: KeyboardEvent) { if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); send() } }

// --- Cible d'écriture ---------------------------------------------------------------------------
interface Ws { id: string; name: string }
interface PageRow { id: string; title: string; can_edit?: boolean }
const { data: workspaces } = useQuery({ queryKey: ['workspaces'], queryFn: async () => (await api<{ workspaces: Ws[] }>('/api/workspaces')).workspaces, enabled: aiOpen })
const targetWs = ref('')
const targetPage = ref('')
const mode = ref<AiMode>('suggest')
const minutes = ref(60)
watch([() => wsId.value, workspaces], () => { if (!targetWs.value && wsId.value) targetWs.value = wsId.value }, { immediate: true })
const { data: targetPages } = useQuery({
  queryKey: computed(() => ['ai', 'target-pages', targetWs.value]),
  queryFn: async () => (await api<PageRow[]>(`/api/workspaces/${targetWs.value}/pages`, { silent: true })).filter((p) => p.can_edit),
  enabled: computed(() => aiOpen.value && !!targetWs.value),
})
watch(targetPages, (p) => {
  if (!p?.length) { targetPage.value = ''; return }
  const cur = here.value.kind === 'page' ? here.value.id : null
  if (!p.some((x) => x.id === targetPage.value)) targetPage.value = p.find((x) => x.id === cur)?.id ?? p[0]!.id
})
async function grant() {
  if (!targetPage.value) return
  try {
    const c = await ensureConversation()
    grants.value = await api<AiGrant[]>(`/api/ai/conversations/${c.id}/grants`, { method: 'POST', body: { page_id: targetPage.value, mode: mode.value, expires_in_minutes: minutes.value } })
  } catch (e) { toast.error(errorMessage(e)) }
}
async function revoke(g: AiGrant) {
  if (!conv.value) return
  await api(`/api/ai/conversations/${conv.value.id}/grants/${g.id}`, { method: 'DELETE' })
  grants.value = grants.value.filter((x) => x.id !== g.id)
}
async function decide(s: AiSuggestion, accept: boolean) {
  try {
    await api(`/api/ai/suggestions/${s.id}/${accept ? 'accept' : 'reject'}`, { method: 'POST' })
    suggestions.value = suggestions.value.filter((x) => x.id !== s.id)
    if (accept) { pageChanged(); toast.success(t('ai.chat.applied')) }
  } catch (e) { toast.error(errorMessage(e)) }
}

// --- Jeux de révision : autorisation de créer (espace ou perso) ou de modifier un jeu ---------------
interface StudyRow { id: string; title: string; can_edit?: boolean }
const studyKind = ref<'create' | 'update'>('create')
const studyWhere = ref('personal')
const studySet = ref('')
const { data: editableSets } = useQuery({
  queryKey: ['ai', 'editable-study-sets'],
  queryFn: async () => (await api<StudyRow[]>('/api/study/sets', { silent: true })).filter((s) => s.can_edit),
  enabled: computed(() => aiOpen.value && studyKind.value === 'update'),
})
watch(editableSets, (s) => { if (!s?.some((x) => x.id === studySet.value)) studySet.value = s?.[0]?.id ?? '' })
const canStudyGrant = computed(() => (studyKind.value === 'update' ? !!studySet.value : true))
async function grantStudy() {
  if (!canStudyGrant.value) return
  const body: Record<string, unknown> = { expires_in_minutes: minutes.value }
  if (studyKind.value === 'update') body.set_id = studySet.value
  else if (studyWhere.value === 'personal') body.personal = true
  else body.workspace_id = studyWhere.value
  try {
    const c = await ensureConversation()
    studyGrants.value = await api<AiStudyGrant[]>(`/api/ai/conversations/${c.id}/study-grants`, { method: 'POST', body })
  } catch (e) { toast.error(errorMessage(e)) }
}
async function revokeStudy(g: AiStudyGrant) {
  if (!conv.value) return
  await api(`/api/ai/conversations/${conv.value.id}/study-grants/${g.id}`, { method: 'DELETE' })
  studyGrants.value = studyGrants.value.filter((x) => x.id !== g.id)
}
function studyGrantLabel(g: AiStudyGrant) {
  return g.kind === 'update'
    ? t('ai.chat.studyGrantUpdate', { title: g.set_title || t('ai.chat.untitled') })
    : t('ai.chat.studyGrantCreate', { where: g.personal ? t('ai.chat.personal') : g.workspace_name || '' })
}
// Messages à afficher : le texte, et un lien vers chaque jeu créé ou modifié (aussi dans l'historique).
// Restauration de la version d'un jeu gardée avant une modification de l'IA ; le toast propose d'annuler.
const studyActions = useStudyActions()
const restoredRevs = ref(new Set<string>())
async function restoreStudy(setId: string, revId: string, undo = false) {
  try {
    const undoId = await studyActions.restore(setId, revId)
    if (!undo) restoredRevs.value = new Set(restoredRevs.value).add(revId)
    toast.success(t(undo ? 'study.restoreUndone' : 'study.restored'), undo ? {} : { duration: 10000, action: { label: t('study.undo'), run: () => { void restoreStudy(setId, undoId, true) } } })
  } catch (e: any) {
    toast.error(errorMessage(e))
  }
}
const visible = computed(() => messages.value.filter((m) => (m.role !== 'tool' && (m.content || m.role === 'user')) || (m.role === 'tool' && m.data?.study_set_id)))
</script>

<template>
  <Transition name="ai">
    <aside v-if="aiOpen" class="ai" role="complementary" :aria-label="t('ai.title')">
      <header class="ai-head">
        <Bot :size="15" />
        <span class="ai-title">{{ t('ai.title') }}</span>
        <span class="spacer" />
        <UiButton variant="ghost" size="sm" :icon="Plus" :aria-label="t('ai.chat.new')" @click="openConversation('')" />
        <UiButton variant="ghost" size="sm" :icon="X" :aria-label="t('common.close')" @click="aiOpen = false" />
      </header>

      <div v-if="!connections?.length" class="ai-empty">
        <p>{{ t('ai.chat.noConnection') }}</p>
        <UiButton size="sm" variant="primary" :to="`/w/${wsId}/settings/ai`" @click="aiOpen = false">{{ t('ai.chat.setUp') }}</UiButton>
      </div>
      <template v-else>
        <div class="ai-bar">
          <select v-model="connectionId" class="sel" :aria-label="t('ai.chat.connection')">
            <option v-for="c in connections" :key="c.id" :value="c.id">{{ c.label }} · {{ c.model }}</option>
          </select>
          <select v-model="scope" class="sel" :aria-label="t('ai.chat.context')">
            <option value="here" :disabled="here.kind === 'none'">{{ t(`ai.chat.ctx.${here.kind}`) }}</option>
            <option value="none">{{ t('ai.chat.ctx.none') }}</option>
          </select>
          <select class="sel" :value="conv?.id ?? ''" :aria-label="t('ai.chat.history')" @change="openConversation(($event.target as HTMLSelectElement).value)">
            <option value="">{{ t('ai.chat.new') }}</option>
            <option v-for="h in history" :key="h.id" :value="h.id">{{ h.title || t('ai.chat.untitled') }} · {{ relativeTime(h.updated_at) }}</option>
          </select>
        </div>

        <div ref="list" class="ai-list">
          <p v-if="!visible.length && !streaming" class="hint">{{ t('ai.chat.readOnlyHint') }}</p>
          <template v-for="m in visible" :key="m.id">
            <div v-if="m.role === 'tool'" class="study-row">
              <NuxtLink :to="`/study/${m.data.study_set_id}?mode=edit`" class="msg study-link" @click="aiOpen = false"><GraduationCap :size="13" /> {{ t('ai.chat.studyLink') }}</NuxtLink>
              <UiButton v-if="m.data.study_revision_id" size="sm" variant="ghost" :icon="Undo2" :disabled="restoredRevs.has(m.data.study_revision_id)" @click="restoreStudy(m.data.study_set_id, m.data.study_revision_id)">{{ t('ai.chat.studyRestore') }}</UiButton>
            </div>
            <!-- eslint-disable-next-line vue/no-v-html : Markdown nettoyé par DOMPurify -->
            <div v-else-if="m.role === 'assistant'" class="msg assistant md" v-html="renderChatMarkdown(m.content)" />
            <div v-else class="msg" :class="m.role">{{ m.content }}</div>
          </template>
          <!-- eslint-disable-next-line vue/no-v-html : Markdown nettoyé par DOMPurify -->
          <div v-if="streaming" class="msg assistant md" v-html="renderChatMarkdown(streaming)" />
          <div v-if="busy && toolLog.length" class="tools"><Wrench :size="11" /> {{ toolLog.join(' · ') }}</div>
          <div v-for="s in suggestions" :key="s.id" class="sugg">
            <div class="sugg-head"><PenLine :size="13" /> {{ t('ai.chat.suggestion') }}</div>
            <pre class="diff"><span v-for="(l, i) in s.diff" :key="i" :class="{ add: l.op === '+', del: l.op === '-' }">{{ l.op === ' ' ? '  ' : `${l.op} ` }}{{ l.text }}
</span></pre>
            <div class="sugg-actions">
              <UiButton size="sm" variant="primary" :icon="Check" @click="decide(s, true)">{{ t('ai.chat.accept') }}</UiButton>
              <UiButton size="sm" variant="ghost" :icon="Ban" @click="decide(s, false)">{{ t('ai.chat.reject') }}</UiButton>
            </div>
          </div>
        </div>

        <details class="target" :open="grants.length > 0 || studyGrants.length > 0">
          <summary><PenLine :size="13" /> {{ t('ai.chat.writeTarget') }} <span v-if="!grants.length && !studyGrants.length" class="faint">· {{ t('ai.chat.readOnly') }}</span></summary>
          <div v-for="g in grants" :key="g.id" class="grant">
            <span class="grant-text truncate">{{ g.page_title || t('ai.chat.untitled') }} · {{ t(`ai.chat.mode.${g.mode}`) }} · {{ relativeTime(g.expires_at) }}</span>
            <UiButton size="sm" variant="ghost" @click="revoke(g)">{{ t('common.revoke') }}</UiButton>
          </div>
          <div class="grant-form">
            <select v-model="targetWs" class="sel" :aria-label="t('ai.chat.workspace')">
              <option v-for="w in workspaces" :key="w.id" :value="w.id">{{ w.name }}</option>
            </select>
            <select v-model="targetPage" class="sel" :aria-label="t('ai.chat.page')">
              <option v-for="p in targetPages" :key="p.id" :value="p.id">{{ p.title || t('ai.chat.untitled') }}</option>
            </select>
            <select v-model="mode" class="sel" :aria-label="t('ai.chat.modeLabel')">
              <option v-for="m in ['suggest', 'append', 'replace']" :key="m" :value="m">{{ t(`ai.chat.mode.${m}`) }}</option>
            </select>
            <select v-model.number="minutes" class="sel" :aria-label="t('ai.chat.expiry')">
              <option :value="15">15 min</option><option :value="60">1 h</option><option :value="240">4 h</option><option :value="1440">24 h</option>
            </select>
            <UiButton size="sm" variant="primary" :disabled="!targetPage" @click="grant">{{ t('ai.chat.allow') }}</UiButton>
          </div>
          <p class="hint">{{ t(`ai.chat.modeHint.${mode}`) }}</p>

          <div class="grant-title"><GraduationCap :size="13" /> {{ t('ai.chat.studyGrants') }}</div>
          <div v-for="g in studyGrants" :key="g.id" class="grant">
            <span class="grant-text truncate">{{ studyGrantLabel(g) }} · {{ relativeTime(g.expires_at) }}</span>
            <UiButton size="sm" variant="ghost" @click="revokeStudy(g)">{{ t('common.revoke') }}</UiButton>
          </div>
          <div class="grant-form">
            <select v-model="studyKind" class="sel" :aria-label="t('ai.chat.studyGrants')">
              <option value="create">{{ t('ai.chat.studyCreate') }}</option>
              <option value="update">{{ t('ai.chat.studyUpdate') }}</option>
            </select>
            <select v-if="studyKind === 'create'" v-model="studyWhere" class="sel" :aria-label="t('ai.chat.workspace')">
              <option value="personal">{{ t('ai.chat.personal') }}</option>
              <option v-for="w in workspaces" :key="w.id" :value="w.id">{{ w.name }}</option>
            </select>
            <select v-else v-model="studySet" class="sel" :aria-label="t('ai.chat.studySet')">
              <option v-for="s in editableSets" :key="s.id" :value="s.id">{{ s.title }}</option>
            </select>
            <UiButton size="sm" variant="primary" :disabled="!canStudyGrant" @click="grantStudy">{{ t('ai.chat.allow') }}</UiButton>
          </div>
        </details>

        <form class="ai-input" @submit.prevent="send">
          <UiInput v-model="draft" multiline :rows="2" :placeholder="t('ai.chat.placeholder')" @keydown="onKey" />
          <UiButton v-if="busy" variant="ghost" size="sm" :icon="Square" :aria-label="t('ai.chat.stop')" @click.prevent="stop" />
          <UiButton v-else variant="primary" size="sm" :icon="Send" :disabled="!draft.trim()" :aria-label="t('ai.chat.send')" @click.prevent="send" />
        </form>
      </template>
    </aside>
  </Transition>
</template>

<style scoped>
.ai {
  position: fixed; right: 16px; bottom: 16px; z-index: 60; display: flex; flex-direction: column;
  width: 420px; height: min(680px, calc(100vh - 32px)); max-width: calc(100vw - 32px);
  background: var(--bg-elevated); border-radius: var(--r-lg); box-shadow: var(--shadow-lg); overflow: hidden;
}
.ai-head { display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 12px; border-bottom: 1px solid var(--border); color: var(--text-2); }
.ai-title { font-weight: 600; font-size: var(--fs-sm); color: var(--text); }
.spacer { flex: 1; }
.ai-empty { padding: 24px 16px; display: flex; flex-direction: column; gap: 12px; align-items: flex-start; color: var(--text-3); }
.ai-bar { display: flex; flex-wrap: wrap; gap: 6px; padding: 8px 10px; border-bottom: 1px solid var(--border); }
.sel { height: 28px; border-radius: var(--r-sm); background: var(--bg-hover); color: var(--text); border: 1px solid var(--border); padding: 0 6px; min-width: 0; max-width: 100%; font-size: var(--fs-sm); }
.ai-bar .sel { flex: 1 1 120px; }
.ai-list { flex: 1; overflow-y: auto; padding: 10px 12px; display: flex; flex-direction: column; gap: 8px; }
.hint { color: var(--text-3); font-size: var(--fs-sm); line-height: 1.45; margin: 4px 0; }
.msg { white-space: pre-wrap; word-break: break-word; padding: 8px 10px; border-radius: var(--r-md); line-height: 1.5; max-width: 92%; }
.msg.md { white-space: normal; }
.msg.md :deep(p) { margin: 0 0 6px; }
.msg.md :deep(p:last-child), .msg.md :deep(ul:last-child), .msg.md :deep(ol:last-child) { margin-bottom: 0; }
.msg.md :deep(ul), .msg.md :deep(ol) { margin: 0 0 6px; padding-left: 20px; }
.msg.md :deep(pre) { overflow-x: auto; padding: 6px 8px; border-radius: var(--r-sm); background: var(--bg-elevated); font-size: var(--fs-xs); }
.msg.md :deep(code) { font-family: var(--font-mono, monospace); font-size: 0.92em; }
.msg.md :deep(table) { border-collapse: collapse; font-size: var(--fs-sm); }
.msg.md :deep(th), .msg.md :deep(td) { border: 1px solid var(--border); padding: 2px 6px; }
.msg.md :deep(h1), .msg.md :deep(h2), .msg.md :deep(h3), .msg.md :deep(h4) { font-size: 1em; margin: 6px 0 4px; }
.msg.user { align-self: flex-end; background: var(--accent-soft); }
.msg.assistant { align-self: flex-start; background: var(--bg-hover); }
.study-row { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.study-link { align-self: flex-start; display: inline-flex; align-items: center; gap: 6px; background: var(--accent-soft); color: var(--text); text-decoration: none; }
.tools { display: flex; align-items: center; gap: 6px; color: var(--text-4); font-size: var(--fs-xs); }
.sugg { border: 1px solid var(--border); border-radius: var(--r-md); padding: 8px; }
.sugg-head { display: flex; align-items: center; gap: 6px; font-weight: 600; font-size: var(--fs-sm); margin-bottom: 6px; }
.diff { margin: 0; max-height: 200px; overflow: auto; font-size: var(--fs-xs); font-family: var(--font-mono, monospace); white-space: pre-wrap; }
.diff .add { background: color-mix(in srgb, var(--green, #3a3) 22%, transparent); display: block; }
.diff .del { background: color-mix(in srgb, var(--red, #d33) 22%, transparent); display: block; }
.sugg-actions { display: flex; gap: 6px; margin-top: 8px; }
.target { border-top: 1px solid var(--border); padding: 6px 12px; font-size: var(--fs-sm); }
.target summary { cursor: pointer; display: flex; align-items: center; gap: 6px; color: var(--text-2); font-weight: 500; }
.faint { color: var(--text-4); font-weight: 400; }
.grant { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-top: 6px; }
.grant-text { min-width: 0; }
.grant-title { display: flex; align-items: center; gap: 6px; margin-top: 12px; color: var(--text-2); font-weight: 500; }
.grant-form { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
.grant-form .sel { flex: 1 1 110px; }
.ai-input { display: flex; align-items: flex-end; gap: 6px; padding: 8px 10px; border-top: 1px solid var(--border); }
.ai-input :deep(textarea), .ai-input :deep(.input) { flex: 1; }
.ai-input > :first-child { flex: 1; }
.ai-enter-active, .ai-leave-active { transition: opacity var(--dur-fast), transform var(--dur-fast); }
.ai-enter-from, .ai-leave-to { opacity: 0; transform: translateY(8px); }
@media (max-width: 640px) { .ai { left: 8px; right: 8px; bottom: 8px; width: auto; } }
</style>
