<script setup lang="ts">
import { Copy, KeyRound } from 'lucide-vue-next'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

/** Inscription du second facteur (TOTP) : clé, code de contrôle, puis codes de secours à noter. */
const emit = defineEmits<{ done: [loggedIn: boolean] }>()
const { t } = useI18n()
const secret = ref('')
const url = ref('')
const code = ref('')
const busy = ref(false)
const error = ref('')
const recovery = ref<string[] | null>(null)
const loggedIn = ref(false)

onMounted(async () => {
  try {
    const r = await api<{ secret: string; otpauth_url: string }>('/api/auth/local/totp/setup', { method: 'POST', silent: true })
    secret.value = r.secret
    url.value = r.otpauth_url
  } catch (e) {
    error.value = errorMessage(e)
  }
})

async function confirm() {
  busy.value = true
  error.value = ''
  try {
    const r = await api<{ recovery_codes: string[]; logged_in: boolean }>('/api/auth/local/totp/confirm', { method: 'POST', body: { code: code.value }, silent: true })
    recovery.value = r.recovery_codes
    loggedIn.value = r.logged_in
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}
const grouped = computed(() => secret.value.match(/.{1,4}/g)?.join(' ') ?? '')
function copyCodes() {
  navigator.clipboard.writeText((recovery.value ?? []).join('\n'))
  toast.success(t('common.copied'))
}
</script>

<template>
  <div class="enrol">
    <template v-if="!recovery">
      <h2>{{ t('local.enrolTitle') }}</h2>
      <p class="faint">{{ t('local.enrolStep1') }}</p>
      <div class="secret" data-testid="totp-secret">{{ grouped }}</div>
      <a v-if="url" :href="url" class="faint open">{{ t('local.openInApp') }}</a>
      <p class="faint">{{ t('local.enrolStep2') }}</p>
      <form class="stack" @submit.prevent="confirm">
        <UiInput v-model="code" inputmode="numeric" autocomplete="one-time-code" maxlength="7" placeholder="123456" autofocus />
        <div v-if="error" class="error">{{ error }}</div>
        <UiButton type="submit" variant="primary" block :icon="KeyRound" :loading="busy" :disabled="!secret || code.replace(/\s/g, '').length < 6">{{ t('local.confirm') }}</UiButton>
      </form>
    </template>
    <template v-else>
      <h2>{{ t('local.recoveryTitle') }}</h2>
      <p class="faint">{{ t('local.recoveryHint') }}</p>
      <ul class="codes" data-testid="recovery-codes"><li v-for="c in recovery" :key="c">{{ c }}</li></ul>
      <div class="stack">
        <UiButton :icon="Copy" block @click="copyCodes">{{ t('local.copyCodes') }}</UiButton>
        <UiButton variant="primary" block @click="emit('done', loggedIn)">{{ t('local.savedCodes') }}</UiButton>
      </div>
    </template>
  </div>
</template>

<style scoped>
.enrol { text-align: left; display: flex; flex-direction: column; gap: 10px; }
h2 { margin: 0; font-size: var(--fs-lg); font-weight: 600; }
p { margin: 0; }
.stack { display: flex; flex-direction: column; gap: 8px; }
.secret { font-family: var(--font-mono, monospace); font-size: 16px; letter-spacing: .08em; padding: 10px 12px; border-radius: var(--r-md); background: var(--bg-hover); word-break: break-all; user-select: all; }
.open { text-decoration: underline; }
.codes { list-style: none; margin: 0; padding: 12px; display: grid; grid-template-columns: 1fr 1fr; gap: 6px 12px; font-family: var(--font-mono, monospace); border-radius: var(--r-md); background: var(--bg-hover); }
.error { padding: 8px 10px; border-radius: var(--r-md); background: var(--red-soft); color: var(--red); font-size: var(--fs-md); }
</style>
