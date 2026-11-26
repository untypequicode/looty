<script setup lang="ts">
import { LockKeyhole, Send, CheckCircle2, XCircle } from 'lucide-vue-next'
import { api, errorMessage } from '~/composables/api'
import { links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

/**
 * Page ouverte quand on suit le lien d'une page sans en avoir le droit : au lieu d'un 404 muet, on peut
 * demander l'accès. Les personnes qui gèrent la page reçoivent une notification et acceptent ou refusent.
 * Rien ici ne dit si la page existe ni ce qu'elle contient.
 */
definePageMeta({ layout: 'default' })
const route = useRoute()
const { t } = useI18n()
const pageId = computed(() => route.params.page as string)
const state = ref<'loading' | 'none' | 'pending' | 'denied' | 'ok'>('loading')
const workspaceId = ref('')
const message = ref('')
const busy = ref(false)

async function load() {
  try {
    const r = await api<{ state: 'none' | 'pending' | 'denied' | 'ok'; workspace_id?: string }>(`/api/share/access-status/${pageId.value}`, { silent: true })
    state.value = r.state
    workspaceId.value = r.workspace_id ?? ''
  } catch {
    state.value = 'none'
  }
}
async function send() {
  busy.value = true
  try {
    const r = await api<{ state: 'pending' | 'denied' | 'ok'; workspace_id?: string }>('/api/share/request-access', { method: 'POST', body: { page_id: pageId.value, message: message.value } })
    state.value = r.state
    workspaceId.value = r.workspace_id ?? ''
  } catch (e) {
    toast.error(errorMessage(e))
  } finally {
    busy.value = false
  }
}
onMounted(load)
useHead({ title: () => t('requestAccess.title') })
</script>

<template>
  <main class="ra">
    <div v-if="state === 'loading'" class="ra-box"><UiSpinner /></div>
    <section v-else class="ra-box" aria-live="polite">
      <component :is="state === 'ok' ? CheckCircle2 : state === 'denied' ? XCircle : state === 'pending' ? Send : LockKeyhole" :size="32" class="ra-icon" />
      <template v-if="state === 'none'">
        <h1>{{ t('requestAccess.title') }}</h1>
        <p>{{ t('requestAccess.hint') }}</p>
        <label class="ra-label" for="ra-msg">{{ t('requestAccess.messageLabel') }}</label>
        <textarea id="ra-msg" v-model="message" class="ra-msg" rows="3" maxlength="500" :placeholder="t('requestAccess.messagePlaceholder')" />
        <UiButton variant="primary" :loading="busy" :icon="Send" @click="send">{{ t('requestAccess.send') }}</UiButton>
      </template>
      <template v-else-if="state === 'pending'">
        <h1>{{ t('requestAccess.pendingTitle') }}</h1>
        <p>{{ t('requestAccess.pending') }}</p>
      </template>
      <template v-else-if="state === 'denied'">
        <h1>{{ t('requestAccess.deniedTitle') }}</h1>
        <p>{{ t('requestAccess.denied') }}</p>
      </template>
      <template v-else>
        <h1>{{ t('requestAccess.okTitle') }}</h1>
        <p>{{ t('requestAccess.ok') }}</p>
        <UiButton variant="primary" :to="links.doc(workspaceId, pageId)">{{ t('requestAccess.open') }}</UiButton>
      </template>
      <UiButton variant="ghost" to="/">{{ t('requestAccess.home') }}</UiButton>
    </section>
  </main>
</template>

<style scoped>
.ra { min-height: 100vh; display: grid; place-items: center; padding: 24px; }
.ra-box { width: min(440px, 100%); display: flex; flex-direction: column; align-items: stretch; gap: 12px; text-align: center; }
.ra-icon { align-self: center; color: var(--text-3); }
h1 { margin: 0; font-family: var(--font-display); font-size: 22px; }
p { margin: 0; color: var(--text-2); line-height: 1.5; }
.ra-label { text-align: left; font-size: var(--fs-sm); font-weight: 500; }
.ra-msg { width: 100%; padding: 8px 10px; border-radius: var(--r-md); border: 1px solid var(--border-strong); background: var(--bg-input); font: inherit; resize: vertical; }
</style>
