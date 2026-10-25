<script setup lang="ts">
import { FileText, Sigma, Globe, NotebookText } from 'lucide-vue-next'
import type { Placement } from '@floating-ui/vue'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { usePages, type NewPageProject } from '~/composables/usePages'
import { useI18n } from '~/composables/useI18n'

/**
 * « Nouveau » des documents : une page, ou une page qui porte un projet (LaTeX, site,
 * notebook) et s'ouvre sur ses fichiers et son aperçu.
 */
const props = withDefaults(defineProps<{ parentId?: string | null; placement?: Placement }>(), { parentId: null, placement: 'bottom-end' })
const emit = defineEmits<{ (e: 'created', id: string): void }>()
const { t } = useI18n()
const { createAndOpen } = usePages()
const open = ref(false)

async function create(project?: NewPageProject) {
  const p = await createAndOpen(props.parentId, project)
  emit('created', p.id)
}
const items = computed<MenuItem[]>(() => [
  { label: t('projects.pageKinds.doc'), icon: FileText, onSelect: () => create() },
  { divider: true },
  { heading: t('projects.pageKinds.projects') },
  { label: t('projects.pageKinds.latex'), icon: Sigma, onSelect: () => create({ kind: 'latex', template: 'article' }) },
  { label: t('projects.pageKinds.site'), icon: Globe, onSelect: () => create({ kind: 'site', template: 'starter' }) },
  { label: t('projects.pageKinds.notebook'), icon: NotebookText, onSelect: () => create({ kind: 'notebook', template: 'starter' }) },
])
</script>

<template>
  <UiMenu v-model:open="open" :items="items" :placement="placement" :width="230">
    <slot :open="open" />
  </UiMenu>
</template>
