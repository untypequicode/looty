<script setup lang="ts">
import { Sparkles, Check, ChevronRight, Copy, Plus, Stethoscope } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { setupFlag } from '~/composables/useSetup'
import { toast } from '~/composables/useToast'
import { links, useWorkspaces } from '~/composables/useWorkspace'

/** Assistant de premier lancement (issue 163) : chaque étape reste disponible dans l'administration. */
const { t } = useI18n()
const { user, isLoading } = useAuth()
const STEPS = ['signin', 'instance', 'email', 'push', 'space', 'finish'] as const
type Step = (typeof STEPS)[number]
const step = ref<Step>('signin')
const index = computed(() => STEPS.indexOf(step.value))
const { data: state } = useQuery({ queryKey: ['setup', 'state'], queryFn: () => api<{ local_accounts: boolean; oidc_providers: number }>('/api/setup/state', { silent: true }) })

watchEffect(() => { if (!isLoading.value && user.value && !user.value.is_super_admin) navigateTo('/') })

const { data: workspaces } = useWorkspaces()
// Les pages d'administration vivent sous un espace : le premier créé ici, ou n'importe lequel de la personne.
const anySpace = computed(() => spaceId.value ?? workspaces.value?.[0]?.id ?? '')
const settingsLink = (section: string) => (anySpace.value ? links.settings(anySpace.value, section) : null)

const next = () => { step.value = STEPS[Math.min(STEPS.length - 1, index.value + 1)]! }
const prev = () => { step.value = STEPS[Math.max(0, index.value - 1)]! }

// Premier espace et invitations.
const spaceName = ref('')
const spaceId = ref<string | null>(null)
const creating = ref(false)
const inviteLinks = ref<string[]>([])
async function createSpace() {
  creating.value = true
  try {
    const r = await api<{ workspace: { id: string } }>('/api/workspaces', { method: 'POST', body: { name: spaceName.value.trim() } })
    spaceId.value = r.workspace.id
  } finally { creating.value = false }
}
async function newInvite() {
  const r = await api<{ token: string }>(`/api/workspaces/${spaceId.value}/invites`, { method: 'POST', body: { role: 'member', expires_in_days: 7 } })
  inviteLinks.value.push(`${location.origin}/invite/${r.token}`)
}
function copy(text: string) { navigator.clipboard.writeText(text); toast.success(t('common.linkCopied')) }

const finishing = ref(false)
async function finish(target: 'home' | 'doctor') {
  finishing.value = true
  try {
    await api('/api/setup/complete', { method: 'POST' })
    setupFlag.needed = false
    setupFlag.checked = true
    await navigateTo((target === 'doctor' && settingsLink('admin-doctor')) || '/')
  } finally { finishing.value = false }
}
const https = computed(() => (import.meta.client ? location.protocol === 'https:' : true))
useHead({ title: () => t('setup.title') })
definePageMeta({ layout: false })
</script>

<template>
  <div class="setup">
    <div class="card">
      <div class="logo"><Sparkles :size="22" /></div>
      <h1>{{ t('setup.title') }}</h1>
      <p class="muted">{{ t('setup.subtitle') }}</p>
      <ol class="dots"><li v-for="(s, i) in STEPS" :key="s" :class="{ on: i === index, done: i < index }">{{ i < index ? '✓' : i + 1 }}</li></ol>

      <section v-if="step === 'signin'" class="step">
        <h2>{{ t('setup.signin.title') }}</h2>
        <p class="ok"><Check :size="15" /> {{ t('setup.signin.admin', { email: user?.email ?? '' }) }}</p>
        <p>{{ state?.oidc_providers ? t('setup.signin.oidc', { count: state.oidc_providers }) : t('setup.signin.noOidc') }}</p>
        <p>{{ state?.local_accounts ? t('setup.signin.local') : t('setup.signin.noLocal') }}</p>
        <NuxtLink v-if="settingsLink('admin-auth')" :to="settingsLink('admin-auth')!" class="aside">{{ t('setup.signin.manage') }}</NuxtLink>
      </section>

      <section v-else-if="step === 'instance'" class="step wide">
        <h2>{{ t('setup.instance.title') }}</h2>
        <p class="faint">{{ t('setup.instance.hint') }}</p>
        <SettingsAdminInstance />
      </section>

      <section v-else-if="step === 'email'" class="step wide">
        <h2>{{ t('setup.email.title') }}</h2>
        <p class="faint">{{ t('setup.email.hint') }}</p>
        <SettingsAdminEmail />
      </section>

      <section v-else-if="step === 'push'" class="step">
        <h2>{{ t('setup.push.title') }}</h2>
        <p>{{ t('setup.push.hint') }}</p>
        <p :class="https ? 'ok' : 'bad'">{{ https ? t('setup.push.https') : t('setup.push.noHttps') }}</p>
      </section>

      <section v-else-if="step === 'space'" class="step">
        <h2>{{ t('setup.space.title') }}</h2>
        <template v-if="!spaceId">
          <p class="faint">{{ t('setup.space.hint') }}</p>
          <UiInput v-model="spaceName" :placeholder="t('setup.space.placeholder')" maxlength="80" @keyup.enter="spaceName.trim() && createSpace()" />
          <UiButton variant="primary" :icon="Plus" :loading="creating" :disabled="!spaceName.trim()" @click="createSpace">{{ t('setup.space.create') }}</UiButton>
        </template>
        <template v-else>
          <p class="ok"><Check :size="15" /> {{ t('setup.space.created', { name: spaceName }) }}</p>
          <p class="faint">{{ t('setup.space.inviteHint') }}</p>
          <div v-for="l in inviteLinks" :key="l" class="link"><code>{{ l }}</code><UiButton size="sm" :icon="Copy" @click="copy(l)">{{ t('common.copy') }}</UiButton></div>
          <UiButton :icon="Plus" @click="newInvite">{{ t('setup.space.invite') }}</UiButton>
        </template>
      </section>

      <section v-else class="step">
        <h2>{{ t('setup.finish.title') }}</h2>
        <p>{{ t('setup.finish.hint') }}</p>
        <UiButton :icon="Stethoscope" :loading="finishing" @click="finish('doctor')">{{ t('setup.finish.doctor') }}</UiButton>
        <UiButton variant="primary" :loading="finishing" @click="finish('home')">{{ t('setup.finish.done') }}</UiButton>
      </section>

      <div v-if="step !== 'finish'" class="nav">
        <UiButton v-if="index > 0" variant="ghost" @click="prev">{{ t('setup.back') }}</UiButton>
        <span class="grow" />
        <UiButton variant="ghost" :loading="finishing" @click="finish('home')">{{ t('setup.skip') }}</UiButton>
        <UiButton variant="primary" :icon-right="ChevronRight" @click="next">{{ t('setup.next') }}</UiButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.setup { min-height: 100dvh; display: grid; place-items: start center; padding: 48px 20px; }
.card { width: 100%; max-width: 720px; }
.logo { width: 48px; height: 48px; margin: 0 auto 16px; border-radius: 14px; display: grid; place-items: center; color: #fff; background: linear-gradient(135deg, var(--accent), #b18cff); }
h1 { margin: 0; text-align: center; font-size: 22px; font-weight: 600; font-family: var(--font-display); }
.muted { margin: 8px 0 20px; text-align: center; }
.dots { display: flex; justify-content: center; gap: 8px; list-style: none; padding: 0; margin: 0 0 24px; }
.dots li { width: 26px; height: 26px; border-radius: 50%; display: grid; place-items: center; font-size: 12px; background: var(--bg-hover); }
.dots li.on { background: var(--accent); color: #fff; }
.dots li.done { background: var(--accent-soft); color: var(--accent); }
.step { display: flex; flex-direction: column; gap: 10px; }
.step h2 { margin: 0; font-size: var(--fs-lg); }
.step p { margin: 0; }
.ok { color: var(--green, inherit); display: flex; align-items: center; gap: 6px; }
.bad { color: var(--red); }
.aside { color: var(--accent); font-size: var(--fs-md); }
.link { display: flex; gap: 8px; align-items: center; }
.link code { flex: 1; overflow-wrap: anywhere; font-size: var(--fs-sm); padding: 6px 8px; border-radius: 6px; background: var(--bg-hover); }
.nav { display: flex; gap: 8px; margin-top: 28px; align-items: center; }
.grow { flex: 1; }
</style>
