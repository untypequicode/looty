<script setup lang="ts">
import { useDemo } from '~/composables/useDemo'
import { useI18n } from '~/composables/useI18n'

// Entoure une fonction indisponible dans la démo : visible, mais inactive, avec la mention (`when` : seulement si vrai).
const props = withDefaults(defineProps<{ when?: boolean }>(), { when: true })
const { isDemo } = useDemo()
const { t } = useI18n()
</script>

<template>
  <div v-if="isDemo && props.when" class="demo-lock">
    <div class="demo-lock-note" role="status">{{ t('demo.unavailable') }}</div>
    <div class="demo-lock-body" inert aria-disabled="true"><slot /></div>
  </div>
  <slot v-else />
</template>

<style scoped>
.demo-lock-note {
  display: inline-block; margin-bottom: 10px; padding: 3px 10px; border-radius: var(--r-md);
  background: var(--accent-soft); color: var(--accent); font-size: var(--fs-sm); font-weight: 500;
}
.demo-lock-body { opacity: .5; filter: grayscale(.4); user-select: none; }
</style>
