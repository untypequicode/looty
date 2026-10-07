<script setup lang="ts">
import {
  Mail, MailOpen, RefreshCw, ExternalLink, Plus, MoreHorizontal, Pencil, Trash2, Paperclip, AlertTriangle, ArrowLeft, Image, Star,
  Inbox, Send, FileText, Archive, ShieldAlert, Folder, FolderPlus, ChevronRight, Reply, ReplyAll, Forward, FolderInput, Search, X,
  CornerUpLeft, PenSquare, Eraser, CheckCheck, Menu as MenuIcon, Download,
} from 'lucide-vue-next'
import type { Component } from 'vue'
import { useQueryClient, type InfiniteData } from '@tanstack/vue-query'
import { useLocalStorage, refDebounced } from '@vueuse/core'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { confirmDialog, promptDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import {
  useMailAccounts, useMailFolders, useMailFolder, useFolderMessage, useMailInvalidate, useComposeDraft, mailDoc, mailKeys, mailActions, attachmentUrl,
  type MailAccount, type MailFolder, type FolderSummary, type MailAddress, type MailFilter, type FolderRole,
} from '~/composables/useMail'

definePageMeta({ layout: 'workspace' })

const { t, locale, relativeTime } = useI18n()
const { isMobile } = useBreakpoint()
const route = useRoute()
const router = useRouter()
const qc = useQueryClient()
const invalidate = useMailInvalidate()
const draft = useComposeDraft()

// --- Comptes ---
const { data: accounts, isLoading: loadingAccounts } = useMailAccounts()
const accountId = ref<string | null>(null)
watch([accounts, () => route.query.account], ([list, q]) => {
  if (!list?.length) { accountId.value = null; return }
  if (typeof q === 'string' && list.some((a) => a.id === q)) accountId.value = q
  else if (!accountId.value || !list.some((a) => a.id === accountId.value)) accountId.value = list[0]!.id
}, { immediate: true })
const account = computed(() => accounts.value?.find((a) => a.id === accountId.value) ?? null)
const senders = computed(() => (accounts.value ?? []).filter((a) => a.can_send))

// --- Dossiers ---
const folderName = ref<string>('INBOX')
watch(() => route.query.folder, (f) => { folderName.value = typeof f === 'string' && f ? f : 'INBOX' }, { immediate: true })
function go(accId: string, folder = 'INBOX') {
  selected.value = null
  selection.value = new Set()
  search.value = ''
  router.replace({ query: { ...route.query, account: accId, folder: folder === 'INBOX' ? undefined : folder } })
  accountId.value = accId
  folderName.value = folder
  if (isMobile.value) foldersOpen.value = false
}
const { data: folderData, error: foldersError } = useMailFolders(accountId)
const folders = computed(() => folderData.value?.folders ?? [])
const folder = computed(() => folders.value.find((f) => f.name === folderName.value) ?? null)
const role = computed<FolderRole | null>(() => (folderName.value.toUpperCase() === 'INBOX' ? 'inbox' : folder.value?.role ?? null))
const collapsed = useLocalStorage<string[]>('looty.mail.collapsed', [])
/** Arbre aplati : enfants sous leur parent, dans l'ordre du serveur. */
const tree = computed(() => {
  const byParent = new Map<string | null, MailFolder[]>()
  const names = new Set(folders.value.map((f) => f.name))
  for (const f of folders.value) {
    const p = f.parent && names.has(f.parent) ? f.parent : null
    byParent.set(p, [...(byParent.get(p) ?? []), f])
  }
  const out: { f: MailFolder; depth: number; hasChildren: boolean }[] = []
  const walk = (p: string | null, depth: number) => {
    for (const f of byParent.get(p) ?? []) {
      const hasChildren = byParent.has(f.name)
      out.push({ f, depth, hasChildren })
      if (hasChildren && !collapsed.value.includes(f.name)) walk(f.name, depth + 1)
    }
  }
  walk(null, 0)
  return out
})
const roleIcon: Record<string, Component> = { inbox: Inbox, sent: Send, drafts: FileText, archive: Archive, junk: ShieldAlert, trash: Trash2, flagged: Star, all: Archive }
const folderLabel = (f?: MailFolder | null) => (!f ? '' : f.role && ['inbox', 'sent', 'drafts', 'archive', 'junk', 'trash'].includes(f.role) && (f.role === 'inbox' || f.parent == null) ? t(`mail.folders.${f.role}`) : f.label)
function toggleCollapse(name: string) {
  collapsed.value = collapsed.value.includes(name) ? collapsed.value.filter((n) => n !== name) : [...collapsed.value, name]
}
const foldersOpen = ref(false)

// --- Liste ---
const filter = useLocalStorage<MailFilter>('looty.mail.filter2', 'all')
const search = ref('')
const q = refDebounced(computed(() => search.value.trim()), 350)
const pages = useMailFolder(accountId, folderName, filter, q)
const messages = computed(() => (pages.data.value?.pages ?? []).flatMap((pg) => pg.messages))
const selected = ref<number | null>(null)
const selection = ref(new Set<number>())
const current = computed(() => messages.value.find((m) => m.uid === selected.value) ?? null)
const { data: message, isFetching: loadingMessage, error: messageError } = useFolderMessage(accountId, folderName, selected)
watch([accountId, folderName, filter, q], () => { selected.value = null; selection.value = new Set() })
const outgoing = computed(() => role.value === 'sent' || role.value === 'drafts')

/** Modifie les messages en cache (affichage immédiat avant la réponse du serveur). */
function patch(fn: (m: FolderSummary) => FolderSummary | null) {
  qc.setQueriesData<InfiniteData<{ messages: FolderSummary[]; has_more: boolean; total: number }>>({ queryKey: ['mail', 'folder', accountId.value, folderName.value] }, (d) =>
    d && { ...d, pages: d.pages.map((p) => ({ ...p, messages: p.messages.map(fn).filter((m): m is FolderSummary => !!m) })) })
}
function bumpUnread(delta: number) {
  if (!delta) return
  qc.setQueryData<{ delimiter: string; folders: MailFolder[] }>(mailKeys.folders(accountId.value!), (d) => d && { ...d, folders: d.folders.map((f) => (f.name === folderName.value ? { ...f, unread: Math.max(0, f.unread + delta) } : f)) })
  if (role.value === 'inbox') qc.setQueryData<MailAccount[]>(mailKeys.accounts, (l) => l?.map((a) => (a.id === accountId.value ? { ...a, unread: Math.max(0, a.unread + delta) } : a)))
}
const refreshCounts = () => {
  qc.invalidateQueries({ queryKey: mailKeys.folders(accountId.value ?? '') })
  qc.invalidateQueries({ queryKey: mailKeys.accounts })
}

function open(m: FolderSummary) {
  selected.value = m.uid
  showImages.value = false
  // Lu côté serveur à l'ouverture : reflété tout de suite.
  if (!m.seen) { patch((x) => (x.uid === m.uid ? { ...x, seen: true } : x)); bumpUnread(-1) }
}
const targets = (m?: FolderSummary | null) => (m ? [m] : messages.value.filter((x) => selection.value.has(x.uid)))
async function setSeen(list: FolderSummary[], seen: boolean) {
  const uids = list.map((m) => m.uid)
  bumpUnread(list.filter((m) => m.seen !== seen).length * (seen ? -1 : 1))
  patch((x) => (uids.includes(x.uid) ? { ...x, seen } : x))
  try { await mailActions.flags(accountId.value!, folderName.value, uids, { seen }) } catch { pages.refetch(); refreshCounts() }
}
async function setFlagged(list: FolderSummary[], flagged: boolean) {
  const uids = list.map((m) => m.uid)
  patch((x) => (uids.includes(x.uid) ? { ...x, flagged } : x))
  try { await mailActions.flags(accountId.value!, folderName.value, uids, { flagged }) } catch { pages.refetch() }
}
function dropFromList(uids: number[]) {
  const next = selected.value != null && uids.includes(selected.value) ? nextAfter(selected.value) : selected.value
  bumpUnread(-messages.value.filter((m) => uids.includes(m.uid) && !m.seen).length)
  patch((x) => (uids.includes(x.uid) ? null : x))
  selection.value = new Set([...selection.value].filter((u) => !uids.includes(u)))
  selected.value = isMobile.value && next !== selected.value ? null : next
}
function nextAfter(uid: number) {
  const i = messages.value.findIndex((m) => m.uid === uid)
  return (messages.value[i + 1] ?? messages.value[i - 1])?.uid ?? null
}
async function moveTo(list: FolderSummary[], to: string) {
  const uids = list.map((m) => m.uid)
  if (!uids.length || to === folderName.value) return
  dropFromList(uids)
  try {
    await mailActions.move(accountId.value!, folderName.value, uids, to)
    toast.success(t('mail.moved', { count: uids.length, folder: folderLabel(folders.value.find((f) => f.name === to)) }))
  } catch (e) { toast.error(errorMessage(e)); pages.refetch() } finally { refreshCounts() }
}
async function remove(list: FolderSummary[]) {
  const uids = list.map((m) => m.uid)
  if (!uids.length) return
  const permanent = role.value === 'trash'
  if (permanent && !(await confirmDialog({ title: t('mail.deleteForeverTitle', { count: uids.length }), message: t('mail.deleteForeverHint'), danger: true, confirmLabel: t('common.delete') }))) return
  dropFromList(uids)
  try {
    const r = await mailActions.remove(accountId.value!, folderName.value, uids)
    toast.success(r.permanent ? t('mail.deletedForever', { count: uids.length }) : t('mail.trashed', { count: uids.length }))
  } catch (e) { toast.error(errorMessage(e)); pages.refetch() } finally { refreshCounts() }
}
function toggleSelect(uid: number, e?: MouseEvent) {
  const s = new Set(selection.value)
  // Maj+clic : sélection de la plage depuis le dernier coché.
  if (e?.shiftKey && lastChecked.value != null) {
    const ids = messages.value.map((m) => m.uid)
    const [a, b] = [ids.indexOf(lastChecked.value), ids.indexOf(uid)].sort((x, y) => x - y)
    ids.slice(a, b + 1).forEach((u) => s.add(u))
  } else if (s.has(uid)) s.delete(uid)
  else s.add(uid)
  lastChecked.value = uid
  selection.value = s
}
const lastChecked = ref<number | null>(null)
const allSelected = computed(() => messages.value.length > 0 && messages.value.every((m) => selection.value.has(m.uid)))
function toggleAll() { selection.value = allSelected.value ? new Set() : new Set(messages.value.map((m) => m.uid)) }
const selected_ = computed(() => targets())

// Glisser-déposer vers un dossier.
function onDragStart(e: DragEvent, m: FolderSummary) {
  const uids = selection.value.has(m.uid) ? [...selection.value] : [m.uid]
  e.dataTransfer?.setData('application/x-looty-mail', JSON.stringify(uids))
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
}
const dropTarget = ref<string | null>(null)
function onDrop(e: DragEvent, f: MailFolder) {
  dropTarget.value = null
  const raw = e.dataTransfer?.getData('application/x-looty-mail')
  if (!raw || !f.selectable) return
  const uids: number[] = JSON.parse(raw)
  moveTo(messages.value.filter((m) => uids.includes(m.uid)), f.name)
}

// --- Lecture ---
const showImages = ref(false)
const dark = ref(false)
onMounted(() => (dark.value = document.documentElement.classList.contains('dark')))
const srcdoc = computed(() => (message.value ? mailDoc(message.value, showImages.value, dark.value) : ''))
const hasRemoteImages = computed(() => !!message.value?.html && /<img[^>]+src=["']?https?:/i.test(message.value.html))
const who = (a?: MailAddress) => (a ? a.name || a.address || '' : '')
const list = (l: MailAddress[]) => l.map((a) => (a.name ? `${a.name} <${a.address}>` : a.address)).join(', ')
const fullDate = (d?: string | null) => (d ? new Date(d).toLocaleString(locale.value, { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' }) : '')
function shortDate(d?: string | null) {
  if (!d) return ''
  const date = new Date(d), now = new Date()
  if (date.toDateString() === now.toDateString()) return date.toLocaleTimeString(locale.value, { hour: '2-digit', minute: '2-digit' })
  return date.toLocaleDateString(locale.value, date.getFullYear() === now.getFullYear() ? { day: 'numeric', month: 'short' } : { day: 'numeric', month: 'short', year: '2-digit' })
}
const size = (n: number) => (n < 1024 ? `${n} o` : n < 1048576 ? `${Math.round(n / 1024)} Ko` : `${(n / 1048576).toFixed(1)} Mo`)
const rowWho = (m: FolderSummary) => (outgoing.value ? `${t('mail.to')} ${m.to.join(', ') || '—'}` : m.from_name || m.from_addr || t('mail.unknownSender'))

// --- Rédaction ---
const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
const fmt = (a: MailAddress) => (a.name ? `${a.name.replace(/[",<>]/g, '')} <${a.address}>` : a.address ?? '')
const senderFor = () => (account.value?.can_send ? account.value.id : senders.value[0]?.id)
function needSending(): string | null {
  const id = senderFor()
  if (id) return id
  toast(t('mail.setupSending'))
  editing.value = account.value
  dialogOpen.value = true
  return null
}
function compose(to: string[] = []) {
  const id = needSending()
  if (id) draft.value = { account: id, mode: 'new', to, cc: [], bcc: [], subject: '', html: '' }
}
function originalBody() {
  const m = message.value!
  return m.html ?? `<p>${esc(m.text ?? '').replace(/\n/g, '<br>')}</p>`
}
function reply(all: boolean) {
  const m = message.value
  const id = m && needSending()
  if (!m || !id) return
  const me = new Set((accounts.value ?? []).map((a) => a.email.toLowerCase()))
  const mine = m.from.some((a) => me.has((a.address ?? '').toLowerCase()))
  let to = (mine ? m.to : m.reply_to.length ? m.reply_to : m.from).map(fmt)
  const toAddr = new Set(to.map((x) => (x.match(/<([^>]+)>/)?.[1] ?? x).toLowerCase()))
  const cc = all ? [...(mine ? [] : m.to), ...m.cc].filter((a) => a.address && !me.has(a.address.toLowerCase()) && !toAddr.has(a.address.toLowerCase())).map(fmt) : []
  if (!to.length) to = cc.splice(0, 1)
  const subject = m.subject ?? ''
  draft.value = {
    account: id, mode: all ? 'replyAll' : 'reply', to, cc, bcc: [],
    subject: /^(re|réf?)\s*:/i.test(subject) ? subject : `Re: ${subject}`, html: '',
    in_reply_to: m.message_id, references: [...m.references, ...(m.message_id ? [m.message_id] : [])].slice(-20),
    reply: { folder: m.folder, uid: m.uid },
    quote: `<p>${esc(t('mail.wrote', { date: fullDate(m.date), who: m.from[0] ? fmt(m.from[0]) : '' }))}</p>${originalBody()}`,
  }
}
function forward() {
  const m = message.value
  const id = m && needSending()
  if (!m || !id) return
  const subject = m.subject ?? ''
  const head = [
    `---------- ${t('mail.forwardedHeader')} ----------`,
    `${t('mail.from')} : ${m.from.map(fmt).join(', ')}`,
    `${t('mail.date')} : ${fullDate(m.date)}`,
    `${t('mail.subject')} : ${subject}`,
    `${t('mail.to')} : ${m.to.map(fmt).join(', ')}`,
  ].map(esc).join('<br>')
  draft.value = {
    account: id, mode: 'forward', to: [], cc: [], bcc: [],
    subject: /^(fwd?|tr)\s*:/i.test(subject) ? subject : `Fwd: ${subject}`, html: '',
    forward: { folder: m.folder, uid: m.uid, attachments: m.attachments.map((a) => ({ name: a.name, size: a.size })) },
    quote: `<p>${head}</p>${originalBody()}`,
  }
}

// --- Dossiers : actions ---
async function newFolder(parent?: string | null) {
  const label = await promptDialog({ title: parent ? t('mail.newSubfolder') : t('mail.newFolder'), placeholder: t('mail.folderName'), confirmLabel: t('common.create') })
  if (!label?.trim()) return
  try {
    const r = await mailActions.createFolder(accountId.value!, label.trim(), parent)
    if (parent) collapsed.value = collapsed.value.filter((n) => n !== parent)
    await qc.invalidateQueries({ queryKey: mailKeys.folders(accountId.value!) })
    go(accountId.value!, r.name)
  } catch (e) { toast.error(errorMessage(e)) }
}
async function renameFolder(f: MailFolder) {
  const label = await promptDialog({ title: t('mail.renameFolder'), initial: f.label, confirmLabel: t('common.save') })
  if (!label?.trim() || label.trim() === f.label) return
  try {
    const r = await mailActions.renameFolder(accountId.value!, f.name, label.trim())
    await qc.invalidateQueries({ queryKey: mailKeys.folders(accountId.value!) })
    if (folderName.value === f.name) go(accountId.value!, r.name)
  } catch (e) { toast.error(errorMessage(e)) }
}
async function deleteFolder(f: MailFolder) {
  if (!(await confirmDialog({ title: t('mail.deleteFolderTitle', { name: f.label }), message: t('mail.deleteFolderHint'), danger: true, confirmLabel: t('common.delete') }))) return
  try {
    await mailActions.deleteFolder(accountId.value!, f.name)
    if (folderName.value === f.name || folderName.value.startsWith(`${f.name}${folderData.value?.delimiter ?? '/'}`)) go(accountId.value!)
    qc.invalidateQueries({ queryKey: mailKeys.folders(accountId.value!) })
  } catch (e) { toast.error(errorMessage(e)) }
}
async function emptyFolder(f: MailFolder) {
  if (!(await confirmDialog({ title: t('mail.emptyFolderTitle', { name: folderLabel(f) }), message: t('mail.emptyFolderHint'), danger: true, confirmLabel: t('mail.emptyFolder') }))) return
  await mailActions.emptyFolder(accountId.value!, f.name)
  if (folderName.value === f.name) { selected.value = null; pages.refetch() }
  refreshCounts()
}
async function readAllIn(f: MailFolder) {
  if (f.role === 'inbox') await api(`/api/mail/accounts/${accountId.value}/read-all`, { method: 'POST' })
  else {
    const uids = messages.value.filter((m) => !m.seen).map((m) => m.uid)
    if (uids.length) await mailActions.flags(accountId.value!, f.name, uids, { seen: true })
  }
  if (folderName.value === f.name) pages.refetch()
  refreshCounts()
}
const folderMenuOpen = ref<string | null>(null)
function folderItems(f: MailFolder): MenuItem[] {
  const own = !f.role
  return [
    { label: t('mail.readAll'), icon: CheckCheck, disabled: !f.unread, onSelect: () => readAllIn(f) },
    { label: t('mail.newSubfolder'), icon: FolderPlus, onSelect: () => newFolder(f.name) },
    ...(own ? [{ label: t('mail.renameFolder'), icon: Pencil, onSelect: () => renameFolder(f) }] : []),
    ...(f.role === 'trash' || f.role === 'junk' ? [{ divider: true }, { label: t('mail.emptyFolder'), icon: Eraser, danger: true, disabled: !f.total, onSelect: () => emptyFolder(f) }] : []),
    ...(own ? [{ divider: true }, { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => deleteFolder(f) }] : []),
  ]
}
const moveItems = (list: () => FolderSummary[]): MenuItem[] => tree.value
  .filter(({ f }) => f.selectable && f.name !== folderName.value)
  .map(({ f, depth }) => ({ label: `${' '.repeat(depth)}${folderLabel(f)}`, icon: f.role ? roleIcon[f.role] : Folder, onSelect: () => moveTo(list(), f.name) }))
const moveOpen = ref(false)
const bulkMoveOpen = ref(false)

// --- Comptes : actions ---
const dialogOpen = ref(false)
const editing = ref<MailAccount | null>(null)
function addAccount() { editing.value = null; dialogOpen.value = true }
function onSaved(a: MailAccount) { if (!editing.value) go(a.id) }
const refreshing = ref(false)
async function refresh() {
  if (!account.value) return
  refreshing.value = true
  try {
    if (role.value === 'inbox') {
      const r = await api<{ new: number }>(`/api/mail/accounts/${account.value.id}/refresh`, { method: 'POST' })
      if (r.new) toast.success(t('mail.newCount', { count: r.new }))
    }
  } finally {
    refreshing.value = false
    invalidate()
  }
}
const moreOpen = ref(false)
const moreItems = computed<MenuItem[]>(() => [
  ...(account.value?.webmail_url ? [{ label: t('mail.openWebmail'), icon: ExternalLink, onSelect: openWebmail }, { divider: true }] : []),
  { label: t('mail.editAccount'), icon: Pencil, onSelect: () => { editing.value = account.value; dialogOpen.value = true } },
  { label: t('mail.addAccount'), icon: Plus, onSelect: addAccount },
  { divider: true },
  { label: t('mail.removeAccount'), icon: Trash2, danger: true, onSelect: removeAccount },
])
async function removeAccount() {
  const a = account.value
  if (!a || !(await confirmDialog({ title: t('mail.removeTitle', { name: a.label }), message: t('mail.removeHint'), danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/mail/accounts/${a.id}`, { method: 'DELETE' })
  accountId.value = null
  router.replace({ query: {} })
  invalidate()
}
const readMoreOpen = ref(false)
function openWebmail() { if (account.value?.webmail_url) window.open(account.value.webmail_url, '_blank', 'noopener') }

// --- Raccourcis clavier ---
const searchInput = ref<HTMLInputElement | null>(null)
function onKey(e: KeyboardEvent) {
  const el = e.target as HTMLElement
  if (draft.value || e.ctrlKey || e.metaKey || e.altKey || el.closest('input, textarea, [contenteditable="true"], [role="dialog"]')) return
  const i = messages.value.findIndex((m) => m.uid === selected.value)
  const cur = current.value
  const actions: Record<string, () => void> = {
    j: () => { const m = messages.value[i + 1] ?? (i < 0 ? messages.value[0] : undefined); if (m) open(m) },
    k: () => { const m = messages.value[i - 1]; if (m) open(m) },
    r: () => message.value && reply(false),
    a: () => message.value && reply(true),
    f: () => message.value && forward(),
    c: () => compose(),
    s: () => cur && setFlagged([cur], !cur.flagged),
    u: () => { if (cur) { setSeen([cur], false); selected.value = null } },
    '#': () => cur ? remove([cur]) : selection.value.size && remove(selected_.value),
    Delete: () => cur ? remove([cur]) : selection.value.size && remove(selected_.value),
    '/': () => searchInput.value?.focus(),
    Escape: () => { if (selection.value.size) selection.value = new Set(); else selected.value = null },
  }
  const fn = actions[e.key]
  if (fn) { e.preventDefault(); fn() }
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))

useHead({ title: () => (folder.value ? `${folderLabel(folder.value)} · ${t('mail.title')}` : t('mail.title')) })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('mail.title'), icon: Mail }]">
      <template v-if="account">
        <UiButton v-if="isMobile" size="sm" variant="ghost" :icon="MenuIcon" :aria-label="t('mail.foldersTitle')" @click="foldersOpen = !foldersOpen" />
        <UiTooltip :text="t('mail.refresh')">
          <UiButton size="sm" variant="ghost" :icon="RefreshCw" :loading="refreshing" :aria-label="t('mail.refresh')" @click="refresh" />
        </UiTooltip>
        <UiMenu v-model:open="moreOpen" :items="moreItems" placement="bottom-end">
          <UiButton size="sm" variant="ghost" :icon="MoreHorizontal" :aria-label="t('mail.account')" />
        </UiMenu>
        <UiButton size="sm" variant="primary" :icon="PenSquare" @click="compose()">{{ t('mail.write') }}</UiButton>
      </template>
    </AppPageHeader>

    <div v-if="loadingAccounts" class="mail-empty"><UiSpinner /></div>
    <UiEmpty v-else-if="!account" class="mail-empty" :icon="Mail" :title="t('mail.emptyTitle')" :description="t('mail.emptyHint')">
      <UiButton variant="primary" :icon="Plus" @click="addAccount">{{ t('mail.addAccount') }}</UiButton>
    </UiEmpty>

    <div v-else class="mail" :class="{ reading: selected != null, 'folders-open': foldersOpen }">
      <!-- Comptes et dossiers -->
      <nav class="m-nav" :aria-label="t('mail.foldersTitle')">
        <template v-for="a in accounts" :key="a.id">
          <button class="n-account" :class="{ on: a.id === accountId }" @click="a.id !== accountId && go(a.id)">
            <span class="n-acc-label truncate">{{ a.label }}</span>
            <span v-if="a.id !== accountId && a.unread" class="n-count">{{ a.unread }}</span>
          </button>
          <template v-if="a.id === accountId">
            <p v-if="foldersError" class="n-error">{{ errorMessage(foldersError) }}</p>
            <div v-else-if="!folders.length" class="n-skel"><div v-for="i in 4" :key="i" class="skeleton" style="height: 26px; margin: 3px 0" /></div>
            <div
              v-for="{ f, depth, hasChildren } in tree" :key="f.name"
              class="n-folder" :class="{ on: f.name === folderName, drop: dropTarget === f.name, disabled: !f.selectable }"
              :style="{ paddingLeft: `${8 + depth * 14}px` }" role="button" :tabindex="f.selectable ? 0 : -1"
              @click="f.selectable && go(a.id, f.name)" @keydown.enter="f.selectable && go(a.id, f.name)"
              @dragover.prevent="f.selectable && (dropTarget = f.name)" @dragleave="dropTarget === f.name && (dropTarget = null)" @drop.prevent="onDrop($event, f)"
            >
              <button v-if="hasChildren" class="n-caret" :class="{ open: !collapsed.includes(f.name) }" :aria-label="f.label" @click.stop="toggleCollapse(f.name)"><ChevronRight :size="12" /></button>
              <span v-else class="n-caret-space" />
              <component :is="f.role ? roleIcon[f.role] ?? Folder : Folder" :size="14" class="n-icon" />
              <span class="n-label truncate">{{ folderLabel(f) }}</span>
              <span v-if="f.unread && f.role !== 'sent' && f.role !== 'trash'" class="n-count">{{ f.unread }}</span>
              <span class="n-more-wrap" @click.stop @keydown.enter.stop>
                <UiMenu :open="folderMenuOpen === f.name" :items="folderItems(f)" placement="bottom-end" @update:open="(o: boolean) => (folderMenuOpen = o ? f.name : null)">
                  <button class="n-more" :class="{ open: folderMenuOpen === f.name }" :aria-label="t('mail.folderActions')"><MoreHorizontal :size="13" /></button>
                </UiMenu>
              </span>
            </div>
            <button v-if="folders.length" class="n-new" @click="newFolder()"><FolderPlus :size="13" /> {{ t('mail.newFolder') }}</button>
          </template>
        </template>
      </nav>
      <div v-if="isMobile && foldersOpen" class="m-nav-backdrop" @click="foldersOpen = false" />

      <!-- Liste -->
      <section class="m-list">
        <div class="l-head">
          <div class="l-search">
            <Search :size="14" />
            <input ref="searchInput" v-model="search" :placeholder="t('mail.searchIn', { folder: folderLabel(folder) || 'INBOX' })" @keydown.esc="search = ''; ($event.target as HTMLInputElement).blur()" />
            <button v-if="search" :aria-label="t('common.clear')" @click="search = ''"><X :size="13" /></button>
          </div>
          <UiSegmented v-model="filter" :options="[{ value: 'all', label: t('mail.all') }, { value: 'unread', label: t('mail.unread') }, { value: 'flagged', icon: Star, title: t('mail.flagged') }]" />
        </div>
        <div v-if="selection.size" class="l-bulk">
          <input type="checkbox" :checked="allSelected" :aria-label="t('mail.selectAll')" @change="toggleAll" />
          <span class="l-bulk-n">{{ t('mail.selectedCount', { count: selection.size }) }}</span>
          <UiTooltip :text="t('mail.markRead')"><UiButton size="xs" variant="ghost" :icon="MailOpen" :aria-label="t('mail.markRead')" @click="setSeen(selected_, true)" /></UiTooltip>
          <UiTooltip :text="t('mail.markUnread')"><UiButton size="xs" variant="ghost" :icon="Mail" :aria-label="t('mail.markUnread')" @click="setSeen(selected_, false)" /></UiTooltip>
          <UiTooltip :text="t('mail.flag')"><UiButton size="xs" variant="ghost" :icon="Star" :aria-label="t('mail.flag')" @click="setFlagged(selected_, !selected_.every((m) => m.flagged))" /></UiTooltip>
          <UiMenu v-model:open="bulkMoveOpen" :items="moveItems(() => selected_)" :width="240">
            <UiButton size="xs" variant="ghost" :icon="FolderInput" :aria-label="t('mail.moveTo')" :title="t('mail.moveTo')" />
          </UiMenu>
          <UiTooltip :text="role === 'trash' ? t('mail.deleteForever') : t('common.delete')"><UiButton size="xs" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove(selected_)" /></UiTooltip>
          <span class="l-spacer" />
          <UiButton size="xs" variant="ghost" :icon="X" :aria-label="t('common.cancel')" @click="selection = new Set()" />
        </div>
        <div class="l-rows">
          <div v-if="account.last_error && role === 'inbox'" class="m-error">
            <AlertTriangle :size="14" />
            <span>{{ t(`errors.${account.last_error.split(':')[0]}`) === `errors.${account.last_error.split(':')[0]}` ? account.last_error : t(`errors.${account.last_error.split(':')[0]}`) }}</span>
            <UiButton size="xs" variant="ghost" @click="editing = account; dialogOpen = true">{{ t('mail.fix') }}</UiButton>
          </div>
          <div v-if="pages.isLoading.value" class="m-skel"><div v-for="i in 8" :key="i" class="skeleton" style="height: 58px; margin: 4px 0" /></div>
          <p v-else-if="pages.error.value" class="m-none">{{ errorMessage(pages.error.value) }}</p>
          <p v-else-if="!messages.length" class="m-none">{{ q ? t('mail.noResults') : filter === 'unread' ? t('mail.noUnread') : filter === 'flagged' ? t('mail.noFlagged') : t('mail.emptyFolderList') }}</p>
          <div
            v-for="m in messages" v-else :key="m.uid" class="m-row" role="button" tabindex="0" draggable="true"
            :class="{ unread: !m.seen, on: m.uid === selected, checked: selection.has(m.uid), selecting: selection.size > 0 }"
            @click="selection.size ? toggleSelect(m.uid, $event) : open(m)" @keydown.enter="open(m)" @dragstart="onDragStart($event, m)"
          >
            <input type="checkbox" class="m-check" :checked="selection.has(m.uid)" :aria-label="t('mail.select')" @click.stop="toggleSelect(m.uid, $event)" />
            <span class="m-main">
              <span class="m-top">
                <span class="m-from truncate">{{ rowWho(m) }}</span>
                <CornerUpLeft v-if="m.answered" :size="12" class="m-ic" :aria-label="t('mail.answered')" />
                <Paperclip v-if="m.has_attachments" :size="12" class="m-ic" />
                <time class="m-date">{{ shortDate(m.sent_at) }}</time>
              </span>
              <span class="m-subject-line">
                <span class="m-subject truncate">{{ m.subject || t('mail.noSubject') }}</span>
                <button class="m-star" :class="{ on: m.flagged }" :aria-label="m.flagged ? t('mail.unflag') : t('mail.flag')" @click.stop="setFlagged([m], !m.flagged)"><Star :size="13" /></button>
                <button class="m-toggle" :aria-label="m.seen ? t('mail.markUnread') : t('mail.markRead')" :title="m.seen ? t('mail.markUnread') : t('mail.markRead')" @click.stop="setSeen([m], !m.seen)">
                  <Mail v-if="m.seen" :size="13" /><MailOpen v-else :size="13" />
                </button>
              </span>
              <span class="m-snippet truncate">{{ m.snippet }}</span>
            </span>
          </div>
          <div v-if="pages.hasNextPage.value" class="m-more">
            <UiButton size="sm" variant="ghost" :loading="pages.isFetchingNextPage.value" @click="pages.fetchNextPage()">{{ t('mail.loadMore') }}</UiButton>
          </div>
          <p v-if="account.last_check_at && role === 'inbox'" class="m-checked">{{ t('mail.checked', { when: relativeTime(account.last_check_at) }) }}</p>
        </div>
      </section>

      <!-- Lecture -->
      <section class="m-read">
        <div v-if="selected == null" class="m-placeholder"><Mail :size="28" /><span>{{ t('mail.pick') }}</span></div>
        <template v-else>
          <div class="r-bar">
            <UiButton v-if="isMobile" size="sm" variant="ghost" :icon="ArrowLeft" :aria-label="t('common.close')" @click="selected = null" />
            <UiTooltip :text="t('mail.reply')" shortcut="R"><UiButton size="sm" variant="ghost" :icon="Reply" :disabled="!message" :aria-label="t('mail.reply')" @click="reply(false)" /></UiTooltip>
            <UiTooltip :text="t('mail.replyAll')" shortcut="A"><UiButton size="sm" variant="ghost" :icon="ReplyAll" :disabled="!message" :aria-label="t('mail.replyAll')" @click="reply(true)" /></UiTooltip>
            <UiTooltip :text="t('mail.forward')" shortcut="F"><UiButton size="sm" variant="ghost" :icon="Forward" :disabled="!message" :aria-label="t('mail.forward')" @click="forward" /></UiTooltip>
            <span class="r-sep" />
            <UiTooltip v-if="current" :text="current.flagged ? t('mail.unflag') : t('mail.flag')" shortcut="S">
              <UiButton size="sm" variant="ghost" :class="{ starred: current.flagged }" :icon="Star" :aria-label="t('mail.flag')" @click="setFlagged([current], !current.flagged)" />
            </UiTooltip>
            <UiTooltip v-if="current" :text="t('mail.markUnread')" shortcut="U">
              <UiButton size="sm" variant="ghost" :icon="Mail" :aria-label="t('mail.markUnread')" @click="setSeen([current], false); selected = null" />
            </UiTooltip>
            <UiMenu v-if="current" v-model:open="moveOpen" :items="moveItems(() => (current ? [current] : []))" :width="240">
              <UiButton size="sm" variant="ghost" :icon="FolderInput" :aria-label="t('mail.moveTo')" :title="t('mail.moveTo')" />
            </UiMenu>
            <UiTooltip v-if="current" :text="role === 'trash' ? t('mail.deleteForever') : t('common.delete')" shortcut="#">
              <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove([current])" />
            </UiTooltip>
            <span class="l-spacer" />
            <UiMenu v-if="account.webmail_url" v-model:open="readMoreOpen" :items="[{ label: t('mail.openWebmail'), icon: ExternalLink, onSelect: openWebmail }]" placement="bottom-end">
              <UiButton size="sm" variant="ghost" :icon="MoreHorizontal" :aria-label="t('mail.more')" />
            </UiMenu>
          </div>
          <header class="r-head">
            <h2 class="r-subject">{{ message?.subject || current?.subject || t('mail.noSubject') }}</h2>
            <div class="r-meta">
              <UiAvatar :name="who(message?.from[0]) || current?.from_name || current?.from_addr || '?'" :size="32" />
              <div class="r-people">
                <div class="r-from"><b>{{ who(message?.from[0]) || current?.from_name }}</b> <span v-if="message?.from[0]?.address" class="r-addr">&lt;{{ message.from[0].address }}&gt;</span></div>
                <div v-if="message?.to.length" class="r-to truncate" :title="list(message.to)">{{ t('mail.to') }} {{ list(message.to) }}<template v-if="message.cc.length"> · {{ t('mail.cc') }} {{ list(message.cc) }}</template></div>
              </div>
              <time class="r-date">{{ fullDate(message?.date || current?.sent_at) }}</time>
            </div>
            <div v-if="message?.attachments.length || hasRemoteImages" class="r-extra">
              <a v-for="a in message?.attachments" :key="a.index" class="r-att" :href="attachmentUrl(account.id, message!.folder, message!.uid, a.index)" :download="a.name">
                <Paperclip :size="12" />{{ a.name }} <small>{{ size(a.size) }}</small><Download :size="12" class="r-att-dl" />
              </a>
              <UiButton v-if="hasRemoteImages && !showImages" size="xs" variant="ghost" :icon="Image" @click="showImages = true">{{ t('mail.showImages') }}</UiButton>
            </div>
          </header>
          <div v-if="loadingMessage && !message" class="r-loading"><UiSpinner /></div>
          <p v-else-if="messageError" class="r-failed">{{ t('mail.loadFailed') }}</p>
          <iframe v-else-if="message" class="r-body" :class="{ html: !!message.html }" sandbox="allow-popups allow-popups-to-escape-sandbox" referrerpolicy="no-referrer" :srcdoc="srcdoc" :title="message.subject || ''" />
        </template>
      </section>
    </div>

    <MailAccountDialog v-model:open="dialogOpen" :account="editing" @saved="onSaved" />
    <MailCompose />
  </div>
</template>

<style scoped>
.page { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.mail-empty { flex: 1; display: grid; place-items: center; }
.mail { flex: 1; min-height: 0; display: grid; grid-template-columns: 210px minmax(280px, 380px) 1fr; position: relative; }

.m-nav { overflow-y: auto; border-right: 1px solid var(--border); padding: 8px 6px; display: flex; flex-direction: column; gap: 1px; background: var(--bg-panel); }
.n-account { display: flex; align-items: center; gap: 6px; padding: 6px 8px 4px; font-size: var(--fs-xs); font-weight: 600; text-transform: uppercase; letter-spacing: .03em; color: var(--text-3); text-align: left; border-radius: var(--r-sm); }
.n-account:not(:first-child) { margin-top: 8px; }
.n-account:not(.on):hover { background: var(--bg-hover); color: var(--text-2); }
.n-acc-label { flex: 1; min-width: 0; }
.n-folder { display: flex; align-items: center; gap: 4px; height: 28px; padding-right: 4px; border-radius: var(--r-sm); color: var(--text-2); cursor: pointer; font-size: var(--fs-md); }
.n-folder:hover { background: var(--bg-hover); }
.n-folder.on { background: var(--bg-active, var(--bg-hover)); color: var(--text); font-weight: 500; }
.n-folder.drop { background: var(--accent-soft); outline: 1px dashed var(--accent); }
.n-folder.disabled { cursor: default; opacity: .7; }
.n-caret, .n-caret-space { width: 14px; height: 14px; flex: none; display: grid; place-items: center; color: var(--text-3); border-radius: 3px; }
.n-caret:hover { background: var(--bg-active); }
.n-caret svg { transition: transform .12s; }
.n-caret.open svg { transform: rotate(90deg); }
.n-icon { flex: none; color: var(--text-3); }
.n-folder.on .n-icon { color: var(--text-2); }
.n-label { flex: 1; min-width: 0; }
.n-count { font-size: var(--fs-xs); font-weight: 600; color: var(--accent-text); min-width: 18px; text-align: right; }
.n-more { width: 20px; height: 20px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); opacity: 0; flex: none; }
.n-folder:hover .n-more, .n-more:focus-visible, .n-more.open { opacity: 1; }
.n-more-wrap { display: contents; }
.n-more:hover { background: var(--bg-active); color: var(--text); }
@media (hover: none) { .n-more { opacity: 1; } }
.n-new { display: flex; align-items: center; gap: 6px; padding: 5px 8px; margin-top: 4px; font-size: var(--fs-sm); color: var(--text-3); border-radius: var(--r-sm); text-align: left; }
.n-new:hover { background: var(--bg-hover); color: var(--text-2); }
.n-error { margin: 4px 8px; font-size: var(--fs-sm); color: var(--red); }

.m-list { min-height: 0; display: flex; flex-direction: column; border-right: 1px solid var(--border); }
.l-head { display: flex; align-items: center; gap: 6px; padding: 8px 8px 6px; }
.l-search { flex: 1; min-width: 0; display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 8px; border-radius: var(--r-md); background: var(--bg-input); border: 1px solid var(--border); color: var(--text-3); }
.l-search:focus-within { border-color: var(--accent); }
.l-search input { flex: 1; min-width: 0; border: 0; outline: none; background: transparent; color: var(--text); font: inherit; font-size: var(--fs-sm); }
.l-search button { display: grid; place-items: center; color: var(--text-3); }
.l-bulk { display: flex; align-items: center; gap: 2px; padding: 4px 8px 4px 16px; border-bottom: 1px solid var(--border); background: var(--accent-soft); }
.l-bulk-n { font-size: var(--fs-sm); font-weight: 500; margin: 0 8px 0 6px; }
.l-spacer { flex: 1; }
.l-rows { flex: 1; min-height: 0; overflow-y: auto; padding: 2px 6px 6px; }
.m-row { display: flex; align-items: flex-start; gap: 8px; width: 100%; padding: 8px 10px; border-radius: var(--r-md); text-align: left; position: relative; cursor: pointer; }
.m-row:hover { background: var(--bg-hover); }
.m-row.on { background: var(--bg-active, var(--bg-hover)); }
.m-row.checked { background: var(--accent-soft); }
.m-row.unread::before { content: ''; position: absolute; left: 2px; top: 15px; width: 5px; height: 5px; border-radius: 50%; background: var(--accent); }
.m-check { margin-top: 3px; flex: none; opacity: 0; width: 14px; }
.m-row:hover .m-check, .m-row.selecting .m-check, .m-check:focus-visible { opacity: 1; }
@media (hover: none) { .m-check { display: none; } .m-row.selecting .m-check { display: block; } }
.m-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
.m-top, .m-subject-line { display: flex; align-items: center; gap: 6px; }
.m-from { flex: 1; min-width: 0; color: var(--text-2); }
.m-row.unread .m-from, .m-row.unread .m-subject { color: var(--text); font-weight: 600; }
.m-ic { color: var(--text-3); flex: none; }
.m-date { flex: none; font-size: var(--fs-sm); color: var(--text-3); }
.m-subject { flex: 1; min-width: 0; color: var(--text-2); font-size: var(--fs-md); }
.m-snippet { color: var(--text-3); font-size: var(--fs-sm); }
.m-star, .m-toggle { width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); opacity: 0; flex: none; }
.m-row:hover .m-star, .m-row:hover .m-toggle, .m-star:focus-visible, .m-toggle:focus-visible, .m-star.on { opacity: 1; }
.m-star.on { color: #e0a100; }
.m-star.on svg { fill: currentColor; }
.m-star:hover, .m-toggle:hover { color: var(--text); background: var(--bg-active, var(--bg-hover)); }
@media (hover: none) { .m-toggle { display: none; } .m-star { opacity: 1; } }
.m-more { display: flex; justify-content: center; padding: 8px; }
.m-none, .m-checked { color: var(--text-3); font-size: var(--fs-sm); text-align: center; padding: 16px; margin: 0; }
.m-checked { padding: 10px; }
.m-error { display: flex; align-items: center; gap: 8px; margin: 4px 4px 8px; padding: 8px 10px; border-radius: var(--r-md); background: color-mix(in srgb, var(--red) 12%, transparent); color: var(--red); font-size: var(--fs-sm); }
.m-error span { flex: 1; }

.m-read { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
.m-placeholder { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; color: var(--text-3); }
.r-bar { display: flex; align-items: center; gap: 2px; padding: 6px 10px; border-bottom: 1px solid var(--border); }
.r-sep { width: 1px; height: 18px; background: var(--border); margin: 0 4px; }
.r-bar :deep(.starred) { color: #e0a100; }
.r-bar :deep(.starred svg) { fill: currentColor; }
.r-head { padding: 14px 20px 12px; border-bottom: 1px solid var(--border); display: flex; flex-direction: column; gap: 10px; }
.r-subject { margin: 0; font-size: 18px; font-weight: 600; line-height: 1.3; }
.r-meta { display: flex; align-items: center; gap: 10px; }
.r-people { flex: 1; min-width: 0; }
.r-addr, .r-to { color: var(--text-3); font-size: var(--fs-sm); }
.r-date { flex: none; color: var(--text-3); font-size: var(--fs-sm); }
.r-extra { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.r-att { display: inline-flex; align-items: center; gap: 5px; padding: 3px 8px; border-radius: var(--r-sm); background: var(--bg-hover); font-size: var(--fs-sm); color: var(--text); text-decoration: none; }
.r-att:hover { background: var(--bg-active); }
.r-att small { color: var(--text-3); }
.r-att-dl { color: var(--text-3); }
.r-loading, .r-failed { flex: 1; display: grid; place-items: center; color: var(--text-3); }
.r-body { flex: 1; width: 100%; border: 0; background: transparent; }
.r-body.html { background: #fff; }

@media (max-width: 1100px) and (min-width: 768px) {
  .mail { grid-template-columns: 180px minmax(240px, 320px) 1fr; }
}
@media (max-width: 767px) {
  .mail { grid-template-columns: 1fr; }
  .m-nav { position: absolute; inset: 0 auto 0 0; width: 260px; z-index: 20; transform: translateX(-100%); transition: transform .18s; box-shadow: var(--shadow-lg); }
  .mail.folders-open .m-nav { transform: none; }
  .m-nav-backdrop { position: absolute; inset: 0; z-index: 19; background: rgba(0, 0, 0, .25); }
  .m-list { border-right: 0; }
  .mail.reading .m-list, .mail:not(.reading) .m-read { display: none; }
}
</style>
