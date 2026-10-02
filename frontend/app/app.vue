<script setup lang="ts">
import { useHead } from '#imports'

const { data: settings } = await useAsyncData('public-settings', () => $fetch('/api/settings'))

if (settings.value) {
  const headConfig: any = {
    titleTemplate: (titleChunk: string) => {
      return titleChunk ? `${titleChunk} - ${settings.value.app_name}` : settings.value.app_name
    },
    link: [],
    script: []
  }

  if (settings.value.favicon_url) {
    headConfig.link.push({ rel: 'icon', href: settings.value.favicon_url })
  }

  if (settings.value.umami_website_id && settings.value.umami_script_url) {
    headConfig.script.push({
      src: settings.value.umami_script_url,
      'data-website-id': settings.value.umami_website_id,
      async: true,
      defer: true
    })
  }

  useHead(headConfig)
}
</script>

<template>
  <NuxtPage />
</template>

<style>
body {
  margin: 0;
  padding: 0;
}
</style>
