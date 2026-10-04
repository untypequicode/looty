<script setup lang="ts">
import { api, errorMessage } from '~/composables/api'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { useMailInvalidate, type MailAccount } from '~/composables/useMail'

const open = defineModel<boolean>('open', { default: false })
/** Compte à modifier (sinon : ajout). */
const props = defineProps<{ account?: MailAccount | null }>()
const emit = defineEmits<{ saved: [account: MailAccount] }>()
const { wsId } = useWorkspace()
const { t } = useI18n()
const invalidate = useMailInvalidate()

const form = reactive({ label: '', email: '', host: '', port: 993, security: 'tls' as 'tls' | 'starttls', username: '', password: '', webmail: '', notify: true })
const busy = ref(false)
const error = ref('')
const editing = computed(() => !!props.account)

watch(open, (o) => {
  if (!o) return
  const a = props.account
  Object.assign(form, a
    ? { label: a.label, email: a.email, host: a.imap_host, port: a.imap_port, security: a.security, username: a.username, password: '', webmail: a.webmail_url ?? '', notify: a.notify }
    : { label: '', email: '', host: '', port: 993, security: 'tls', username: '', password: '', webmail: '', notify: true })
  error.value = ''
})
watch(() => form.security, (s) => { if (form.port === 993 || form.port === 143) form.port = s === 'tls' ? 993 : 143 })

/** Pré-remplissage depuis l'adresse : imap.<domaine>, identifiant = adresse. */
function onEmail() {
  if (editing.value) return
  const domain = form.email.split('@')[1]?.trim()
  if (!domain) return
  if (!form.host) form.host = `imap.${domain}`
  if (!form.username) form.username = form.email.trim()
  if (!form.label) form.label = domain
}

const canSave = computed(() => editing.value || (form.label.trim() && form.host.trim() && form.username.trim() && form.password))
async function save() {
  busy.value = true
  error.value = ''
  try {
    if (props.account) {
      await api(`/api/mail/accounts/${props.account.id}`, { method: 'PUT', silent: true, body: { label: form.label, webmail_url: form.webmail, notify: form.notify, password: form.password || undefined } })
      emit('saved', { ...props.account, label: form.label, webmail_url: form.webmail, notify: form.notify })
    } else {
      const a = await api<MailAccount>('/api/mail/accounts', {
        method: 'POST',
        silent: true,
        body: { workspace_id: wsId.value, label: form.label, email: form.email || undefined, imap_host: form.host, imap_port: Number(form.port), security: form.security, username: form.username, password: form.password, webmail_url: form.webmail || undefined, notify: form.notify },
      })
      toast.success(t('mail.added'))
      emit('saved', a)
    }
    invalidate()
    open.value = false
  } catch (e) {
    error.value = errorMessage(e)
  } finally { busy.value = false }
}
</script>

<template>
  <UiDialog v-model:open="open" :title="editing ? t('mail.editTitle') : t('mail.addTitle')" :description="editing ? '' : t('mail.addHint')" :width="480">
    <div class="ma">
      <template v-if="!editing">
        <div class="ma-label">{{ t('mail.address') }}</div>
        <UiInput v-model="form.email" type="email" placeholder="prenom.nom@etu.univ.fr" autocomplete="off" @blur="onEmail" />
        <div class="ma-label">{{ t('mail.server') }}</div>
        <div class="ma-row">
          <UiInput v-model="form.host" placeholder="imap.univ.fr" autocomplete="off" />
          <UiInput v-model.number="form.port" class="ma-port" type="number" min="1" max="65535" />
        </div>
        <UiSegmented v-model="form.security" size="md" :options="[{ value: 'tls', label: 'SSL/TLS' }, { value: 'starttls', label: 'STARTTLS' }]" />
        <div class="ma-label">{{ t('mail.username') }}</div>
        <UiInput v-model="form.username" autocomplete="off" />
      </template>
      <div class="ma-label">{{ editing ? t('mail.newPassword') : t('mail.password') }}</div>
      <UiInput v-model="form.password" type="password" autocomplete="new-password" :placeholder="editing ? t('mail.keepPassword') : ''" />
      <div class="ma-label">{{ t('mail.label') }}</div>
      <UiInput v-model="form.label" :placeholder="t('mail.labelPlaceholder')" maxlength="120" />
      <div class="ma-label">{{ t('mail.webmail') }}</div>
      <UiInput v-model="form.webmail" placeholder="https://webmail.univ.fr" autocomplete="off" />
      <p class="ma-hint">{{ t('mail.webmailHint') }}</p>
      <label class="ma-switch"><UiSwitch v-model="form.notify" /><span>{{ t('mail.notify') }}</span></label>
      <p v-if="error" class="ma-err">{{ error }}</p>
    </div>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="busy" :disabled="!canSave" @click="save">{{ editing ? t('common.save') : t('mail.connect') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.ma { display: flex; flex-direction: column; gap: 8px; }
.ma-label { font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; margin-top: 6px; }
.ma-hint { margin: 0; color: var(--text-3); font-size: var(--fs-sm); }
.ma-row { display: flex; gap: 8px; }
.ma-row > :first-child { flex: 1; }
.ma-port { width: 90px; flex: none; }
.ma-switch { display: flex; align-items: center; gap: 10px; margin-top: 8px; cursor: pointer; }
.ma-err { margin: 4px 0 0; color: var(--red); font-size: var(--fs-sm); }
</style>
