<script setup lang="ts">
const model = defineModel<boolean>({ default: false })
defineProps<{ disabled?: boolean; label?: string }>()
</script>

<template>
  <button
    type="button"
    role="switch"
    class="switch"
    :class="{ on: model }"
    :aria-checked="model"
    :aria-label="label"
    :disabled="disabled"
    @click="model = !model"
  >
    <span class="knob" />
  </button>
</template>

<style scoped>
.switch {
  position: relative;
  width: 30px;
  height: 18px;
  flex: none;
  border-radius: var(--r-full);
  background: var(--border-strong);
  transition: background var(--dur) var(--ease);
}
.switch.on { background: var(--accent); }
.switch:disabled { opacity: 0.5; cursor: not-allowed; }
.knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
  transition: transform var(--dur) var(--ease-out);
}
.switch.on .knob { transform: translateX(12px); }
</style>
