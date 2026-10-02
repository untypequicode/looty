<script setup lang="ts">
const props = withDefaults(defineProps<{
  name?: string | null
  src?: string | null
  size?: number
  online?: boolean
  ring?: boolean
}>(), { size: 20 })

const PALETTE = ['#7170ff', '#4cb782', '#f2994a', '#eb5757', '#4ea7fc', '#b18cff', '#f2c94c', '#26b5ce', '#e06ba8']
const color = computed(() => {
  const n = props.name || '?'
  let h = 0
  for (let i = 0; i < n.length; i++) h = (h * 31 + n.charCodeAt(i)) >>> 0
  return PALETTE[h % PALETTE.length]
})
const initials = computed(() => {
  const parts = (props.name || '?').trim().split(/\s+/)
  return ((parts[0]?.[0] ?? '?') + (parts.length > 1 ? parts[parts.length - 1][0] : '')).toUpperCase()
})
const failed = ref(false)
watch(() => props.src, () => (failed.value = false))
</script>

<template>
  <span
    class="avatar"
    :class="{ ring }"
    :style="{ width: `${size}px`, height: `${size}px`, fontSize: `${Math.max(8, Math.round(size * 0.42))}px`, background: src && !failed ? 'transparent' : color }"
    :title="name || undefined"
  >
    <img v-if="src && !failed" :src="src" :alt="name || ''" @error="failed = true" />
    <span v-else>{{ size >= 22 ? initials : initials[0] }}</span>
    <span v-if="online" class="dot" :style="{ width: `${Math.max(6, size * 0.32)}px`, height: `${Math.max(6, size * 0.32)}px` }" />
  </span>
</template>

<style scoped>
.avatar {
  position: relative;
  flex: none;
  display: inline-grid;
  place-items: center;
  border-radius: 50%;
  color: #fff;
  font-weight: 600;
  letter-spacing: 0;
  user-select: none;
}
.avatar.ring { box-shadow: 0 0 0 2px var(--bg); }
.avatar img { width: 100%; height: 100%; border-radius: 50%; object-fit: cover; }
.dot {
  position: absolute;
  right: -1px;
  bottom: -1px;
  border-radius: 50%;
  background: var(--green);
  box-shadow: 0 0 0 2px var(--bg-sidebar);
}
</style>
