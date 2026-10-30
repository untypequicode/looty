<script setup lang="ts">
import { Menu, PanelLeftOpen, ChevronRight, ArrowLeft, Lock } from 'lucide-vue-next'
import { usePersonalBorrowed } from '~/composables/usePersonal'
import { useWorkspaceId, links } from '~/composables/useWorkspace'
import { isNativeApp, nativeBack } from '~/composables/useNative'
import type { Component } from 'vue'
import { useUi } from '~/composables/useUi'
import { isPane } from '~/composables/usePane'
const NuxtLink = resolveComponent('NuxtLink')

export interface Crumb {
  label: string
  to?: string
  icon?: Component
  emoji?: string | null
}

defineProps<{ crumbs?: Crumb[]; title?: string }>()
const { sidebarOpen, sidebarCollapsed } = useUi()
// Volet secondaire de l'écran partagé : pas de barre latérale à ouvrir.
const pane = isPane
// Élément perso ouvert depuis un autre espace : repère cliquable vers l'accueil perso.
const borrowed = usePersonalBorrowed()
const wsId = useWorkspaceId()
</script>

<template>
  <header class="ph">
    <!-- Application Android : retour vers l'écran natif au lieu du menu latéral. -->
    <UiButton v-if="isNativeApp" class="ph-menu ph-back" variant="ghost" size="sm" :icon="ArrowLeft" :aria-label="$t('common.back')" @click="nativeBack" />
    <UiButton v-else-if="!pane" class="ph-menu" variant="ghost" size="sm" :icon="Menu" :aria-label="$t('nav.openMenu')" @click="sidebarOpen = true" />
    <UiTooltip v-if="sidebarCollapsed && !pane" :text="$t('nav.expandSidebar')" shortcut="[">
      <UiButton class="ph-expand" variant="ghost" size="sm" :icon="PanelLeftOpen" @click="sidebarCollapsed = false" />
    </UiTooltip>
    <nav class="ph-crumbs" :aria-label="$t('common.breadcrumb')">
      <template v-if="borrowed">
        <NuxtLink :to="links.home(wsId)" class="ph-crumb ph-personal"><Lock :size="12" />{{ $t('personal.title') }}</NuxtLink>
        <ChevronRight v-if="crumbs?.length || title" :size="13" class="ph-sep" />
      </template>
      <template v-for="(c, i) in crumbs ?? []" :key="i">
        <ChevronRight v-if="i > 0" :size="13" class="ph-sep" />
        <component :is="c.to ? NuxtLink : 'span'" :to="c.to" class="ph-crumb" :class="{ last: i === (crumbs?.length ?? 0) - 1 }">
          <span v-if="c.emoji" class="ph-emoji">{{ c.emoji }}</span>
          <component :is="c.icon" v-else-if="c.icon" :size="14" class="ph-icon" />
          <span class="truncate">{{ c.label }}</span>
        </component>
      </template>
      <span v-if="title" class="ph-crumb last"><span class="truncate">{{ title }}</span></span>
      <slot name="title" />
    </nav>
    <div class="ph-actions"><slot /></div>
  </header>
</template>

<style scoped>
.ph {
  display: flex;
  align-items: center;
  gap: 6px;
  height: var(--header-h);
  padding: 0 12px 0 16px;
  border-bottom: 1px solid var(--border);
  flex: none;
  min-width: 0;
  background: var(--bg);
}
.ph-menu { display: none; margin-left: -6px; }
.ph-personal { gap: 5px; color: var(--accent-text, var(--accent)); font-weight: 500; flex: none; }
.ph-crumbs { display: flex; align-items: center; gap: 4px; min-width: 0; flex: 1; overflow: hidden; }
.ph-crumb {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  height: 26px;
  padding: 0 6px;
  border-radius: var(--r-sm);
  color: var(--text-2);
  font-weight: 500;
  white-space: nowrap;
}
.ph-crumb:not(.last) { flex-shrink: 1; max-width: 220px; }
a.ph-crumb:hover { background: var(--bg-hover); color: var(--text); }
.ph-crumb.last { color: var(--text); flex-shrink: 1; }
.ph-sep { color: var(--text-4); flex: none; }
.ph-icon { color: var(--text-3); flex: none; }
.ph-emoji { flex: none; }
.ph-actions { display: flex; align-items: center; gap: 4px; flex: none; }
@media (max-width: 1023px) {
  .ph-menu { display: inline-flex; }
  .ph-expand { display: none; }
  .ph { padding: 0 8px 0 12px; }
  .ph-crumb:not(.last) { display: none; }
  .ph-sep { display: none; }
}
</style>
