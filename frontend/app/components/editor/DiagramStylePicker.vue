<script setup lang="ts">
import { Palette, Settings2 } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useI18n } from '~/composables/useI18n'
import { BUILTIN_STYLES, diagramStyleSettings, resolveStyle, type DiagramStyle } from '~/editor/diagramStyles'

/** Style d'un diagramme ou d'un bloc graphe, choisi d'un clic (`null` = défaut de l'espace). */
const model = defineModel<string | null>({ default: null })
defineProps<{ editable: boolean }>()
const { t } = useI18n()
const route = useRoute()
const open = ref(false)

const label = (s: DiagramStyle) => (BUILTIN_STYLES.includes(s) ? t(`diagrams.builtin.${s.id}`) : s.name)
const current = computed(() => resolveStyle(model.value))
const custom = computed(() => diagramStyleSettings.value.styles)

const items = computed<MenuItem[]>(() => {
  const ws = route.params.ws as string | undefined
  return [
    { label: t('diagrams.workspaceDefault', { name: label(resolveStyle(null)) }), checked: model.value == null || !custom.value.concat(BUILTIN_STYLES).some((s) => s.id === model.value), onSelect: () => { model.value = null } },
    { divider: true },
    ...BUILTIN_STYLES.map((s) => ({ label: label(s), checked: model.value === s.id, onSelect: () => { model.value = s.id } })),
    ...(custom.value.length ? [{ heading: t('diagrams.workspaceStyles') } as MenuItem, ...custom.value.map((s) => ({ label: s.name, checked: model.value === s.id, onSelect: () => { model.value = s.id } }))] : []),
    ...(ws ? [{ divider: true } as MenuItem, { label: t('diagrams.manage'), icon: Settings2, onSelect: () => navigateTo(`/w/${ws}/settings/workspace#diagram-styles`) }] : []),
  ]
})
</script>

<template>
  <UiMenu v-if="editable" v-model:open="open" :items="items" placement="bottom-end" :width="230">
    <button class="dsp" :title="t('diagrams.style')"><Palette :size="13" /><span class="truncate">{{ label(current) }}</span></button>
  </UiMenu>
</template>

<style scoped>
.dsp { display: inline-flex; align-items: center; gap: 5px; max-width: 160px; height: 26px; padding: 0 8px; border-radius: var(--r-sm); color: var(--text-2); font-size: var(--fs-xs); }
.dsp:hover { background: var(--bg-hover); color: var(--text); }
</style>
