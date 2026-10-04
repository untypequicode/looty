<script setup lang="ts">
import type { WorkspaceMember, Presence } from '~/types'
import { useLocationInfo } from '~/composables/usePresenceInfo'

const props = defineProps<{ member: WorkspaceMember; presence: Presence }>()
const emit = defineEmits<{ select: [] }>()
const loc = useLocationInfo(computed(() => props.presence.location))
</script>

<template>
  <button class="pr" @click="emit('select')">
    <UiAvatar :name="member.name" :src="member.avatar_url" :size="24" online :class="{ idle: presence.status === 'idle' }" />
    <span class="pr-main">
      <span class="pr-name truncate">{{ member.name }}</span>
      <span v-if="loc" class="pr-loc truncate"><component :is="loc.icon" :size="11" /> {{ loc.label }}</span>
    </span>
  </button>
</template>

<style scoped>
.pr { display: flex; align-items: center; gap: 10px; width: 100%; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; }
.pr:hover { background: var(--bg-hover); }
.pr-main { display: flex; flex-direction: column; min-width: 0; }
.pr-name { font-size: var(--fs-md); }
.pr-loc { display: flex; align-items: center; gap: 4px; font-size: var(--fs-sm); color: var(--text-3); }
.idle { opacity: .55; }
</style>
