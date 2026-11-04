<script setup lang="ts">
import { FileText, Sigma, Globe, NotebookText, PenTool, LockKeyhole } from 'lucide-vue-next'
import type { Placement } from '@floating-ui/vue'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { usePages, type NewPageProject } from '~/composables/usePages'
import { useI18n } from '~/composables/useI18n'
import { useWorkspaceId, useWorkspace, links } from '~/composables/useWorkspace'

/**
 * « Nouveau » des documents : une page, ou une page qui porte un projet (LaTeX, site,
 * notebook) et s'ouvre sur ses fichiers et son aperçu.
 */
/** `personalOnly` : rôle en lecture seule, qui ne peut créer que des documents personnels. */
const props = withDefaults(defineProps<{ parentId?: string | null; placement?: Placement; personalOnly?: boolean }>(), { parentId: null, placement: 'bottom-end', personalOnly: false })
const emit = defineEmits<{ (e: 'created', id: string): void }>()
const { t } = useI18n()
const { createAndOpen, createPage } = usePages()
const open = ref(false)
const wsId = useWorkspaceId()
const { workspace } = useWorkspace()
// Dans l'espace perso, tout est déjà privé.
const personalSpace = computed(() => !!workspace.data.value?.personal)

async function create(project?: NewPageProject, personal = false) {
  const p = await createAndOpen(props.parentId, project, personal)
  emit('created', p.id)
}
async function createCanvas() {
  const p = await createPage({ parentId: props.parentId, title: t('canvas.untitled'), defaultView: 'canvas' })
  await navigateTo(links.doc(wsId.value, p.id))
  emit('created', p.id)
}
const items = computed<MenuItem[]>(() => props.personalOnly
  ? [{ label: t('personalItem.newPersonalDoc'), icon: LockKeyhole, onSelect: () => create(undefined, true) }]
  : [
  { label: t('projects.pageKinds.doc'), icon: FileText, onSelect: () => create() },
  { label: t('canvas.title'), icon: PenTool, onSelect: createCanvas },
  // Document visible de soi seul, rangé ici (ex. son TP sous la page du cours).
  ...(personalSpace.value ? [] : [{ label: t('personalItem.newPersonalDoc'), icon: LockKeyhole, onSelect: () => create(undefined, true) }]),
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
