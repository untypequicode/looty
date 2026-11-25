<script setup lang="ts">
import { FileText, Sigma, Globe, NotebookText, PenTool, LockKeyhole, BookCheck, LayoutTemplate, Presentation } from 'lucide-vue-next'
import { useTemplates } from '~/composables/useTemplates'
import type { Placement } from '@floating-ui/vue'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { usePages, type NewPageProject } from '~/composables/usePages'
import { useI18n } from '~/composables/useI18n'
import { openStudyCreate } from '~/composables/useStudyCreate'
import { useWorkspaceId, useWorkspace, links } from '~/composables/useWorkspace'

/**
 * « Nouveau » des documents : une page, une page qui porte un projet (LaTeX, site,
 * notebook) et s'ouvre sur ses fichiers et son aperçu, ou un entraînement.
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
// Modèles de document : proposés quand il y en a (ceux de l'espace et les modèles fournis).
defineOptions({ inheritAttrs: false })
const tplOpen = ref(false)
const { data: allTemplates } = useTemplates(wsId, () => !props.personalOnly)
const pageTemplates = computed(() => (allTemplates.value ?? []).filter((x) => x.kind === 'page'))
async function fromTemplate(id: string) {
  tplOpen.value = false
  const p = await createPage({ parentId: props.parentId, templateId: id })
  await navigateTo(links.doc(wsId.value, p.id))
  emit('created', p.id)
}

async function create(project?: NewPageProject, personal = false) {
  const p = await createAndOpen(props.parentId, project, personal)
  emit('created', p.id)
}
async function createCanvas() {
  const p = await createPage({ parentId: props.parentId, title: t('canvas.untitled'), defaultView: 'canvas' })
  await navigateTo(links.doc(wsId.value, p.id))
  emit('created', p.id)
}
async function createSlides() {
  const p = await createPage({ parentId: props.parentId, title: t('slides.untitled'), defaultView: 'slides' })
  await navigateTo(links.doc(wsId.value, p.id))
  emit('created', p.id)
}
const items = computed<MenuItem[]>(() => props.personalOnly
  ? [
      { label: t('personalItem.newPersonalDoc'), icon: LockKeyhole, onSelect: () => create(undefined, true) },
      { label: t('study.new'), icon: BookCheck, onSelect: () => openStudyCreate(wsId.value, { parentId: props.parentId, personal: true }) },
    ]
  : [
  { label: t('projects.pageKinds.doc'), icon: FileText, onSelect: () => create() },
  { label: t('canvas.title'), icon: PenTool, onSelect: createCanvas },
  { label: t('slides.newPresentation'), icon: Presentation, onSelect: createSlides },
  ...(pageTemplates.value.length ? [{ label: t('tpl.fromTemplate'), icon: LayoutTemplate, onSelect: () => (tplOpen.value = true) }] : []),
  // Document visible de soi seul, rangé ici (ex. son TP sous la page du cours).
  ...(personalSpace.value ? [] : [{ label: t('personalItem.newPersonalDoc'), icon: LockKeyhole, onSelect: () => create(undefined, true) }]),
  // Entraînement (QCM, questionnaire, fiches, synthèse) : un élément de l'arbre comme une page.
  { label: t('study.new'), icon: BookCheck, onSelect: () => openStudyCreate(wsId.value, { parentId: props.parentId }) },
  { divider: true },
  { heading: t('projects.pageKinds.projects') },
  { label: t('projects.pageKinds.latex'), icon: Sigma, onSelect: () => create({ kind: 'latex', template: 'article' }) },
  { label: t('projects.pageKinds.site'), icon: Globe, onSelect: () => create({ kind: 'site', template: 'starter' }) },
  { label: t('projects.pageKinds.notebook'), icon: NotebookText, onSelect: () => create({ kind: 'notebook', template: 'starter' }) },
])
</script>

<template>
  <UiMenu v-bind="$attrs" v-model:open="open" :items="items" :placement="placement" :width="230">
    <slot :open="open" />
  </UiMenu>
  <UiDialog v-model:open="tplOpen" :title="t('tpl.pick')" :width="460">
    <div class="tpl-pick">
      <button v-for="x in pageTemplates" :key="x.id" type="button" class="tpl-row" @click="fromTemplate(x.id)">
        <LayoutTemplate :size="16" />
        <span>{{ x.name }}</span>
        <span v-if="x.builtin" class="faint tpl-tag">{{ t('tpl.builtin') }}</span>
      </button>
    </div>
  </UiDialog>
</template>

<style scoped>
.tpl-pick { display: flex; flex-direction: column; gap: 4px; }
.tpl-row { display: flex; align-items: center; gap: 10px; padding: 9px 10px; border-radius: var(--r-md); text-align: left; }
.tpl-row:hover { background: var(--bg-hover); }
.tpl-tag { margin-left: auto; font-size: var(--fs-sm); }
</style>
