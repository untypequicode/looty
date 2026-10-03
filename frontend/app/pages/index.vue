<script setup lang="ts">
import { useQueryClient } from '@tanstack/vue-query'
import { Sparkles } from 'lucide-vue-next'
import gsap from 'gsap'
import type { Workspace } from '~/types'
import { api } from '~/composables/api'
import { links, wsKeys } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'

const qc = useQueryClient()
const { user, logout } = useAuth()
const { t } = useI18n()
const ready = ref(false)
const name = ref('')
const creating = ref(false)
const card = ref<HTMLElement | null>(null)

onMounted(async () => {
  const list = await qc.fetchQuery({
    queryKey: wsKeys.list,
    queryFn: async () => (await api<{ workspaces: Workspace[] }>('/api/workspaces')).workspaces,
  })
  const last = localStorage.getItem('looty.lastWorkspace')
  const target = list.find((w) => w.id === last) ?? list[0]
  if (target) return navigateTo(links.home(target.id), { replace: true })
  ready.value = true
  await nextTick()
  if (card.value) gsap.fromTo(card.value.children, { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 0.5, stagger: 0.06, ease: 'power3.out' })
})

async function create() {
  if (!name.value.trim()) return
  creating.value = true
  try {
    const r = await api<{ workspace: Workspace }>('/api/workspaces', { method: 'POST', body: { name: name.value.trim() } })
    await qc.invalidateQueries({ queryKey: wsKeys.list })
    navigateTo(links.home(r.workspace.id))
  } finally {
    creating.value = false
  }
}
</script>

<template>
  <div class="onboard">
    <div v-if="!ready" class="center"><UiSpinner :size="20" /></div>
    <div v-else ref="card" class="card">
      <div class="logo"><Sparkles :size="22" /></div>
      <h1>{{ t('onboarding.welcome', { name: user?.name?.split(' ')[0] ?? '' }) }}</h1>
      <p class="muted">{{ t('onboarding.subtitle') }}</p>
      <form class="form" @submit.prevent="create">
        <label class="field-label">{{ t('workspace.name') }}</label>
        <UiInput v-model="name" :placeholder="t('workspace.namePlaceholder')" size="lg" autofocus />
        <UiButton type="submit" variant="primary" size="lg" block :loading="creating" :disabled="!name.trim()">{{ t('workspace.create') }}</UiButton>
      </form>
      <p class="faint hint">{{ t('onboarding.inviteHint') }}</p>
      <button class="faint logout" @click="logout">{{ t('auth.logout') }}</button>
    </div>
  </div>
</template>

<style scoped>
.onboard {
  min-height: 100dvh;
  display: grid;
  place-items: center;
  padding: 24px;
  background: radial-gradient(1200px 600px at 50% -10%, var(--accent-soft), transparent 60%), var(--bg);
}
.center { display: grid; place-items: center; }
.card { width: 100%; max-width: 400px; text-align: center; }
.logo {
  width: 48px; height: 48px; margin: 0 auto 20px; border-radius: 14px;
  display: grid; place-items: center; color: #fff;
  background: linear-gradient(135deg, var(--accent), #b18cff);
  box-shadow: 0 10px 30px var(--accent-soft);
}
h1 { margin: 0; font-size: 24px; font-weight: 600; letter-spacing: -0.02em; font-family: var(--font-display); }
.muted { margin: 8px 0 28px; }
.form { display: flex; flex-direction: column; gap: 12px; text-align: left; }
.hint { margin-top: 20px; font-size: var(--fs-sm); line-height: 1.5; }
.logout { margin-top: 12px; font-size: var(--fs-sm); text-decoration: underline; }
</style>
