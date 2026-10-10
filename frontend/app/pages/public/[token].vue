<script setup lang="ts">
import { workspacePreamble } from '~/editor/mathScope'
import { FileQuestion } from 'lucide-vue-next'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'

/**
 * Lien public : la page dans la vue de lecture complète (polices, taille, largeur, thème,
 * sommaire, progression). Réglages gardés dans le navigateur du visiteur.
 */
const route = useRoute()
const { t } = useI18n()
const page = ref<{ id: string; title: string; icon?: string | null; content?: string | null; updated_at: string; latex_preamble?: string } | null>(null)
const missing = ref(false)

onMounted(async () => {
  try {
    page.value = await api(`/api/public/pages/${route.params.token}`, { silent: true })
    // Macros LaTeX de l'espace : mêmes formules que dans l'application.
    workspacePreamble.value = page.value?.latex_preamble ?? ''
  } catch {
    missing.value = true
  }
})
useHead({ title: () => (page.value ? `${page.value.icon ? `${page.value.icon} ` : ''}${page.value.title || t('docs.untitled')}` : 'Looty') })
</script>

<template>
  <DocReader :page="page" :error="missing" :current-id="String(route.params.token)" storage="local" crumb="Looty">
    <template #error>
      <UiEmpty :icon="FileQuestion" :title="t('public.notFound')" :description="t('public.notFoundHint')" />
    </template>
  </DocReader>
</template>
