<script setup lang="ts">
import { usePages } from '~/composables/usePages'

/**
 * Documents de l'espace perso dans la barre latérale d'un autre espace : la même arborescence
 * dépliable que la section Documents. Rendu dans `AppShellScope` sur l'espace perso, afin que
 * liens, menus et glisser-déposer s'appliquent à lui.
 */
const { tree } = usePages()
</script>

<template>
  <div class="sb-group tree">
    <AppPageTreeItem v-for="n in tree" :key="n.id" :node="n" />
  </div>
</template>
