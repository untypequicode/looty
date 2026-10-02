<script setup lang="ts">
import type { Component } from 'vue'

defineOptions({ inheritAttrs: false })
const model = defineModel<string | number | null>()
const props = withDefaults(defineProps<{
  icon?: Component
  size?: 'sm' | 'md' | 'lg'
  multiline?: boolean
  rows?: number
  invalid?: boolean
  autofocus?: boolean
}>(), { size: 'md', rows: 3 })

const el = ref<HTMLInputElement | HTMLTextAreaElement | null>(null)
// class/style vont sur le conteneur, le reste (placeholder, type, events...) sur le champ.
const attrs = useAttrs()
const rootAttrs = computed(() => ({ class: attrs.class, style: attrs.style as any }))
const fieldAttrs = computed(() => { const { class: _c, style: _s, ...rest } = attrs; return rest })
onMounted(() => { if (props.autofocus) requestAnimationFrame(() => el.value?.focus()) })
defineExpose({ focus: () => el.value?.focus(), select: () => el.value?.select(), el })
</script>

<template>
  <label class="input" :class="[`input-${size}`, { invalid, multiline }, rootAttrs.class]" :style="rootAttrs.style">
    <component :is="icon" v-if="icon" :size="14" class="input-icon" />
    <textarea v-if="multiline" ref="el" v-model="model" :rows="rows" v-bind="fieldAttrs" />
    <input v-else ref="el" v-model="model" v-bind="fieldAttrs" />
    <slot name="right" />
  </label>
</template>

<style scoped>
.input {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  background: var(--bg-input);
  border: 1px solid var(--border-strong);
  border-radius: var(--r-sm);
  padding: 0 10px;
  transition: border-color var(--dur-fast) var(--ease), box-shadow var(--dur-fast) var(--ease);
  cursor: text;
}
.input:focus-within {
  border-color: var(--border-focus);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.input.invalid { border-color: var(--red); }
.input-sm { height: 28px; font-size: var(--fs-sm); }
.input-md { height: 32px; }
.input-lg { height: 38px; font-size: var(--fs-lg); }
.input.multiline { height: auto; padding: 8px 10px; align-items: flex-start; }
.input input,
.input textarea {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  height: 100%;
  resize: vertical;
}
.input input::placeholder,
.input textarea::placeholder { color: var(--text-3); }
.input-icon { color: var(--text-3); flex: none; }
</style>
