<script setup lang="ts">
import { workspacePreamble } from '~/editor/mathScope'
import { setDiagramStyleSettings, type DiagramStyleSettings } from '~/editor/diagramStyles'
import { FileQuestion } from 'lucide-vue-next'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { publicProject, type PublicProjectSummary } from '~/composables/useProjectReader'

/**
 * Lien public : la page dans la vue de lecture complète (polices, taille, largeur, thème,
 * sommaire, progression). Réglages gardés dans le navigateur du visiteur. Le notebook ou le PDF
 * LaTeX porté par la page s'affiche en lecture seule (GLOSSARY : *Lien public*).
 */
const route = useRoute()
const { t } = useI18n()
const page = ref<{ id: string; title: string; icon?: string | null; content?: string | null; updated_at: string; latex_preamble?: string; diagram_styles?: DiagramStyleSettings; project?: PublicProjectSummary | null } | null>(null)
const project = computed(() => publicProject(String(route.params.token), page.value?.project))
const missing = ref(false)

onMounted(async () => {
  try {
    page.value = await api(`/api/public/pages/${route.params.token}`, { silent: true })
    // Macros LaTeX de l'espace : mêmes formules que dans l'application.
    workspacePreamble.value = page.value?.latex_preamble ?? ''
    setDiagramStyleSettings(page.value?.diagram_styles)
  } catch {
    missing.value = true
  }
})
useHead({ title: () => (page.value ? `${page.value.icon ? `${page.value.icon} ` : ''}${page.value.title || t('docs.untitled')}` : 'Looty') })
</script>

<template>
  <DocReader :page="page" :error="missing" :current-id="String(route.params.token)" storage="local" crumb="Looty" :project="project">
    <template #error>
      <UiEmpty :icon="FileQuestion" :title="t('public.notFound')" :description="t('public.notFoundHint')" />
    </template>
  </DocReader>
</template>
