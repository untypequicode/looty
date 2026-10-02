<script setup lang="ts">
import { Settings } from 'lucide-vue-next'
definePageMeta({ layout: 'workspace' })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: $t('nav.settings'), icon: Settings }]" />
    <UiEmpty :icon="Settings" title="Settings" description="Phase 5" />
  </div>
</template>
