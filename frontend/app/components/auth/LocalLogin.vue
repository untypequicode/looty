<script setup lang="ts">
import { LogIn } from 'lucide-vue-next'
import { api, errorMessage } from '~/composables/api'
import { useI18n, currentLocale } from '~/composables/useI18n'

/** Connexion par adresse et mot de passe (comptes locaux, facultatifs), avec second facteur, inscription et réinitialisation. */
const props = defineProps<{ redirect: string; hasProviders: boolean }>()
const emit = defineEmits<{ ready: [enabled: boolean] }>()
const route = useRoute()
const { t } = useI18n()

interface LocalConfig { enabled: boolean; signup?: 'open' | 'closed'; first_account?: boolean }
const config = ref<LocalConfig>({ enabled: false })
type Mode = 'login' | 'totp' | 'enrol' | 'register' | 'forgot' | 'reset' | 'invite' | 'message'
const mode = ref<Mode>('login')
const email = ref('')
const name = ref('')
const password = ref('')
const code = ref('')
const setupCode = ref('')
const busy = ref(false)
const error = ref('')
const message = ref('')
const open = ref(false)
const token = ref('')

const safeRedirect = computed(() => (props.redirect.startsWith('/') && !props.redirect.startsWith('//') ? props.redirect : '/'))
const go = () => { window.location.href = safeRedirect.value }

onMounted(async () => {
  try { config.value = await api<LocalConfig>('/api/auth/local/config', { silent: true }) } catch { return }
  emit('ready', config.value.enabled)
  if (!config.value.enabled) return
  const q = route.query
  if (typeof q.invite === 'string') {
    token.value = q.invite
    try {
      const r = await api<{ email: string; name: string }>(`/api/auth/local/invite/${encodeURIComponent(q.invite)}`, { silent: true })
      email.value = r.email
      name.value = r.name
      mode.value = 'invite'
    } catch (e) { return say(errorMessage(e)) }
  } else if (typeof q.reset === 'string') {
    token.value = q.reset
    mode.value = 'reset'
  } else if (typeof q.verify === 'string') {
    try {
      await api('/api/auth/local/verify', { method: 'POST', body: { token: q.verify }, silent: true })
      say(t('local.verified'))
    } catch (e) { say(errorMessage(e)) }
  } else if (config.value.first_account) {
    mode.value = 'register'
  }
  open.value = !props.hasProviders || mode.value !== 'login' || !!config.value.first_account
})

function say(text: string) { message.value = text; mode.value = 'message' }

async function run(fn: () => Promise<void>) {
  busy.value = true
  error.value = ''
  try { await fn() } catch (e) {
    const status = (e as any)?.status ?? (e as any)?.statusCode
    error.value = status === 401 ? t('local.badCredentials') : errorMessage(e)
  } finally { busy.value = false }
}

function next(status: string) {
  password.value = ''
  code.value = ''
  if (status === 'ok') return go()
  if (status === 'totp') mode.value = 'totp'
  else if (status === 'totp_enrol') mode.value = 'enrol'
  else if (status === 'verify_email') say(t('local.registerSent'))
  else if (status === 'pending_admin') say(t('local.registerPending'))
}

const login = () => run(async () => next((await api<{ status: string }>('/api/auth/local/login', { method: 'POST', body: { email: email.value, password: password.value }, silent: true })).status))
const sendCode = () => run(async () => next((await api<{ status: string }>('/api/auth/local/totp', { method: 'POST', body: { code: code.value }, silent: true })).status))
const register = () => run(async () => next((await api<{ status: string }>('/api/auth/local/register', { method: 'POST', body: { email: email.value, name: name.value, password: password.value, locale: currentLocale(), setup_code: setupCode.value || undefined }, silent: true })).status))
const forgot = () => run(async () => { await api('/api/auth/local/forgot', { method: 'POST', body: { email: email.value }, silent: true }); say(t('local.forgotSent')) })
const reset = () => run(async () => { await api('/api/auth/local/reset', { method: 'POST', body: { token: token.value, password: password.value }, silent: true }); password.value = ''; say(t('local.resetDone')) })
const accept = () => run(async () => next((await api<{ status: string }>('/api/auth/local/accept-invite', { method: 'POST', body: { token: token.value, name: name.value, password: password.value, locale: currentLocale() }, silent: true })).status))
function back() { mode.value = 'login'; error.value = ''; message.value = '' }
const firstAccount = computed(() => config.value.first_account && mode.value === 'register')
</script>

<template>
  <div v-if="config.enabled" class="local">
    <button v-if="!open && mode === 'login'" class="toggle" @click="open = true">{{ t('local.useEmail') }}</button>
    <template v-else>
      <div v-if="hasProviders && mode === 'login'" class="or faint">{{ t('local.or') }}</div>
      <p v-if="firstAccount" class="faint first">{{ t('local.firstHint') }}</p>
      <div v-if="error" class="error" role="alert">{{ error }}</div>

      <form v-if="mode === 'login'" class="stack" @submit.prevent="login">
        <UiInput v-model="email" type="email" autocomplete="username" :placeholder="t('local.email')" />
        <UiInput v-model="password" type="password" autocomplete="current-password" :placeholder="t('local.password')" />
        <UiButton type="submit" variant="primary" block :icon="LogIn" :loading="busy" :disabled="!email || !password">{{ t('auth.login') }}</UiButton>
        <div class="links">
          <button type="button" class="link" @click="mode = 'forgot'">{{ t('local.forgot') }}</button>
          <button v-if="config.signup === 'open'" type="button" class="link" @click="mode = 'register'">{{ t('local.createAccount') }}</button>
        </div>
      </form>

      <form v-else-if="mode === 'totp'" class="stack" @submit.prevent="sendCode">
        <p class="faint">{{ t('local.totpPrompt') }}</p>
        <UiInput v-model="code" autocomplete="one-time-code" :placeholder="t('local.code')" autofocus />
        <UiButton type="submit" variant="primary" block :loading="busy" :disabled="code.replace(/[\s-]/g, '').length < 6">{{ t('local.verify') }}</UiButton>
      </form>

      <AuthTotpEnrol v-else-if="mode === 'enrol'" @done="go" />

      <form v-else-if="mode === 'register'" class="stack" @submit.prevent="register">
        <h2 v-if="firstAccount">{{ t('local.firstTitle') }}</h2>
        <UiInput v-model="name" autocomplete="name" :placeholder="t('local.name')" />
        <UiInput v-model="email" type="email" autocomplete="username" :placeholder="t('local.email')" />
        <UiInput v-model="password" type="password" autocomplete="new-password" :placeholder="t('local.password')" />
        <p class="faint small">{{ t('local.passwordHint') }}</p>
        <template v-if="firstAccount">
          <UiInput v-model="setupCode" autocomplete="off" placeholder="XXXX-XXXX" />
          <p class="faint small">{{ t('local.setupCodeHint') }}</p>
        </template>
        <UiButton type="submit" variant="primary" block :loading="busy" :disabled="!email || !password || (firstAccount && !setupCode)">{{ t('local.createAccount') }}</UiButton>
        <button v-if="!firstAccount" type="button" class="link" @click="back">{{ t('local.backToSignIn') }}</button>
      </form>

      <form v-else-if="mode === 'forgot'" class="stack" @submit.prevent="forgot">
        <p class="faint">{{ t('local.forgotHint') }}</p>
        <UiInput v-model="email" type="email" autocomplete="username" :placeholder="t('local.email')" />
        <UiButton type="submit" variant="primary" block :loading="busy" :disabled="!email">{{ t('local.sendLink') }}</UiButton>
        <button type="button" class="link" @click="back">{{ t('local.backToSignIn') }}</button>
      </form>

      <form v-else-if="mode === 'reset'" class="stack" @submit.prevent="reset">
        <h2>{{ t('local.resetTitle') }}</h2>
        <UiInput v-model="password" type="password" autocomplete="new-password" :placeholder="t('local.newPassword')" />
        <p class="faint small">{{ t('local.passwordHint') }}</p>
        <UiButton type="submit" variant="primary" block :loading="busy" :disabled="!password">{{ t('local.resetSubmit') }}</UiButton>
      </form>

      <form v-else-if="mode === 'invite'" class="stack" @submit.prevent="accept">
        <h2>{{ t('local.inviteTitle') }}</h2>
        <p class="faint">{{ email }}</p>
        <UiInput v-model="name" autocomplete="name" :placeholder="t('local.name')" />
        <UiInput v-model="password" type="password" autocomplete="new-password" :placeholder="t('local.password')" />
        <p class="faint small">{{ t('local.passwordHint') }}</p>
        <UiButton type="submit" variant="primary" block :loading="busy" :disabled="!password">{{ t('local.inviteSubmit') }}</UiButton>
      </form>

      <div v-else class="stack">
        <p>{{ message }}</p>
        <UiButton block @click="back">{{ t('local.backToSignIn') }}</UiButton>
      </div>
    </template>
  </div>
</template>

<style scoped>
.local { margin-top: 12px; text-align: left; display: flex; flex-direction: column; gap: 10px; }
.stack { display: flex; flex-direction: column; gap: 8px; }
h2 { margin: 0; font-size: var(--fs-lg); font-weight: 600; }
p { margin: 0; }
.small { font-size: var(--fs-sm); }
.first { text-align: center; }
.or { text-align: center; font-size: var(--fs-sm); }
.toggle, .link { background: none; border: 0; padding: 0; color: var(--accent); cursor: pointer; font: inherit; font-size: var(--fs-md); text-align: center; }
.toggle { align-self: center; }
.link:hover, .toggle:hover { text-decoration: underline; }
.links { display: flex; justify-content: space-between; }
.error { padding: 8px 10px; border-radius: var(--r-md); background: var(--red-soft); color: var(--red); font-size: var(--fs-md); }
</style>
