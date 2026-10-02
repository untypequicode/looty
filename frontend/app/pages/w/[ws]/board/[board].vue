<script setup lang="ts">
import { Kanban } from 'lucide-vue-next'
definePageMeta({ layout: 'workspace' })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: $t('nav.boards'), icon: Kanban }]" />
    <UiEmpty :icon="Kanban" title="Board" description="Phase 3" />
  </div>
</template>
