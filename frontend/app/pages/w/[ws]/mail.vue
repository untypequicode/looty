<script setup lang="ts">
import { MailOpen, CheckCheck, Mail, RefreshCw, ExternalLink, Plus, MoreHorizontal, Pencil, Trash2, Paperclip, AlertTriangle, ArrowLeft, Image, ChevronDown, Check } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { useLocalStorage } from '@vueuse/core'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { useMailAccounts, useMailMessages, useMailMessage, useMailInvalidate, mailDoc, mailKeys, type MailAccount, type MailSummary, type MailAddress } from '~/composables/useMail'

definePageMeta({ layout: 'workspace' })

const { t, locale, relativeTime } = useI18n()
const { isMobile } = useBreakpoint()
const route = useRoute()
const router = useRouter()
const qc = useQueryClient()
const invalidate = useMailInvalidate()

const { data: accounts, isLoading: loadingAccounts } = useMailAccounts()
const accountId = ref<string | null>(null)
watch([accounts, () => route.query.account], ([list, q]) => {
  if (!list?.length) { accountId.value = null; return }
  if (typeof q === 'string' && list.some((a) => a.id === q)) accountId.value = q
  else if (!accountId.value || !list.some((a) => a.id === accountId.value)) accountId.value = list[0]!.id
}, { immediate: true })
const account = computed(() => accounts.value?.find((a) => a.id === accountId.value) ?? null)
function selectAccount(id: string) {
  selected.value = null
  router.replace({ query: { ...route.query, account: id } })
  accountId.value = id
}

const filter = useLocalStorage<'all' | 'unread'>('looty.mail.filter', 'all')
const unreadOnly = computed(() => filter.value === 'unread')
const pages = useMailMessages(accountId, unreadOnly)
const loadingMessages = pages.isLoading
/** État lu / non lu changé ici, en attendant la prochaine synchronisation. */
const seenOverride = ref(new Map<number, boolean>())
const messages = computed(() => (pages.data.value?.pages ?? []).flatMap((pg) => pg.messages).map((m) => (seenOverride.value.has(m.uid) ? { ...m, seen: seenOverride.value.get(m.uid)! } : m)))
const selected = ref<number | null>(null)
const { data: message, isFetching: loadingMessage, error: messageError } = useMailMessage(accountId, selected)
watch(accountId, () => { selected.value = null; seenOverride.value = new Map() })

function localSeen(uid: number, seen: boolean, wasSeen: boolean) {
  seenOverride.value = new Map(seenOverride.value).set(uid, seen)
  if (seen !== wasSeen) qc.setQueryData<MailAccount[]>(mailKeys.accounts, (l) => l?.map((a) => (a.id === accountId.value ? { ...a, unread: Math.max(0, a.unread + (seen ? -1 : 1)) } : a)))
}
function open(m: MailSummary) {
  selected.value = m.uid
  showImages.value = false
  // Lu côté serveur à l'ouverture : on reflète tout de suite dans la liste et le compteur.
  if (!m.seen) localSeen(m.uid, true, false)
}
async function setSeen(m: MailSummary, seen: boolean) {
  localSeen(m.uid, seen, m.seen)
  try {
    await api(`/api/mail/accounts/${accountId.value}/messages/${m.uid}/seen`, { method: 'PUT', body: { seen } })
  } catch {
    localSeen(m.uid, m.seen, seen)
  }
}
async function readAll() {
  await api(`/api/mail/accounts/${accountId.value}/read-all`, { method: 'POST' })
  seenOverride.value = new Map()
  invalidate()
}
const current = computed(() => messages.value?.find((m) => m.uid === selected.value) ?? null)

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

function webmail() {
  if (account.value?.webmail_url) window.open(account.value.webmail_url, '_blank', 'noopener')
}

// --- Comptes ---
const dialogOpen = ref(false)
const editing = ref<MailAccount | null>(null)
function addAccount() { editing.value = null; dialogOpen.value = true }
function onSaved(a: MailAccount) { if (!editing.value) selectAccount(a.id) }
const refreshing = ref(false)
async function refresh() {
  if (!account.value) return
  refreshing.value = true
  try {
    const r = await api<{ new: number }>(`/api/mail/accounts/${account.value.id}/refresh`, { method: 'POST' })
    if (r.new) toast.success(t('mail.newCount', { count: r.new }))
  } finally {
    refreshing.value = false
    invalidate()
  }
}
const accountMenuOpen = ref(false)
const accountItems = computed<MenuItem[]>(() => [
  ...(accounts.value ?? []).map((a) => ({ label: a.unread ? `${a.label} · ${a.unread}` : a.label, checked: a.id === accountId.value, onSelect: () => selectAccount(a.id) })),
  { divider: true },
  { label: t('mail.addAccount'), icon: Plus, onSelect: addAccount },
])
const moreOpen = ref(false)
const moreItems = computed<MenuItem[]>(() => [
  { label: t('mail.readAll'), icon: CheckCheck, disabled: !account.value?.unread, onSelect: readAll },
  { divider: true },
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

useHead({ title: () => t('mail.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('mail.title'), icon: Mail }]">
      <template v-if="account">
        <UiMenu v-if="(accounts?.length ?? 0) > 1" v-model:open="accountMenuOpen" :items="accountItems" placement="bottom-end">
          <UiButton size="sm" variant="ghost">{{ account.label }}<ChevronDown :size="14" /></UiButton>
        </UiMenu>
        <UiSegmented v-model="filter" :options="[{ value: 'all', label: t('mail.all') }, { value: 'unread', label: account.unread ? `${t('mail.unread')} · ${account.unread}` : t('mail.unread') }]" />
        <UiTooltip :text="t('mail.refresh')">
          <UiButton size="sm" variant="ghost" :icon="RefreshCw" :loading="refreshing" :aria-label="t('mail.refresh')" @click="refresh" />
        </UiTooltip>
        <UiButton v-if="account.webmail_url" size="sm" :icon="ExternalLink" @click="webmail">{{ t('mail.openWebmail') }}</UiButton>
        <UiMenu v-model:open="moreOpen" :items="moreItems" placement="bottom-end">
          <UiButton size="sm" variant="ghost" :icon="MoreHorizontal" :aria-label="t('mail.account')" />
        </UiMenu>
      </template>
    </AppPageHeader>

    <div v-if="loadingAccounts" class="mail-empty"><UiSpinner /></div>
    <UiEmpty v-else-if="!account" class="mail-empty" :icon="Mail" :title="t('mail.emptyTitle')" :description="t('mail.emptyHint')">
      <UiButton variant="primary" :icon="Plus" @click="addAccount">{{ t('mail.addAccount') }}</UiButton>
    </UiEmpty>

    <div v-else class="mail" :class="{ reading: selected != null }">
      <section class="m-list">
        <div v-if="account.last_error" class="m-error">
          <AlertTriangle :size="14" />
          <span>{{ t(`errors.${account.last_error.split(':')[0]}`) === `errors.${account.last_error.split(':')[0]}` ? account.last_error : t(`errors.${account.last_error.split(':')[0]}`) }}</span>
          <UiButton size="xs" variant="ghost" @click="editing = account; dialogOpen = true">{{ t('mail.fix') }}</UiButton>
        </div>
        <div v-if="loadingMessages" class="m-skel"><div v-for="i in 8" :key="i" class="skeleton" style="height: 58px; margin: 4px 0" /></div>
        <p v-else-if="!messages.length" class="m-none">{{ unreadOnly ? t('mail.noUnread') : account.last_check_at ? t('mail.noMessages') : t('mail.syncing') }}</p>
        <div v-for="m in messages" v-else :key="m.uid" class="m-row" role="button" tabindex="0" :class="{ unread: !m.seen, on: m.uid === selected }" @click="open(m)" @keydown.enter="open(m)">
          <span class="m-top">
            <span class="m-from truncate">{{ m.from_name || m.from_addr || t('mail.unknownSender') }}</span>
            <Paperclip v-if="m.has_attachments" :size="12" class="m-clip" />
            <time class="m-date">{{ shortDate(m.sent_at) }}</time>
            <UiTooltip :text="m.seen ? t('mail.markUnread') : t('mail.markRead')">
              <button class="m-toggle" :aria-label="m.seen ? t('mail.markUnread') : t('mail.markRead')" @click.stop="setSeen(m, !m.seen)">
                <Mail v-if="m.seen" :size="13" /><MailOpen v-else :size="13" />
              </button>
            </UiTooltip>
          </span>
          <span class="m-subject truncate">{{ m.subject || t('mail.noSubject') }}</span>
          <span class="m-snippet truncate">{{ m.snippet }}</span>
        </div>
        <div v-if="pages.hasNextPage.value" class="m-more">
          <UiButton size="sm" variant="ghost" :loading="pages.isFetchingNextPage.value" @click="pages.fetchNextPage()">{{ t('mail.loadMore') }}</UiButton>
        </div>
        <p v-if="account.last_check_at" class="m-checked">{{ t('mail.checked', { when: relativeTime(account.last_check_at) }) }}</p>
      </section>

      <section class="m-read">
        <div v-if="selected == null" class="m-placeholder"><Mail :size="28" /><span>{{ t('mail.pick') }}</span></div>
        <template v-else>
          <header class="r-head">
            <UiButton v-if="isMobile" size="sm" variant="ghost" :icon="ArrowLeft" :aria-label="t('common.close')" @click="selected = null" />
            <div class="r-actions">
              <UiButton v-if="current" size="xs" variant="ghost" :icon="Mail" @click="setSeen(current, false); selected = null">{{ t('mail.markUnread') }}</UiButton>
            </div>
            <h2 class="r-subject">{{ message?.subject || current?.subject || t('mail.noSubject') }}</h2>
            <div class="r-meta">
              <UiAvatar :name="who(message?.from[0]) || current?.from_name || current?.from_addr || '?'" :size="32" />
              <div class="r-people">
                <div class="r-from"><b>{{ who(message?.from[0]) || current?.from_name }}</b> <span v-if="message?.from[0]?.address" class="r-addr">&lt;{{ message.from[0].address }}&gt;</span></div>
                <div v-if="message?.to.length" class="r-to truncate">{{ t('mail.to') }} {{ list(message.to) }}<template v-if="message.cc.length"> · {{ t('mail.cc') }} {{ list(message.cc) }}</template></div>
              </div>
              <time class="r-date">{{ fullDate(message?.date || current?.sent_at) }}</time>
            </div>
            <div v-if="message?.attachments.length || hasRemoteImages" class="r-extra">
              <span v-for="(a, i) in message?.attachments" :key="i" class="r-att"><Paperclip :size="12" />{{ a.name }} <small>{{ size(a.size) }}</small></span>
              <UiButton v-if="hasRemoteImages && !showImages" size="xs" variant="ghost" :icon="Image" @click="showImages = true">{{ t('mail.showImages') }}</UiButton>
              <span v-if="message?.attachments.length && account.webmail_url" class="r-att-hint">{{ t('mail.attachmentsHint') }}</span>
            </div>
          </header>
          <div v-if="loadingMessage && !message" class="r-loading"><UiSpinner /></div>
          <p v-else-if="messageError" class="r-failed">{{ t('mail.loadFailed') }}</p>
          <iframe v-else-if="message" class="r-body" :class="{ html: !!message.html }" sandbox="allow-popups allow-popups-to-escape-sandbox" referrerpolicy="no-referrer" :srcdoc="srcdoc" :title="message.subject || ''" />
        </template>
      </section>
    </div>

    <MailAccountDialog v-model:open="dialogOpen" :account="editing" @saved="onSaved" />
  </div>
</template>

<style scoped>
.page { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.mail-empty { flex: 1; display: grid; place-items: center; }
.mail { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(280px, 380px) 1fr; }
.m-list { overflow-y: auto; border-right: 1px solid var(--border); padding: 6px; }
.m-row { display: flex; flex-direction: column; gap: 1px; width: 100%; padding: 8px 10px; border-radius: var(--r-md); text-align: left; position: relative; }
.m-row:hover { background: var(--bg-hover); }
.m-row { cursor: pointer; }
.m-toggle { width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); opacity: 0; flex: none; }
.m-row:hover .m-toggle, .m-toggle:focus-visible { opacity: 1; }
.m-toggle:hover { color: var(--text); background: var(--bg-active, var(--bg-hover)); }
@media (hover: none) { .m-toggle { opacity: 1; } }
.m-more { display: flex; justify-content: center; padding: 8px; }
.m-row.on { background: var(--bg-active, var(--bg-hover)); }
.m-row.unread::before { content: ''; position: absolute; left: 2px; top: 14px; width: 5px; height: 5px; border-radius: 50%; background: var(--accent); }
.m-top { display: flex; align-items: center; gap: 6px; }
.m-from { flex: 1; min-width: 0; color: var(--text-2); }
.m-row.unread .m-from, .m-row.unread .m-subject { color: var(--text); font-weight: 600; }
.m-clip { color: var(--text-3); flex: none; }
.m-date { flex: none; font-size: var(--fs-sm); color: var(--text-3); }
.m-subject { color: var(--text-2); font-size: var(--fs-md); }
.m-snippet { color: var(--text-3); font-size: var(--fs-sm); }
.m-none, .m-checked { color: var(--text-3); font-size: var(--fs-sm); text-align: center; padding: 16px; margin: 0; }
.m-checked { padding: 10px; }
.m-error { display: flex; align-items: center; gap: 8px; margin: 4px 4px 8px; padding: 8px 10px; border-radius: var(--r-md); background: color-mix(in srgb, var(--red) 12%, transparent); color: var(--red); font-size: var(--fs-sm); }
.m-error span { flex: 1; }
.m-read { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
.m-placeholder { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; color: var(--text-3); }
.r-head { padding: 16px 20px 12px; border-bottom: 1px solid var(--border); display: flex; flex-direction: column; gap: 10px; }
.r-actions { display: flex; justify-content: flex-end; margin: -6px -6px -6px 0; }
.r-subject { margin: 0; font-size: 18px; font-weight: 600; line-height: 1.3; }
.r-meta { display: flex; align-items: center; gap: 10px; }
.r-people { flex: 1; min-width: 0; }
.r-addr, .r-to { color: var(--text-3); font-size: var(--fs-sm); }
.r-date { flex: none; color: var(--text-3); font-size: var(--fs-sm); }
.r-extra { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.r-att { display: inline-flex; align-items: center; gap: 5px; padding: 3px 8px; border-radius: var(--r-sm); background: var(--bg-hover); font-size: var(--fs-sm); }
.r-att small, .r-att-hint { color: var(--text-3); font-size: var(--fs-sm); }
.r-loading, .r-failed { flex: 1; display: grid; place-items: center; color: var(--text-3); }
.r-body { flex: 1; width: 100%; border: 0; background: transparent; }
.r-body.html { background: #fff; }
@media (max-width: 767px) {
  .mail { grid-template-columns: 1fr; }
  .m-list { border-right: 0; }
  .mail.reading .m-list, .mail:not(.reading) .m-read { display: none; }
}
</style>
