<script setup lang="ts">
import { LogIn, Sparkles } from 'lucide-vue-next'
import gsap from 'gsap'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'

const route = useRoute()
const { t } = useI18n()
const providers = ref<{ id: string; name: string }[]>([])
const loading = ref(true)
const redirecting = ref<string | null>(null)
const card = ref<HTMLElement | null>(null)

const error = computed(() => {
  const e = route.query.error as string | undefined
  if (!e) return ''
  const key = `auth.errors.${e}`
  const msg = t(key)
  return msg === key ? t('auth.errors.generic') : msg
})

onMounted(async () => {
  try {
    providers.value = await api('/api/auth/providers', { silent: true })
  } finally {
    loading.value = false
  }
  await nextTick()
  if (card.value) gsap.fromTo(card.value.children, { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.5, stagger: 0.07, ease: 'power3.out' })
})

async function login(id: string) {
  redirecting.value = id
  try {
    const redirect = typeof route.query.redirect === 'string' ? route.query.redirect : '/'
    const { url } = await api<{ url: string }>(`/api/auth/login/${id}?redirect=${encodeURIComponent(redirect)}`)
    window.location.href = url
  } catch {
    redirecting.value = null
  }
}
useHead({ title: () => t('auth.login') })
</script>

<template>
  <div class="login">
    <div class="glow" />
    <div ref="card" class="card">
      <div class="logo"><Sparkles :size="22" /></div>
      <h1>{{ t('auth.title') }}</h1>
      <p class="muted">{{ t('auth.subtitle') }}</p>
      <div v-if="error" class="error">{{ error }}</div>
      <div class="providers">
        <template v-if="loading">
          <div class="skeleton" style="height: 38px" />
        </template>
        <template v-else-if="providers.length">
          <UiButton
            v-for="p in providers"
            :key="p.id"
            size="lg"
            block
            :icon="LogIn"
            :loading="redirecting === p.id"
            :disabled="!!redirecting"
            @click="login(p.id)"
          >
            {{ t('auth.continueWith', { provider: p.name }) }}
          </UiButton>
        </template>
        <p v-else class="faint">{{ t('auth.noProvider') }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.login { min-height: 100dvh; display: grid; place-items: center; padding: 24px; position: relative; overflow: hidden; }
.glow {
  position: absolute;
  width: 900px; height: 900px; top: -520px; left: 50%; transform: translateX(-50%);
  background: radial-gradient(closest-side, var(--accent-soft), transparent);
  pointer-events: none;
}
.card { position: relative; width: 100%; max-width: 360px; text-align: center; }
.logo {
  width: 48px; height: 48px; margin: 0 auto 22px; border-radius: 14px;
  display: grid; place-items: center; color: #fff;
  background: linear-gradient(135deg, var(--accent), #b18cff);
  box-shadow: 0 10px 30px var(--accent-soft);
}
h1 { margin: 0; font-size: 22px; font-weight: 600; letter-spacing: -0.02em; font-family: var(--font-display); }
.muted { margin: 8px 0 28px; }
.providers { display: flex; flex-direction: column; gap: 8px; }
.error {
  margin-bottom: 16px; padding: 10px 12px; border-radius: var(--r-md);
  background: var(--red-soft); color: var(--red); font-size: var(--fs-md);
}
</style>
