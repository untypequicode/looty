<script setup lang="ts">
import { useI18n } from '~/composables/useI18n'

/** Avancement d'un cours Moodle (pourcentage d'activités terminées). */
const props = withDefaults(defineProps<{ value: number; width?: number }>(), { width: 64 })
const { t } = useI18n()
const n = computed(() => Math.round(props.value))
</script>

<template>
  <span class="pb" :title="t('moodle.progress', { n })">
    <span class="pb-track" :style="{ width: `${width}px` }"><span class="pb-fill" :style="{ width: `${value}%` }" /></span>
    <small>{{ n }} %</small>
  </span>
</template>

<style scoped>
.pb { display: flex; align-items: center; gap: 6px; flex: none; }
.pb small { font-size: var(--fs-xs); color: var(--text-3); min-width: 34px; text-align: right; }
.pb-track { height: 5px; border-radius: 3px; background: var(--bg-active); overflow: hidden; }
.pb-fill { display: block; height: 100%; background: #2a78d6; border-radius: 3px; }
</style>
