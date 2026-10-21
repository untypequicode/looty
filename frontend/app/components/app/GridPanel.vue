<script setup lang="ts">
import type { Component } from 'vue'
import { MoreHorizontal, GripVertical, Pencil, X } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useI18n } from '~/composables/useI18n'

/**
 * Cadre d'un panneau de `AppPanelGrid` : l'en-tête sert de poignée de déplacement, le menu
 * permet de renommer (titre vide = titre par défaut) et de retirer le panneau. Le contenu
 * défile à l'intérieur du panneau.
 */
const props = defineProps<{ title: string; icon?: Component; menu?: MenuItem[]; renamable?: boolean; /** Panneaux empilés : pas de déplacement. */ fixed?: boolean }>()
const emit = defineEmits<{ (e: 'rename', title: string | null): void; (e: 'remove'): void }>()
const { t } = useI18n()

const menuOpen = ref(false)
const renaming = ref(false)
const draft = ref('')
const input = ref<HTMLInputElement | null>(null)
function startRename() {
  if (!props.renamable) return
  draft.value = props.title
  renaming.value = true
  nextTick(() => input.value?.select())
}
function commit() {
  if (!renaming.value) return
  renaming.value = false
  const v = draft.value.trim()
  if (v !== props.title) emit('rename', v || null)
}
const items = computed<MenuItem[]>(() => [
  ...(props.menu ?? []),
  ...(props.renamable ? [{ label: t('panelGrid.rename'), icon: Pencil, onSelect: startRename }] : []),
  ...(props.menu?.length || props.renamable ? [{ divider: true }] : []),
  { label: t('panelGrid.remove'), icon: X, danger: true, onSelect: () => emit('remove') },
])
</script>

<template>
  <section class="gp" :class="{ fixed }">
    <header class="gp-head pg-handle" :title="renaming || fixed ? undefined : t('panelGrid.move')">
      <GripVertical :size="14" class="gp-grip" />
      <component :is="icon" v-if="icon" :size="15" class="gp-ic" />
      <input v-if="renaming" ref="input" v-model="draft" class="gp-input" :aria-label="t('panelGrid.rename')" @keydown.enter.prevent="commit" @keydown.esc.prevent="renaming = false" @blur="commit" />
      <h3 v-else class="truncate" @dblclick="startRename">{{ title }}</h3>
      <span class="gp-actions"><slot name="actions" /></span>
      <UiMenu v-model:open="menuOpen" :items="items" placement="bottom-end">
        <UiButton size="xs" variant="ghost" :icon="MoreHorizontal" :aria-label="t('common.more')" />
      </UiMenu>
    </header>
    <div class="gp-body"><slot /></div>
  </section>
</template>

<style scoped>
.gp { display: flex; flex-direction: column; min-height: 0; height: 100%; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); overflow: hidden; }
.gp-head { display: flex; align-items: center; gap: 6px; padding: 8px 8px 6px 6px; cursor: grab; user-select: none; flex: none; }
.gp-head:active { cursor: grabbing; }
.gp.fixed .gp-head { cursor: default; }
.gp.fixed .gp-grip { display: none; }
.gp-grip { color: var(--text-3); opacity: 0; transition: opacity var(--dur-fast); flex: none; }
.gp:hover .gp-grip { opacity: 1; }
.gp-ic { color: var(--text-2); flex: none; }
.gp-head h3 { flex: 1; min-width: 0; margin: 0; font-size: var(--fs-md); font-weight: 600; }
.gp-input { flex: 1; min-width: 0; font: inherit; font-weight: 600; padding: 2px 6px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); color: var(--text); cursor: text; }
.gp-actions { display: flex; align-items: center; gap: 2px; cursor: default; }
.gp-actions:empty { display: none; }
.gp-body { flex: 1; min-height: 0; overflow-y: auto; padding: 0 12px 10px; }
@media (pointer: coarse) { .gp-grip { opacity: 1; } }
</style>
