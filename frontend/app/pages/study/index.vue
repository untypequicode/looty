<script setup lang="ts">
import { usePersonalSpace } from '~/composables/usePersonal'
import { links } from '~/composables/useWorkspace'

/**
 * Ancienne adresse des révisions : les entraînements vivent dans la coque de l'espace
 * (/w/[ws]/study). On rejoint celui de l'espace perso, joignable de partout.
 */
definePageMeta({ layout: false })
const personal = usePersonalSpace()
const target = computed(() => (personal.id.value ? links.study(personal.id.value) : ''))
watch(target, (to) => { if (to) navigateTo(to, { replace: true }) }, { immediate: true })
</script>

<template>
  <div class="center"><UiSpinner /></div>
</template>

<style scoped>
.center { display: grid; place-items: center; min-height: 60vh; }
</style>
