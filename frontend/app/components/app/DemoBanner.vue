<script setup lang="ts">
import { useQuery } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useAuth } from '~/composables/useAuth'
import { useDemo } from '~/composables/useDemo'
import { useI18n, currentLocale } from '~/composables/useI18n'

// Bandeau permanent de la démo : prévient de l'effacement et permet de changer de personnage dans la même copie.
const { isDemo } = useDemo()
const { t } = useI18n()
const { user } = useAuth()
const { data: cast } = useQuery({
  queryKey: ['demo-characters'],
  queryFn: () => api<{ key: string; name: string; role: string }[]>('/api/auth/demo/characters', { silent: true }),
  enabled: isDemo,
})
const current = computed(() => cast.value?.find((c) => c.name === user.value?.name)?.key ?? '')

// Hauteur réservée au bandeau : les mises en page plein écran la retirent de leur hauteur.
onMounted(() => watchEffect(() => document.documentElement.style.setProperty('--demo-banner-h', isDemo.value ? '32px' : '0px')))

async function switchTo(e: Event) {
  const key = (e.target as HTMLSelectElement).value
  if (!key) return
  await api('/api/auth/demo/enter', { method: 'POST', body: { character: key, lang: currentLocale() } })
  window.location.href = '/'
}
</script>

<template>
  <div v-if="isDemo" class="demo-banner" role="note">
    <span>{{ t('demo.banner') }}</span>
    <select v-if="cast?.length" :value="current" :aria-label="t('demo.switch')" @change="switchTo">
      <option v-for="c in cast" :key="c.key" :value="c.key">{{ c.name }} · {{ t(`demo.roles.${c.role}`) }}</option>
    </select>
    <a href="https://looty.untypequicode.fr" target="_blank" rel="noopener">{{ t('demo.host') }}</a>
  </div>
</template>

<style scoped>
.demo-banner {
  --demo-banner-h: 32px;
  display: flex; gap: 12px; justify-content: center; align-items: center; height: var(--demo-banner-h); overflow: hidden; white-space: nowrap;
  padding: 0 12px; background: var(--accent); color: #fff; font-size: var(--fs-sm); text-align: center;
}
.demo-banner a { color: #fff; font-weight: 600; text-decoration: underline; }
.demo-banner select { background: rgba(255, 255, 255, .18); color: #fff; border: 0; border-radius: var(--r-md); padding: 2px 6px; font: inherit; }
.demo-banner option { color: #000; }
</style>
