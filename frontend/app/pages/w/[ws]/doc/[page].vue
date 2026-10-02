<script setup lang="ts">
import { FileText } from 'lucide-vue-next'
definePageMeta({ layout: 'workspace' })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: $t('nav.docs'), icon: FileText }]" />
    <UiEmpty :icon="FileText" title="Doc" description="Phase 4" />
  </div>
</template>
