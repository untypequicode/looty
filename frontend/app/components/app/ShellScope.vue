<script setup lang="ts">
import type { Ref } from 'vue'
import { provideWorkspaceId } from '~/composables/useWorkspace'

/** Les composants de la coque voient `ws` comme espace courant, même autour d'un contenu perso. */
const props = defineProps<{ ws: string }>()
provideWorkspaceId(toRef(props, 'ws') as Ref<string>)
</script>

<template>
  <slot />
</template>
