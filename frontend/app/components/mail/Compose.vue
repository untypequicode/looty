<script setup lang="ts">
import { X, Paperclip, Send, Trash2, Minus, Maximize2, ChevronDown } from 'lucide-vue-next'
import { api, errorMessage, uploadFile } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { useComposeDraft, useMailAccounts, useMailInvalidate } from '~/composables/useMail'

/**
 * Rédaction d'un message (nouveau, réponse, transfert) dans un panneau flottant :
 * destinataires avec suggestions, pièces jointes, signature du compte, Ctrl+Entrée
 * pour envoyer. Le message cité ou transféré est ajouté sous le texte, tel quel.
 */
const draft = useComposeDraft()
const { t } = useI18n()
const { isMobile } = useBreakpoint()
const invalidate = useMailInvalidate()
const { data: accounts } = useMailAccounts()
const senders = computed(() => (accounts.value ?? []).filter((a) => a.can_send))
const account = computed(() => senders.value.find((a) => a.id === draft.value?.account) ?? null)

const minimized = ref(false)
const showCc = ref(false)
const showQuote = ref(false)
const body = ref('')
const files = ref<{ url: string; name: string; size: number }[]>([])
const uploading = ref(0)
const sending = ref(false)
const lite = ref<{ editor?: { value?: { getText: () => string } }; focus: () => void } | null>(null)
const toInput = ref<{ flush: () => void; focus: () => void } | null>(null)
const ccInput = ref<{ flush: () => void } | null>(null)
const bccInput = ref<{ flush: () => void } | null>(null)

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
const signatureHtml = (sig?: string | null) => (sig ? `<p>-- <br>${esc(sig).replace(/\n/g, '<br>')}</p>` : '')

// Nouveau brouillon : corps vide, pièces jointes remises à zéro (la signature du compte
// expéditeur est ajoutée à l'envoi).
watch(() => draft.value && `${draft.value.mode}:${draft.value.account}:${draft.value.reply?.uid ?? draft.value.forward?.uid ?? ''}:${draft.value.subject}`, (k, old) => {
  if (!k || k === old || !draft.value) return
  body.value = ''
  files.value = []
  minimized.value = false
  showCc.value = draft.value.cc.length > 0 || draft.value.bcc.length > 0
  showQuote.value = draft.value.mode === 'forward'
  nextTick(() => (draft.value?.mode === 'new' || draft.value?.mode === 'forward' ? toInput.value?.focus() : lite.value?.focus()))
}, { immediate: true })

const fileInput = ref<HTMLInputElement | null>(null)
async function addFiles(list: FileList | File[]) {
  for (const f of [...list]) {
    if (f.size > 20 * 1024 * 1024) { toast.error(t('errors.mail_too_large')); continue }
    uploading.value++
    try {
      const r = await uploadFile(f)
      files.value.push({ url: r.url, name: r.name, size: r.size })
    } catch (e) { toast.error(errorMessage(e)) } finally { uploading.value-- }
  }
  if (fileInput.value) fileInput.value.value = ''
}
const size = (n: number) => (n < 1024 ? `${n} o` : n < 1048576 ? `${Math.round(n / 1024)} Ko` : `${(n / 1048576).toFixed(1)} Mo`)

const dirty = computed(() => {
  const d = draft.value
  if (!d) return false
  return (d.mode === 'new' && (d.to.length > 0 || d.subject.trim() !== '')) || body.value !== '' || files.value.length > 0
})
async function close() {
  if (dirty.value && !(await confirmDialog({ title: t('mail.discardTitle'), message: t('mail.discardHint'), danger: true, confirmLabel: t('mail.discard') }))) return
  draft.value = null
}

async function send() {
  const d = draft.value
  if (!d || sending.value) return
  toInput.value?.flush(); ccInput.value?.flush(); bccInput.value?.flush()
  if (!d.to.length && !d.cc.length && !d.bcc.length) { toast.error(t('errors.mail_no_recipient')); return }
  if (!d.subject.trim() && !(await confirmDialog({ title: t('mail.noSubjectTitle'), message: t('mail.noSubjectHint'), confirmLabel: t('mail.send') }))) return
  if (uploading.value) { toast(t('mail.waitUploads')); return }
  sending.value = true
  try {
    const quote = d.quote ? (d.mode === 'forward' ? d.quote : `<blockquote type="cite">${d.quote}</blockquote>`) : ''
    const sig = account.value?.signature
    const html = `${body.value || '<p></p>'}${signatureHtml(sig)}${quote}`
    const tmp = document.createElement('div')
    tmp.innerHTML = quote
    const quoteText = tmp.textContent?.trim() ? `\n\n${d.mode === 'forward' ? tmp.innerText : tmp.innerText.split('\n').map((l) => `> ${l}`).join('\n')}` : ''
    const text = (lite.value?.editor?.value?.getText() ?? '') + (sig ? `\n\n-- \n${sig}` : '') + quoteText
    const r = await api<{ warning?: string | null }>(`/api/mail/accounts/${d.account}/send`, {
      method: 'POST',
      silent: true,
      body: {
        to: d.to, cc: d.cc, bcc: d.bcc, subject: d.subject, html, text,
        in_reply_to: d.in_reply_to, references: d.references ?? [],
        attachments: files.value.map((f) => ({ url: f.url, name: f.name })),
        reply: d.reply, forward: d.forward ? { folder: d.forward.folder, uid: d.forward.uid } : undefined,
      },
    })
    toast.success(r.warning ? t('mail.sentNoCopy') : t('mail.sent'))
    draft.value = null
    invalidate()
  } catch (e) {
    toast.error(errorMessage(e))
  } finally { sending.value = false }
}
function onKey(e: KeyboardEvent) {
  if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); send() }
}
const title = computed(() => draft.value?.subject.trim() || t(`mail.compose.${draft.value?.mode ?? 'new'}`))
const fromItems = computed(() => senders.value.map((a) => ({ value: a.id, label: a.display_name ? `${a.display_name} <${a.email}>` : a.email })))
const fromOpen = ref(false)
</script>

<template>
  <Teleport to="body">
    <section v-if="draft" class="cp" :class="{ min: minimized, mobile: isMobile }" role="dialog" :aria-label="title" @keydown="onKey" @dragover.prevent @drop.prevent="addFiles($event.dataTransfer?.files ?? [])">
      <header class="cp-head" @click="minimized && (minimized = false)">
        <span class="cp-title truncate">{{ title }}</span>
        <UiButton v-if="!isMobile" size="xs" variant="ghost" :icon="minimized ? Maximize2 : Minus" :aria-label="minimized ? t('mail.expand') : t('mail.minimize')" @click.stop="minimized = !minimized" />
        <UiButton size="xs" variant="ghost" :icon="X" :aria-label="t('common.close')" @click.stop="close" />
      </header>
      <template v-if="!minimized">
        <div class="cp-fields">
          <div v-if="senders.length > 1" class="cp-from">
            <span class="cp-label">{{ t('mail.from') }}</span>
            <UiCombobox v-model="draft.account" v-model:open="fromOpen" :items="fromItems" :width="320">
              <button class="cp-from-btn">{{ fromItems.find((i) => i.value === draft!.account)?.label }} <ChevronDown :size="13" /></button>
            </UiCombobox>
          </div>
          <div class="cp-to">
            <MailRecipientInput ref="toInput" v-model="draft.to" :label="t('mail.to')" />
            <button v-if="!showCc" class="cp-cc-toggle" @click="showCc = true">{{ t('mail.ccBcc') }}</button>
          </div>
          <template v-if="showCc">
            <MailRecipientInput ref="ccInput" v-model="draft.cc" :label="t('mail.cc')" />
            <MailRecipientInput ref="bccInput" v-model="draft.bcc" :label="t('mail.bcc')" />
          </template>
          <input v-model="draft.subject" class="cp-subject" :placeholder="t('mail.subject')" maxlength="500" />
        </div>
        <div class="cp-body" @click.self="lite?.focus()">
          <EditorLite ref="lite" v-model="body" :placeholder="t('mail.writePlaceholder')" />
          <div v-if="account?.signature" class="cp-sig">-- <br>{{ account.signature }}</div>
          <div v-if="draft.quote" class="cp-quote">
            <button class="cp-quote-toggle" :title="showQuote ? t('mail.hideQuote') : t('mail.showQuote')" @click="showQuote = !showQuote">•••</button>
            <!-- eslint-disable-next-line vue/no-v-html : HTML déjà nettoyé par le serveur -->
            <div v-if="showQuote" class="cp-quote-body" v-html="draft.mode === 'forward' ? draft.quote : `<blockquote>${draft.quote}</blockquote>`" />
          </div>
        </div>
        <div v-if="files.length || draft.forward?.attachments.length || uploading" class="cp-files">
          <span v-for="(f, i) in files" :key="f.url" class="cp-file"><Paperclip :size="12" />{{ f.name }} <small>{{ size(f.size) }}</small>
            <button :aria-label="t('common.delete')" @click="files.splice(i, 1)"><X :size="11" /></button>
          </span>
          <span v-for="f in draft.forward?.attachments ?? []" :key="f.name" class="cp-file fwd"><Paperclip :size="12" />{{ f.name }} <small>{{ size(f.size) }}</small></span>
          <span v-if="uploading" class="cp-file"><UiSpinner :size="12" /> {{ t('mail.uploading') }}</span>
        </div>
        <footer class="cp-foot">
          <UiButton variant="primary" size="sm" :icon="Send" :loading="sending" @click="send">{{ t('mail.send') }}</UiButton>
          <span class="cp-hint"><UiKbd :keys="['mod', 'enter']" /></span>
          <input ref="fileInput" type="file" multiple hidden @change="addFiles(($event.target as HTMLInputElement).files ?? [])" />
          <UiTooltip :text="t('mail.attach')">
            <UiButton size="sm" variant="ghost" :icon="Paperclip" :aria-label="t('mail.attach')" @click="fileInput?.click()" />
          </UiTooltip>
          <span class="cp-spacer" />
          <UiTooltip :text="t('mail.discard')">
            <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('mail.discard')" @click="close" />
          </UiTooltip>
        </footer>
      </template>
    </section>
  </Teleport>
</template>

<style scoped>
.cp { position: fixed; right: 24px; bottom: 0; z-index: 850; width: 580px; max-width: calc(100vw - 32px); height: min(620px, calc(100vh - 80px)); display: flex; flex-direction: column; background: var(--bg-panel); border-radius: var(--r-lg) var(--r-lg) 0 0; box-shadow: var(--shadow-lg); border: 1px solid var(--border); border-bottom: 0; }
.cp.min { height: auto; width: 320px; }
.cp.mobile { inset: 0; width: 100%; max-width: none; height: 100%; border-radius: 0; border: 0; }
.cp-head { display: flex; align-items: center; gap: 4px; padding: 6px 8px 6px 14px; border-bottom: 1px solid var(--border); cursor: default; }
.cp.min .cp-head { cursor: pointer; border-bottom: 0; }
.cp-title { flex: 1; font-weight: 600; font-size: var(--fs-md); }
.cp-fields { padding: 0 14px; }
.cp-from { display: flex; align-items: center; gap: 6px; min-height: 34px; border-bottom: 1px solid var(--border); }
.cp-label { color: var(--text-3); font-size: var(--fs-sm); min-width: 34px; }
.cp-from-btn { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-sm); color: var(--text); padding: 2px 6px; border-radius: var(--r-sm); }
.cp-from-btn:hover { background: var(--bg-hover); }
.cp-to { position: relative; }
.cp-to :deep(.rc) { padding-right: 60px; }
.cp-cc-toggle { position: absolute; right: 0; top: 8px; font-size: var(--fs-sm); color: var(--text-3); }
.cp-cc-toggle:hover { color: var(--text); text-decoration: underline; }
.cp-subject { width: 100%; height: 36px; border: 0; border-bottom: 1px solid var(--border); outline: none; background: transparent; color: var(--text); font: inherit; font-weight: 500; }
.cp-body { flex: 1; min-height: 0; overflow-y: auto; padding: 10px 14px; }
.cp-body :deep(.lite .ProseMirror) { min-height: 120px; }
.cp-sig { margin-top: 10px; white-space: pre-wrap; color: var(--text-3); font-size: var(--fs-sm); }
.cp-quote { margin-top: 8px; }
.cp-quote-toggle { padding: 0 8px; height: 16px; line-height: 12px; border-radius: var(--r-sm); background: var(--bg-hover); color: var(--text-3); font-size: 11px; letter-spacing: 1px; }
.cp-quote-body { margin-top: 8px; font-size: var(--fs-sm); color: var(--text-2); overflow-x: auto; }
.cp-quote-body :deep(blockquote) { margin: 0; padding-left: 10px; border-left: 2px solid var(--border-strong); }
.cp-quote-body :deep(img) { max-width: 100%; height: auto; }
.cp-files { display: flex; flex-wrap: wrap; gap: 6px; padding: 6px 14px; border-top: 1px solid var(--border); }
.cp-file { display: inline-flex; align-items: center; gap: 5px; padding: 3px 4px 3px 8px; border-radius: var(--r-sm); background: var(--bg-hover); font-size: var(--fs-sm); }
.cp-file.fwd { padding-right: 8px; }
.cp-file small { color: var(--text-3); }
.cp-file button { width: 16px; height: 16px; display: grid; place-items: center; border-radius: 50%; color: var(--text-3); }
.cp-file button:hover { background: var(--bg-active); color: var(--text); }
.cp-foot { display: flex; align-items: center; gap: 6px; padding: 8px 10px; border-top: 1px solid var(--border); }
.cp-hint { color: var(--text-3); font-size: var(--fs-xs); display: inline-flex; gap: 2px; }
.cp-spacer { flex: 1; }
@media (max-width: 767px) { .cp-hint { display: none; } }
</style>
