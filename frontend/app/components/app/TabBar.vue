<script setup lang="ts">
import { Plus, X, Copy, ArrowRightToLine, RotateCcw } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useTabs, tabKeyLabel } from '~/composables/useTabs'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'

const { wsId } = useWorkspace()
const { t } = useI18n()
const tabsApi = useTabs(wsId)
const { tabs, activeId } = tabsApi

// Menu contextuel d'un onglet
const menuOpen = ref(false)
const menuAt = ref<{ x: number; y: number } | null>(null)
const menuTab = ref<string | null>(null)
function openMenu(id: string, e: MouseEvent) {
  menuTab.value = id
  menuAt.value = { x: e.clientX, y: e.clientY }
  menuOpen.value = true
}
const menuItems = computed<MenuItem[]>(() => {
  const id = menuTab.value
  if (!id) return []
  const tab = tabs.value.find((x) => x.id === id)
  return [
    { label: t('tabs.duplicate'), icon: Copy, onSelect: () => tab && dup(tab.path) },
    { divider: true },
    { label: t('tabs.close'), icon: X, shortcut: tabKeyLabel('W'), onSelect: () => tabsApi.close(id, links.home(wsId.value)) },
    { label: t('tabs.closeOthers'), disabled: tabs.value.length < 2, onSelect: () => tabsApi.closeOthers(id) },
    { label: t('tabs.closeRight'), icon: ArrowRightToLine, disabled: tabs.value.at(-1)?.id === id, onSelect: () => tabsApi.closeRight(id) },
    { divider: true },
    { label: t('tabs.reopen'), icon: RotateCcw, shortcut: [...tabKeyLabel('shift'), 'T'], disabled: !tabsApi.canReopen(), onSelect: () => tabsApi.reopen() },
  ]
})
function dup(path: string) {
  tabsApi.open(path, { duplicate: true })
}

// Glisser-déposer pour réordonner
const dragging = ref<string | null>(null)
function onDragStart(id: string, e: DragEvent) {
  dragging.value = id
  e.dataTransfer?.setData('text/plain', id)
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
}
function onDragOver(index: number, e: DragEvent) {
  if (!dragging.value) return
  e.preventDefault()
  tabsApi.move(dragging.value, index)
}
</script>

<template>
  <div class="tabbar-top" role="tablist" :aria-label="t('tabs.title')">
    <div class="tabs-scroll">
      <AppTabItem
        v-for="(tab, i) in tabs" :key="tab.id" :tab="tab" :active="tab.id === activeId" :closable="tabs.length > 1"
        draggable="true" :class="{ dragging: dragging === tab.id }"
        @dragstart="onDragStart(tab.id, $event)" @dragover="onDragOver(i, $event)" @dragend="dragging = null"
        @activate="tabsApi.activate(tab.id)" @close="tabsApi.close(tab.id, links.home(wsId))" @menu="openMenu(tab.id, $event)"
      />
    </div>
    <UiTooltip :text="t('tabs.new')" :shortcut="tabKeyLabel('T')">
      <UiButton size="xs" variant="ghost" :icon="Plus" :aria-label="t('tabs.new')" @click="tabsApi.open(links.home(wsId), { duplicate: true })" />
    </UiTooltip>
    <UiMenu v-model:open="menuOpen" :items="menuItems" :virtual-rect="menuAt" placement="bottom-start" />
  </div>
</template>

<style scoped>
.tabbar-top { display: flex; align-items: center; gap: 4px; height: 36px; padding: 0 8px; border-bottom: 1px solid var(--border); background: var(--bg-sidebar, var(--bg)); flex: none; }
.tabs-scroll { display: flex; align-items: center; gap: 2px; min-width: 0; flex: 0 1 auto; overflow-x: auto; scrollbar-width: none; }
.tabs-scroll::-webkit-scrollbar { display: none; }
.dragging { opacity: .5; }
</style>
