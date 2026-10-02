<script setup lang="ts">
import { useQuery } from '@tanstack/vue-query'
import { motion } from '~/composables/useMotion'
import { api } from '~/composables/api'

const { data: settings } = useQuery({
  queryKey: ['public-settings'],
  queryFn: () => api<{ app_name: string; favicon_url?: string | null; umami_website_id?: string | null; umami_script_url?: string | null }>('/api/settings', { silent: true }),
  staleTime: 10 * 60_000,
})

useHead(() => ({
  titleTemplate: (chunk?: string) => {
    const name = settings.value?.app_name || 'Looty'
    return chunk ? `${chunk} · ${name}` : name
  },
  link: settings.value?.favicon_url ? [{ rel: 'icon', href: settings.value.favicon_url }] : [],
  script:
    settings.value?.umami_website_id && settings.value?.umami_script_url
      ? [{ src: settings.value.umami_script_url, 'data-website-id': settings.value.umami_website_id, defer: true }]
      : [],
}))

const pageTransition = { css: false, mode: 'out-in' as const, onEnter: motion.page.onEnter, onLeave: motion.page.onLeave }
</script>

<template>
  <NuxtLayout>
    <NuxtPage :transition="pageTransition" />
  </NuxtLayout>
  <UiToaster />
  <UiConfirmHost />
</template>
