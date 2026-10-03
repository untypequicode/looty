<script setup lang="ts">
defineProps<{ title?: string; description?: string; danger?: boolean }>()
</script>

<template>
  <section class="ss" :class="{ danger }">
    <header v-if="title || description || $slots.actions" class="ss-head">
      <div>
        <h2 v-if="title">{{ title }}</h2>
        <p v-if="description">{{ description }}</p>
      </div>
      <div class="ss-actions"><slot name="actions" /></div>
    </header>
    <div class="ss-card"><slot /></div>
  </section>
</template>

<style scoped>
.ss + .ss { margin-top: 32px; }
.ss-head { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; margin-bottom: 10px; }
.ss-head h2 { margin: 0; font-size: var(--fs-lg); font-weight: 600; }
.ss-head p { margin: 4px 0 0; color: var(--text-3); font-size: var(--fs-md); line-height: 1.5; }
.ss-actions { display: flex; gap: 8px; flex: none; }
.ss-card { border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); overflow: hidden; }
.ss.danger .ss-card { box-shadow: 0 0 0 1px color-mix(in srgb, var(--red) 35%, transparent); }
</style>
